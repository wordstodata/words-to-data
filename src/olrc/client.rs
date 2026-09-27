//! Fetching classification tables from the OLRC, cached.
//!
//! The tables are static HTML on a `.gov` CDN, one page for each session and
//! sort order. A page the cache holds is never fetched again, whatever its age:
//! the cache has no time to live. A run that needs a newer table is told so by
//! deleting the cached page.

use std::cell::Cell;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use super::OlrcError;
use crate::cache::ResponseCache;

/// Where the OLRC publishes its classification tables.
const BASE_URL: &str = "https://usc-cdn.house.gov/classification";

/// How long to wait between two live requests. The CDN states no limit; this is
/// only to be unhurried with it.
const PACE: Duration = Duration::from_secs(5);

/// How this client identifies itself.
const USER_AGENT: &str = concat!(
    "words-to-data/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/wordstodata/words-to-data)"
);

/// One page of a classification table, and where it was published.
#[derive(Debug, Clone)]
pub struct TablePage {
    /// The page as the OLRC published it.
    pub url: String,
    /// The page's HTML.
    pub html: String,
}

pub struct OlrcClient {
    cache: ResponseCache,
    /// `None` means this client may only read the cache.
    agent: Option<ureq::Agent>,
    fetched: Cell<u32>,
    last_request: Cell<Option<Instant>>,
}

impl OlrcClient {
    /// A client that fetches a page the cache does not hold, and writes it
    /// into `cache_dir`.
    ///
    /// `None` for the directory uses the cache this crate shares with its other
    /// clients.
    pub fn new(cache_dir: Option<PathBuf>) -> Self {
        Self {
            cache: ResponseCache::new(None, cache_dir),
            agent: Some(ureq::Agent::new_with_defaults()),
            fetched: Cell::new(0),
            last_request: Cell::new(None),
        }
    }

    /// A client that only reads the cache, and never reaches the network.
    ///
    /// This is what a test uses: it reads the committed page under
    /// `tests/test_data/olrc/classification`.
    pub fn offline(cache_dir: PathBuf) -> Self {
        Self {
            cache: ResponseCache::new(None, Some(cache_dir)),
            agent: None,
            fetched: Cell::new(0),
            last_request: Cell::new(None),
        }
    }

    /// How many pages this client fetched. A page read from the cache is not
    /// counted.
    pub fn fetched(&self) -> u32 {
        self.fetched.get()
    }

    /// The table of one session of one Congress, in public law order:
    /// `tbl119pl_1st.htm` for the 119th Congress, 1st session.
    ///
    /// The public law order is the one read. The Code order publishes the same
    /// rows (see the module's own documentation).
    pub fn public_law_table(&self, congress: u32, session: u32) -> Result<TablePage, OlrcError> {
        let ordinal = match session {
            1 => "1st",
            2 => "2nd",
            other => return Err(OlrcError::Session(other)),
        };
        self.page(&format!("tbl{congress}pl_{ordinal}.htm"))
    }

    /// One page, from the cache when it is there.
    ///
    /// The cache key is `olrc/classification/<page>`, which is also where the
    /// committed fixture sits, so a test and a live run read the same file.
    fn page(&self, name: &str) -> Result<TablePage, OlrcError> {
        let url = format!("{BASE_URL}/{name}");
        let key = format!("olrc/classification/{name}");
        if let Some(html) = self.cache.get(&key) {
            return Ok(TablePage { url, html });
        }

        let Some(agent) = &self.agent else {
            return Err(OlrcError::Offline {
                page: name.to_string(),
                directory: self.cache.directory().display().to_string(),
            });
        };

        self.wait_for_the_pace();
        self.fetched.set(self.fetched.get() + 1);
        self.last_request.set(Some(Instant::now()));

        let html = agent
            .get(&url)
            .header("User-Agent", USER_AGENT)
            .call()
            .and_then(|mut response| response.body_mut().read_to_string())
            .map_err(|error| OlrcError::Http(format!("{url}: {error}")))?;

        self.cache.set(&key, &html)?;
        Ok(TablePage { url, html })
    }

    /// Sleep until [`PACE`] has passed since the last live request.
    fn wait_for_the_pace(&self) {
        if let Some(last) = self.last_request.get() {
            let waited = last.elapsed();
            if waited < PACE {
                std::thread::sleep(PACE - waited);
            }
        }
    }
}
