use std::time::Duration;

use words_to_data::congress::ResponseCache;

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("w2d_cache_test_{}_{}", std::process::id(), tag));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn should_keep_file_on_disk_when_entry_expired() {
    let dir = temp_dir("keep");
    let cache = ResponseCache::new(Some(Duration::ZERO), Some(dir.clone()));

    cache.set("bill/x.json", "payload").unwrap();

    // Zero TTL -> the entry is stale, so a read is a miss.
    assert_eq!(cache.get("bill/x.json"), None);
    // ...but a read must never delete a cached file. Committed test fixtures
    // live in this cache and must survive being read.
    assert!(
        dir.join("bill/x.json").exists(),
        "expired read must not delete the cached file"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn should_never_expire_when_ttl_none() {
    let dir = temp_dir("never");
    let cache = ResponseCache::new(None, Some(dir.clone()));

    cache.set("member/A000375.json", "payload").unwrap();

    // No TTL configured -> entries never expire, regardless of file age.
    assert_eq!(
        cache.get("member/A000375.json"),
        Some("payload".to_string())
    );

    let _ = std::fs::remove_dir_all(&dir);
}
