//! A cache of responses from an outside publisher, on disk.
//!
//! One directory, one file per response, keyed by a path the caller chooses. It
//! belongs to no one publisher: `congress` keys on an API endpoint and
//! `courtlistener` keys on a record id, and both write under the same shared
//! directory.
//!
//! Two rules matter more than they look.
//!
//! **A read never deletes.** An entry past its time to live is reported as a
//! miss and is left on disk, because the committed test fixtures live in this
//! cache and a test that read one must not destroy it.
//!
//! **A cached response is the record.** Every API this crate reads is rate
//! limited — CourtListener allows 125 requests a day — so a second run must
//! answer from the cache rather than spend the quota again.

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

pub struct ResponseCache {
    cache_dir: PathBuf,
    /// Time-to-live for cached entries. `None` means entries never expire,
    /// which is what committed test fixtures rely on.
    ttl: Option<Duration>,
}

impl ResponseCache {
    pub fn new(ttl: Option<Duration>, cache_dir: Option<PathBuf>) -> Self {
        let cache_dir =
            cache_dir.unwrap_or_else(|| dirs::cache_dir().unwrap().join("words_to_data/"));
        Self { cache_dir, ttl }
    }

    /// The directory this cache reads and writes, so a caller can say where it
    /// looked.
    pub fn directory(&self) -> &Path {
        &self.cache_dir
    }

    fn key_to_path(&self, key: &str) -> PathBuf {
        self.cache_dir.join(key)
    }

    pub fn get(&self, key: &str) -> Option<String> {
        let path = self.key_to_path(key);

        if !path.exists() {
            return None;
        }

        // Check TTL. A read is non-destructive: an expired entry is treated as a
        // cache miss but is never deleted, so committed fixtures survive reads.
        if let Some(ttl) = self.ttl {
            let metadata = fs::metadata(&path).ok()?;
            let modified = metadata.modified().ok()?;
            let age = SystemTime::now().duration_since(modified).ok()?;

            if age > ttl {
                return None;
            }
        }

        let mut file = fs::File::open(&path).ok()?;
        let mut contents = String::new();
        file.read_to_string(&mut contents).ok()?;

        Some(contents)
    }

    /// Write a response under `key`. The error is `std::io::Error` rather than
    /// one publisher's error type, because the cache serves several.
    pub fn set(&self, key: &str, data: &str) -> std::io::Result<()> {
        let path = self.key_to_path(key);
        // Unwrap shouldn't be an issue here, since we're always given a reasonable key
        fs::create_dir_all(path.parent().expect("A valid path should have been provided to the cache, there should _always_ be a directory and filename"))?;

        let mut file = fs::File::create(&path)?;
        file.write_all(data.as_bytes())?;

        Ok(())
    }

    pub fn clear(&self) -> std::io::Result<()> {
        if self.cache_dir.exists() {
            fs::remove_dir_all(&self.cache_dir)?;
        }
        Ok(())
    }
}
