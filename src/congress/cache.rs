use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use super::CongressError;

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

    fn key_to_path(&self, key: &str) -> PathBuf {
        // Sanitize key for filesystem
        //let safe_key = key.replace(['/', '\\'], "_");
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

    pub fn set(&self, key: &str, data: &str) -> Result<(), CongressError> {
        let path = self.key_to_path(key);
        // Unwrap shouldn't be an issue here, since we're always given a reasonable key
        fs::create_dir_all(path.parent().expect("A valid path should have been provided to the cache, there should _always_ be a directory and filename"))?;

        let mut file = fs::File::create(&path)?;
        file.write_all(data.as_bytes())?;

        Ok(())
    }

    pub fn clear(&self) -> Result<(), CongressError> {
        if self.cache_dir.exists() {
            fs::remove_dir_all(&self.cache_dir)?;
        }
        Ok(())
    }
}
