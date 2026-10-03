//! Downloading US Code release points from our mirror.
//!
//! The mirror publishes a manifest (`index.json`) mapping each release-point
//! date to an absolute zip URL. This module reads that manifest and fetches
//! the release-point archives on demand, caching extracted XML so repeated
//! runs don't re-download hundreds of megabytes.

use std::collections::HashMap;
use std::error::Error as StdError;
use std::fs;
use std::io::{self, Cursor, Read};
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::progress::Progress;

/// Release-point zips are hundreds of MB, well over ureq's default read cap.
const MAX_ZIP_BYTES: u64 = 4 * 1024 * 1024 * 1024;

/// The mirror manifest: available release-point dates and their zip URLs.
#[derive(Debug, Clone, Deserialize)]
pub struct MirrorIndex {
    /// Release-point dates (`YYYY-MM-DD`), newest first.
    pub versions: Vec<String>,
    /// Map of date to the absolute URL of that release point's zip.
    pub download: HashMap<String, String>,
}

impl MirrorIndex {
    /// Zip URL for a given release-point date, if the mirror has it.
    pub fn url_for(&self, date: &str) -> Option<&str> {
        self.download.get(date).map(String::as_str)
    }
}

/// Fetch and parse the mirror manifest from `index_url` (e.g.
/// `https://wordstodata.com/mirror/uslm/index.json`).
pub fn fetch_index(index_url: &str) -> Result<MirrorIndex, Box<dyn StdError>> {
    let body = crate::http::Http::new()
        .call(|agent| agent.get(index_url).call())?
        .body_mut()
        .read_to_string()?;
    Ok(serde_json::from_str(&body)?)
}

/// Download the release-point zip at `url` and extract its XML into `dest_dir`.
///
/// The zip is hundreds of megabytes, so `progress` hears each chunk as it
/// arrives, counted in bytes against the length the server states.
pub fn download_release(
    url: &str,
    dest_dir: &Path,
    progress: &dyn Progress,
) -> Result<(), Box<dyn StdError>> {
    let mut response = crate::http::Http::new().call(|agent| agent.get(url).call())?;
    let body = response.body_mut();
    progress.begin_bytes("download", body.content_length());

    let mut reader = body.with_config().limit(MAX_ZIP_BYTES).reader();
    let mut bytes = Vec::new();
    let mut chunk = vec![0; 1 << 16];
    loop {
        let read = reader.read(&mut chunk)?;
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&chunk[..read]);
        progress.advance(read as u64);
    }

    progress.begin("extract", None);
    extract_release(&bytes, dest_dir)?;
    Ok(())
}

/// The persistent cache root shared with the rest of the toolkit (e.g. the
/// Congress client): `<user cache dir>/words_to_data`.
pub fn default_cache_dir() -> PathBuf {
    dirs::cache_dir()
        .expect("no user cache directory available")
        .join("words_to_data")
}

/// Return the folder of extracted XML for `date`, downloading + extracting the
/// release point from `url` only if it isn't already cached under `cache_dir`.
///
/// Release points are immutable, so a populated `<cache_dir>/uslm/<date>` folder
/// is always reused — this is what stops repeated runs from re-downloading.
pub fn ensure_release(
    url: &str,
    date: &str,
    cache_dir: &Path,
    progress: &dyn Progress,
) -> Result<PathBuf, Box<dyn StdError>> {
    let folder = cache_dir.join("uslm").join(date);
    if !is_populated(&folder) {
        download_release(url, &folder, progress)?;
    }
    Ok(folder)
}

/// A release point is already extracted if its folder holds at least one file.
fn is_populated(folder: &Path) -> bool {
    folder
        .read_dir()
        .map(|mut entries| entries.next().is_some())
        .unwrap_or(false)
}

/// Extract every `.xml` file from a release-point zip into `dest_dir`, flat.
///
/// Files are written by base name (any internal directory structure is
/// dropped) so the result is a single flat folder ready for
/// [`crate::utils::load_uslm_folder`], regardless of how the archive is laid
/// out. `dest_dir` is created if missing.
pub fn extract_release(zip_bytes: &[u8], dest_dir: &Path) -> io::Result<()> {
    fs::create_dir_all(dest_dir)?;

    let mut archive = zip::ZipArchive::new(Cursor::new(zip_bytes))
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        // Keep only XML files, flattened to their base name.
        let Some(name) = entry.enclosed_name() else {
            continue;
        };
        if name.extension().is_none_or(|ext| ext != "xml") {
            continue;
        }
        let Some(file_name) = name.file_name() else {
            continue;
        };

        let mut out = fs::File::create(dest_dir.join(file_name))?;
        io::copy(&mut entry, &mut out)?;
    }

    Ok(())
}
