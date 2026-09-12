use std::time::Duration;

use words_to_data::congress::ResponseCache;

#[test]
fn should_keep_file_on_disk_when_entry_expired() {
    let temp = tempfile::tempdir().expect("a temporary directory");
    let dir = temp.path().join("cache");
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
}

#[test]
fn should_never_expire_when_ttl_none() {
    let temp = tempfile::tempdir().expect("a temporary directory");
    let dir = temp.path().join("cache");
    let cache = ResponseCache::new(None, Some(dir));

    cache.set("member/A000375.json", "payload").unwrap();

    // No TTL configured -> entries never expire, regardless of file age.
    assert_eq!(
        cache.get("member/A000375.json"),
        Some("payload".to_string())
    );
}
