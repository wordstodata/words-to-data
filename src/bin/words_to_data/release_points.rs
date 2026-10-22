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

use std::collections::BTreeSet;
use std::error::Error as StdError;
use std::path::{Path, PathBuf};

use serde::Serialize;

use words_to_data::dataset::Dataset;
use words_to_data::progress::Progress;
use words_to_data::storage::Storage;
use words_to_data::uscode::{self, MirrorIndex};

use crate::ui::Task;

/// Default mirror manifest (release-point date -> zip URL).
pub const DEFAULT_MIRROR_INDEX: &str = "https://wordstodata.com/mirror/uslm/index.json";

/// Where release points are read from: the mirror, or only what is cached.
pub struct ReleaseSource {
    /// The mirror manifest, and `None` when this run must not reach the network.
    index: Option<MirrorIndex>,
    cache_dir: PathBuf,
}

/// One release point, and where it can be read from.
#[derive(Serialize)]
pub struct Listed {
    /// The release-point date, `YYYY-MM-DD`.
    pub date: String,
    /// Whether the mirror offers it, and `None` when the mirror was not read.
    pub in_mirror: Option<bool>,
    /// Whether the cache holds it.
    pub in_cache: bool,
}

/// The result of looking for one release point.
enum Located {
    /// The folder of extracted XML, ready to parse.
    Folder(PathBuf),
    /// This source does not offer the date, and why not, in words.
    NotOffered(String),
}

impl ReleaseSource {
    /// The source a run reads, made the one way every command makes it.
    ///
    /// Online, the mirror manifest is read, and a release point that is not
    /// cached is downloaded. `offline` reads only the cache and never reaches
    /// the network: a date the cache has not got is named, not downloaded.
    /// `build-dataset`, `add-release-points` and `release-points` all come
    /// through here, so the listing and the build cannot disagree about where
    /// a release point comes from.
    pub fn new(
        mirror_index: &str,
        cache_dir: Option<&str>,
        offline: bool,
    ) -> Result<Self, Box<dyn StdError>> {
        let index = if offline {
            None
        } else {
            Some(uscode::fetch_index(mirror_index)?)
        };
        Ok(Self {
            index,
            cache_dir: cache_root(cache_dir),
        })
    }

    /// The cache directory this source reads and fills.
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// Each release point this source knows of, oldest first.
    ///
    /// A date is listed if the mirror offers it or the cache holds it. Offline
    /// the mirror is not read, so `in_mirror` is `None` for every date.
    pub fn list(&self) -> Vec<Listed> {
        let cached = self.cached_dates();
        let offered: BTreeSet<String> = self
            .index
            .as_ref()
            .map(|index| index.dates().into_iter().map(str::to_string).collect())
            .unwrap_or_default();

        cached
            .union(&offered)
            .map(|date| Listed {
                date: date.clone(),
                in_mirror: self.index.as_ref().map(|_| offered.contains(date)),
                in_cache: cached.contains(date),
            })
            .collect()
    }

    /// The dates whose `<cache>/uslm/<date>` folder holds a release point.
    ///
    /// The same test `locate` makes, so a date listed as cached is a date a
    /// run reads from the cache.
    fn cached_dates(&self) -> BTreeSet<String> {
        let Ok(entries) = self.cache_dir.join("uslm").read_dir() else {
            return BTreeSet::new();
        };
        entries
            .filter_map(Result::ok)
            .filter(|entry| is_populated(&entry.path()))
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect()
    }

    /// The folder of extracted XML for one release point.
    fn locate(&self, date: &str, progress: &dyn Progress) -> Result<Located, Box<dyn StdError>> {
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
            progress,
        )?))
    }
}

/// The cache directory named, or the one shared with the rest of the toolkit.
fn cache_root(cache_dir: Option<&str>) -> PathBuf {
    cache_dir.map_or_else(uscode::default_cache_dir, PathBuf::from)
}

/// A release point is already extracted if its folder holds at least one file.
fn is_populated(folder: &Path) -> bool {
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
        let task = Task::start(format!("Release point {date}"));
        let located = source
            .locate(date, &task)
            .map_err(|e| format!("release point {date} could not be fetched: {e}"))?;

        let folder = match located {
            Located::Folder(folder) => folder,
            Located::NotOffered(reason) => match missing {
                Missing::Skip => {
                    task.done(&format!("skipped: {reason}"));
                    continue;
                }
                Missing::Stop => return Err(format!("release point {date} is not here: {reason}")),
            },
        };

        dataset
            .add_uslm_folder_reporting(&folder.to_string_lossy(), date, None, &task)
            .map_err(|e| format!("release point {date} could not be added: {e}"))?;
        task.done(&format!("{} titles", xml_files_in(&folder)));
        added.push(date.clone());
    }

    Ok(added)
}

/// How many XML files a release point's folder holds: one per title.
fn xml_files_in(folder: &Path) -> usize {
    folder.read_dir().map_or(0, |entries| {
        entries
            .filter_map(Result::ok)
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "xml"))
            .count()
    })
}
