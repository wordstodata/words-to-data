use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use super::CongressError;

pub struct ResponseCache {
    cache_dir: PathBuf,
    ttl: Duration,
}

impl ResponseCache {
    pub fn new(ttl: Duration) -> Self {
        let xdg_dir = dirs::cache_dir().unwrap().join("words_to_data/");
        Self {
            cache_dir: xdg_dir,
            ttl,
        }
    }

    fn key_to_path(&self, key: &str) -> PathBuf {
        // Sanitize key for filesystem
        let safe_key = key.replace(['/', '\\'], "_");
        self.cache_dir.join(safe_key)
    }

    pub fn get(&self, key: &str) -> Option<String> {
        let path = self.key_to_path(key);

        if !path.exists() {
            return None;
        }

        // Check TTL
        let metadata = fs::metadata(&path).ok()?;
        let modified = metadata.modified().ok()?;
        let age = SystemTime::now().duration_since(modified).ok()?;

        if age > self.ttl {
            // Expired
            let _ = fs::remove_file(&path);
            return None;
        }

        let mut file = fs::File::open(&path).ok()?;
        let mut contents = String::new();
        file.read_to_string(&mut contents).ok()?;

        Some(contents)
    }

    pub fn set(&self, key: &str, data: &str) -> Result<(), CongressError> {
        fs::create_dir_all(&self.cache_dir)?;

        let path = self.key_to_path(key);
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
