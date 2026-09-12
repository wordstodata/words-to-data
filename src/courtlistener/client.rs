//! Fetching CourtListener records, paced and cached.
//!
//! The quota is the reason this file exists rather than a bare `agent.get`. A
//! token allows 5 requests a minute, 50 an hour and 125 a day on a rolling
//! window, and a refused request still counts against all three. So:
//!
//! - every response is written to the shared cache and read back from it, which
//!   makes a second run of any command free;
//! - live requests are spaced by [`PACE`], so a run of twenty does not trip the
//!   per-minute limit and get itself throttled;
//! - the client counts what it spent, so a caller can report it.

use std::cell::Cell;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use super::CourtListenerError;
use crate::cache::ResponseCache;

const BASE_URL: &str = "https://www.courtlistener.com/api/rest/v4";

/// How long to wait between live requests.
///
/// The limit is 5 a minute, so 12 seconds is the floor and 13 leaves a margin
/// for a clock that disagrees with theirs. Being throttled is worse than being
/// slow: a refused request costs a request.
const PACE: Duration = Duration::from_secs(13);

/// How long a cached response stays fresh.
///
/// Long, on purpose. An opinion is published once and never amended, so the text
/// does not go stale; what can change is the metadata CourtListener holds about
/// it, and re-reading that is not worth a request from a daily budget of 125.
const TTL: Duration = Duration::from_secs(30 * 24 * 60 * 60);

/// How this client identifies itself, as the terms of use ask.
const USER_AGENT: &str = concat!(
    "words-to-data/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/wordstodata/words-to-data)"
);

pub struct CourtListenerClient {
    /// `None` means this client may only read the cache.
    token: Option<String>,
    cache: ResponseCache,
    agent: ureq::Agent,
    spent: Cell<u32>,
    last_request: Cell<Option<Instant>>,
}

impl CourtListenerClient {
    /// A client that may fetch, using `token` and writing into `cache_dir`.
    ///
    /// `None` for the directory uses the cache this crate shares with the
    /// Congress client.
    pub fn new(token: String, cache_dir: Option<PathBuf>) -> Self {
        Self::build(Some(token), Some(TTL), cache_dir)
    }

    /// A client that may only read the cache, and never expires an entry.
    ///
    /// This is what a test uses: it reads the committed records under
    /// `tests/test_data/courtlistener` and fails loudly rather than reaching the
    /// network, so a suite cannot quietly spend the maintainer's quota.
    pub fn offline(cache_dir: PathBuf) -> Self {
        Self::build(None, None, Some(cache_dir))
    }

    fn build(token: Option<String>, ttl: Option<Duration>, cache_dir: Option<PathBuf>) -> Self {
        Self {
            token,
            cache: ResponseCache::new(ttl, cache_dir),
            agent: ureq::Agent::new_with_defaults(),
            spent: Cell::new(0),
            last_request: Cell::new(None),
        }
    }

    /// How many live requests this client has made.
    ///
    /// The figure to report after a run. A cache hit is not a request.
    pub fn requests_spent(&self) -> u32 {
        self.spent.get()
    }

    /// One opinion record, as JSON.
    pub fn opinion(&self, id: u64) -> Result<String, CourtListenerError> {
        self.record("opinion", "opinions", id)
    }

    /// One cluster record, as JSON. This is where the filing date lives.
    pub fn cluster(&self, id: u64) -> Result<String, CourtListenerError> {
        self.record("cluster", "clusters", id)
    }

    /// Fetch one record of one kind, from the cache when it is there.
    ///
    /// The cache key is `courtlistener/<kind>_<id>.json`, which is also the name
    /// the committed fixtures use, so a test and a live run read the same file.
    fn record(
        &self,
        kind: &'static str,
        collection: &str,
        id: u64,
    ) -> Result<String, CourtListenerError> {
        let key = format!("courtlistener/{kind}_{id}.json");
        if let Some(cached) = self.cache.get(&key) {
            return Ok(cached);
        }

        let Some(token) = &self.token else {
            return Err(CourtListenerError::Offline {
                kind,
                id,
                directory: self.cache.directory().display().to_string(),
            });
        };

        self.wait_for_the_pace();
        let url = format!("{BASE_URL}/{collection}/{id}/");

        // Counted before the call, not after. A request that comes back 429 or
        // 401 has still been spent, and a counter that only counts successes
        // under-reports exactly when the number matters.
        self.spent.set(self.spent.get() + 1);
        self.last_request.set(Some(Instant::now()));

        let mut response = self
            .agent
            .get(&url)
            .header("Authorization", &format!("Token {token}"))
            .header("User-Agent", USER_AGENT)
            .call()
            .map_err(|error| match error {
                ureq::Error::StatusCode(429) => CourtListenerError::RateLimited,
                ureq::Error::StatusCode(401) | ureq::Error::StatusCode(403) => {
                    CourtListenerError::Unauthorized
                }
                ureq::Error::StatusCode(404) => CourtListenerError::NotFound { kind, id },
                // The token must never reach a message. `url` carries no
                // credentials; the header did, and the header is not printed.
                other => CourtListenerError::Http(format!("{url}: {other}")),
            })?;

        let body = response
            .body_mut()
            .read_to_string()
            .map_err(|error| CourtListenerError::Http(error.to_string()))?;

        self.cache.set(&key, &body)?;
        Ok(body)
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
