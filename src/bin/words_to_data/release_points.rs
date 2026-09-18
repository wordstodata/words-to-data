//! Where a US Code release point comes from, and putting one into a dataset.
//!
//! Two commands do this. `build-dataset` makes a dataset out of release points,
//! and `add-release-points` puts more into a dataset that is already there. Two
//! code paths answering one question differently is the class of fault #165
//! was, so both come through here.
//!
//! A release point is how the source publishes and not what the dataset holds:
//! one folder of fifty-seven titles becomes fifty-seven expressions, one per
//! title (`docs/adr/0003-storage-is-keyed-by-work.md`). Nothing about adding one
//! is new, therefore, and no stored type changes.

use std::error::Error as StdError;
use std::path::PathBuf;

use words_to_data::dataset::Dataset;
use words_to_data::storage::Storage;
use words_to_data::uscode::{self, MirrorIndex};

/// Default mirror manifest (release-point date -> zip URL).
pub const DEFAULT_MIRROR_INDEX: &str = "https://wordstodata.com/mirror/uslm/index.json";

/// Where release points are read from: the mirror, or only what is cached.
pub struct ReleaseSource {
    /// The mirror manifest, and `None` when this run must not reach the network.
    index: Option<MirrorIndex>,
    cache_dir: PathBuf,
}

/// The result of looking for one release point.
enum Located {
    /// The folder of extracted XML, ready to parse.
    Folder(PathBuf),
    /// This source does not offer the date, and why not, in words.
    NotOffered(String),
}

impl ReleaseSource {
    /// Read the mirror manifest, and download a release point that is not cached.
    pub fn from_mirror(
        mirror_index: &str,
        cache_dir: Option<&str>,
    ) -> Result<Self, Box<dyn StdError>> {
        Ok(Self {
            index: Some(uscode::fetch_index(mirror_index)?),
            cache_dir: cache_root(cache_dir),
        })
    }

    /// Read only the cache, and never reach the network.
    ///
    /// A date the cache has not got is named rather than downloaded, which is
    /// how `add-opinions --offline` reads the records that are committed.
    pub fn cached_only(cache_dir: Option<&str>) -> Self {
        Self {
            index: None,
            cache_dir: cache_root(cache_dir),
        }
    }

    /// The folder of extracted XML for one release point.
    fn locate(&self, date: &str) -> Result<Located, Box<dyn StdError>> {
        let cached = self.cache_dir.join("uslm").join(date);

        let Some(index) = &self.index else {
            // Release points are immutable, so a folder that is already there
            // is the release point and needs no check against the mirror.
            if is_populated(&cached) {
                return Ok(Located::Folder(cached));
            }
            return Ok(Located::NotOffered(format!(
                "this run reads only the cache, and {} holds nothing",
                cached.display()
            )));
        };

        let Some(url) = index.url_for(date) else {
            return Ok(Located::NotOffered("not in mirror manifest".to_string()));
        };
        Ok(Located::Folder(uscode::ensure_release(
            url,
            date,
            &self.cache_dir,
        )?))
    }
}

/// The cache directory named, or the one shared with the rest of the toolkit.
fn cache_root(cache_dir: Option<&str>) -> PathBuf {
    cache_dir.map_or_else(uscode::default_cache_dir, PathBuf::from)
}

/// A release point is already extracted if its folder holds at least one file.
fn is_populated(folder: &std::path::Path) -> bool {
    folder
        .read_dir()
        .is_ok_and(|mut entries| entries.next().is_some())
}

/// What to do about a date the source does not offer.
pub enum Missing {
    /// Say so and carry on with the rest. `build-dataset` builds what it can.
    Skip,
    /// Stop. A run told to grow a dataset must not report success for a release
    /// point it never added.
    Stop,
}

/// Add each release point named to the dataset, and answer with the dates added.
///
/// Oldest first, so the dataset's expressions arrive in the order the source
/// published them.
pub fn add_all<S: Storage>(
    dataset: &mut Dataset<S>,
    source: &ReleaseSource,
    dates: &[String],
    missing: Missing,
) -> Result<Vec<String>, String> {
    let mut oldest_first = dates.to_vec();
    oldest_first.sort();

    let mut added = Vec::new();
    for date in &oldest_first {
        println!("Loading release point {date}...");
        let located = source
            .locate(date)
            .map_err(|e| format!("release point {date} could not be fetched: {e}"))?;

        let folder = match located {
            Located::Folder(folder) => folder,
            Located::NotOffered(reason) => match missing {
                Missing::Skip => {
                    eprintln!("Skipping {date}: {reason}");
                    continue;
                }
                Missing::Stop => return Err(format!("release point {date} is not here: {reason}")),
            },
        };

        println!("Parsing {date}...");
        dataset
            .add_uslm_folder(&folder.to_string_lossy(), date, None)
            .map_err(|e| format!("release point {date} could not be added: {e}"))?;
        added.push(date.clone());
    }

    Ok(added)
}
