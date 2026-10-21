//! End-to-end tests over `release-points`, driving the real binary (#292).
//!
//! `build-dataset` and `add-release-points` take release-point dates, and no
//! command listed the dates there are. These tests pin the listing: each date,
//! oldest first, with whether the mirror offers it and whether the cache holds
//! it.
//!
//! A cache is the committed corpus copied under `<cache>/uslm/<date>`, which is
//! what a download leaves behind. The mirror is the committed manifest
//! (`tests/test_data/mirror/index.json`), served from a local port, so no test
//! reaches the network.

use std::process::{Command, Output};

/// Where the committed release points are.
const CORPUS: &str = "tests/test_data/usc";

/// The committed mirror manifest. It offers 2025-06-12, 2025-07-18 and 2025-07-30.
const MANIFEST: &str = "tests/test_data/mirror/index.json";

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args(args)
        .output()
        .expect("the binary should run")
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn stderr_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

/// A cache directory holding title 1 of each release point named.
fn cache_holding(name: &str, dates: &[&str]) -> String {
    let cache = format!("{}/{name}_cache", env!("CARGO_TARGET_TMPDIR"));
    let _ = std::fs::remove_dir_all(&cache);
    for date in dates {
        let folder = format!("{cache}/uslm/{date}");
        std::fs::create_dir_all(&folder).expect("the cache folder should be creatable");
        std::fs::copy(
            format!("{CORPUS}/{date}/usc01.xml"),
            format!("{folder}/usc01.xml"),
        )
        .expect("the corpus should hold this release point");
    }
    cache
}

/// The address of a local server that answers every request with the
/// committed mirror manifest.
fn mirror_serving_the_manifest() -> String {
    use std::io::{BufRead, BufReader, Write};

    let manifest = std::fs::read(MANIFEST).expect("the manifest is committed");
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a local port");
    let address = listener.local_addr().expect("the port's address");
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            // Read the request up to its blank line before answering it.
            let mut reader = BufReader::new(&stream);
            let mut line = String::new();
            while reader.read_line(&mut line).is_ok_and(|read| read > 2) {
                line.clear();
            }
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                manifest.len()
            );
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(&manifest);
        }
    });
    format!("http://{address}/index.json")
}

/// The listing as `--json` prints it.
fn json_listing(output: &Output) -> serde_json::Value {
    assert!(
        output.status.success(),
        "release-points should exit zero, stderr: {}",
        stderr_of(output)
    );
    let printed = stdout_of(output);
    serde_json::from_str(&printed)
        .unwrap_or_else(|e| panic!("stdout should parse as JSON ({e}), got:\n{printed}"))
}

/// The lines of the listing that name a date, in the order printed.
fn date_lines(printed: &str) -> Vec<String> {
    printed
        .lines()
        .filter(|line| line.starts_with("20"))
        .map(str::to_string)
        .collect()
}

#[test]
fn should_list_exactly_the_cached_release_points_when_the_run_is_offline() {
    let cache = cache_holding("list_offline", &["2025-07-30", "2025-07-18", "2025-08-14"]);

    let output = run(&["release-points", "--offline", "--cache-dir", &cache]);

    assert!(
        output.status.success(),
        "release-points should exit zero, stderr: {}",
        stderr_of(&output)
    );
    let printed = stdout_of(&output);
    let dates: Vec<String> = date_lines(&printed)
        .iter()
        .map(|line| line.split_whitespace().next().unwrap().to_string())
        .collect();
    assert_eq!(
        dates,
        vec!["2025-07-18", "2025-07-30", "2025-08-14"],
        "every cached release point, oldest first, and nothing else, got:\n{printed}"
    );
    assert!(
        date_lines(&printed)
            .iter()
            .all(|line| line.contains("cache only")),
        "offline, each date is marked as in the cache only, got:\n{printed}"
    );
    assert!(
        printed.contains(&cache),
        "the listing names the cache directory it read, got:\n{printed}"
    );
    assert!(
        printed.contains("not the mirror"),
        "and says that it lists the cache, not the mirror, got:\n{printed}"
    );
}

/// A pipe reads `--json`, so stdout is the JSON and nothing else.
#[test]
fn should_print_parseable_json_with_no_mirror_answer_when_the_run_is_offline() {
    let cache = cache_holding("list_offline_json", &["2025-07-18", "2025-07-30"]);

    let output = run(&[
        "release-points",
        "--offline",
        "--cache-dir",
        &cache,
        "--json",
    ]);

    assert!(
        output.status.success(),
        "release-points should exit zero, stderr: {}",
        stderr_of(&output)
    );
    let printed = stdout_of(&output);
    let listing: serde_json::Value = serde_json::from_str(&printed)
        .unwrap_or_else(|e| panic!("stdout should parse as JSON ({e}), got:\n{printed}"));

    assert_eq!(
        listing["cache_dir"], cache,
        "the JSON names the cache it read"
    );
    assert!(
        listing["mirror_index"].is_null(),
        "offline, no mirror was read, got: {listing}"
    );
    let points = listing["release_points"]
        .as_array()
        .expect("release_points should be a list");
    assert_eq!(points.len(), 2, "one entry per cached release point");
    for (point, date) in points.iter().zip(["2025-07-18", "2025-07-30"]) {
        assert_eq!(point["date"], date, "oldest first, got: {point}");
        assert!(
            point["in_mirror"].is_null(),
            "offline, the mirror is not asked, got: {point}"
        );
        assert_eq!(point["in_cache"], true, "got: {point}");
    }
}

/// Online, a date is listed if the mirror offers it or the cache holds it, and
/// a date the mirror has dropped is still listed, as the cache's alone.
#[test]
fn should_list_the_mirror_and_the_cache_together_when_the_run_is_online() {
    let mirror = mirror_serving_the_manifest();
    let cache = cache_holding("list_online", &["2025-07-18", "2025-07-30", "2025-08-14"]);

    let listing = json_listing(&run(&[
        "release-points",
        "--mirror-index",
        &mirror,
        "--cache-dir",
        &cache,
        "--json",
    ]));

    assert_eq!(
        listing["mirror_index"], mirror,
        "the JSON names the mirror read"
    );
    assert_eq!(listing["cache_dir"], cache, "and the cache read");
    let found: Vec<(String, bool, bool)> = listing["release_points"]
        .as_array()
        .expect("release_points should be a list")
        .iter()
        .map(|point| {
            (
                point["date"].as_str().unwrap().to_string(),
                point["in_mirror"]
                    .as_bool()
                    .expect("online, the mirror answers"),
                point["in_cache"].as_bool().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        found,
        vec![
            ("2025-06-12".to_string(), true, false),
            ("2025-07-18".to_string(), true, true),
            ("2025-07-30".to_string(), true, true),
            ("2025-08-14".to_string(), false, true),
        ],
        "the union, oldest first, each marked with where it is"
    );
}

/// The text form says the same: a date only the cache holds is marked so.
#[test]
fn should_mark_a_date_the_mirror_dropped_when_the_run_is_online() {
    let mirror = mirror_serving_the_manifest();
    let cache = cache_holding("list_online_text", &["2025-08-14"]);

    let output = run(&[
        "release-points",
        "--mirror-index",
        &mirror,
        "--cache-dir",
        &cache,
    ]);

    assert!(
        output.status.success(),
        "release-points should exit zero, stderr: {}",
        stderr_of(&output)
    );
    let printed = stdout_of(&output);
    let line_for = |date: &str| {
        date_lines(&printed)
            .into_iter()
            .find(|line| line.starts_with(date))
            .unwrap_or_else(|| panic!("{date} should be listed, got:\n{printed}"))
    };
    assert!(
        line_for("2025-08-14").contains("cache only")
            && line_for("2025-08-14").contains("no longer offers"),
        "a date only the cache holds is marked so, got:\n{printed}"
    );
    assert!(
        line_for("2025-06-12").contains("mirror only"),
        "a date only the mirror offers is marked so, got:\n{printed}"
    );
    assert!(
        printed.contains(&cache) && printed.contains(&mirror),
        "the listing names the cache and the mirror it read, got:\n{printed}"
    );
}

/// The listing and the build read one source, so they agree.
///
/// Every date listed as offered by the mirror goes into a build from the same
/// mirror and cache, and the dataset holds it. The date only the cache holds is
/// not offered, and the build skips it. The test takes the offered dates the
/// cache also holds, because a date the cache has not got would be downloaded,
/// and no test reaches the network.
#[test]
fn should_name_only_dates_build_dataset_accepts_when_it_says_the_mirror_offers_them() {
    let mirror = mirror_serving_the_manifest();
    let cache = cache_holding("list_agrees", &["2025-07-18", "2025-07-30", "2025-08-14"]);

    let listing = json_listing(&run(&[
        "release-points",
        "--mirror-index",
        &mirror,
        "--cache-dir",
        &cache,
        "--json",
    ]));
    let points = listing["release_points"].as_array().unwrap();
    let offered_and_cached: Vec<&str> = points
        .iter()
        .filter(|p| p["in_mirror"] == true && p["in_cache"] == true)
        .map(|p| p["date"].as_str().unwrap())
        .collect();
    let not_offered: Vec<&str> = points
        .iter()
        .filter(|p| p["in_mirror"] == false)
        .map(|p| p["date"].as_str().unwrap())
        .collect();
    assert_eq!(offered_and_cached, vec!["2025-07-18", "2025-07-30"]);
    assert_eq!(not_offered, vec!["2025-08-14"]);

    let dataset = format!("{}/list_agrees.json", env!("CARGO_TARGET_TMPDIR"));
    let every_date = [offered_and_cached.clone(), not_offered.clone()].concat();
    let built = run(&[
        "build-dataset",
        "--uslm-dates",
        &every_date.join(","),
        "--mirror-index",
        &mirror,
        "--cache-dir",
        &cache,
        &dataset,
    ]);
    assert!(
        built.status.success(),
        "build-dataset should exit zero, stderr: {}",
        stderr_of(&built)
    );

    let held = stdout_of(&run(&["expressions", &dataset]));
    for date in offered_and_cached {
        assert!(
            held.contains(&format!("uscode/title_1@{date}")),
            "{date} was listed as offered, so the build holds it, got:\n{held}"
        );
    }
    for date in not_offered {
        assert!(
            !held.contains(date),
            "{date} was listed as not offered, so the build skips it, got:\n{held}"
        );
    }
}

/// Every file under a folder, with its bytes, in a fixed order.
fn tree_of(folder: &str) -> Vec<(String, Vec<u8>)> {
    fn walk(path: &std::path::Path, into: &mut Vec<(String, Vec<u8>)>) {
        let Ok(entries) = std::fs::read_dir(path) else {
            return;
        };
        for entry in entries.map(Result::unwrap) {
            let path = entry.path();
            if path.is_dir() {
                into.push((path.display().to_string(), Vec::new()));
                walk(&path, into);
            } else {
                into.push((path.display().to_string(), std::fs::read(&path).unwrap()));
            }
        }
    }
    let mut tree = Vec::new();
    walk(std::path::Path::new(folder), &mut tree);
    tree.sort();
    tree
}

/// A listing only reads. A date the mirror offers and the cache has not got is
/// listed, not downloaded, and a cache that is not there is not made.
#[test]
fn should_write_nothing_to_the_cache_when_it_lists() {
    let mirror = mirror_serving_the_manifest();
    let cache = cache_holding("list_writes_nothing", &["2025-07-18", "2025-08-14"]);
    let before = tree_of(&cache);

    for offline in [false, true] {
        let mut args = vec![
            "release-points",
            "--mirror-index",
            &mirror,
            "--cache-dir",
            &cache,
        ];
        if offline {
            args.push("--offline");
        }
        let output = run(&args);
        assert!(
            output.status.success(),
            "release-points should exit zero, stderr: {}",
            stderr_of(&output)
        );
        assert_eq!(
            tree_of(&cache),
            before,
            "the cache is as it was (offline: {offline})"
        );
    }

    let no_cache = format!("{}/list_no_cache_cache", env!("CARGO_TARGET_TMPDIR"));
    let _ = std::fs::remove_dir_all(&no_cache);
    let listing = json_listing(&run(&[
        "release-points",
        "--mirror-index",
        &mirror,
        "--cache-dir",
        &no_cache,
        "--json",
    ]));
    assert!(
        listing["release_points"]
            .as_array()
            .unwrap()
            .iter()
            .all(|p| p["in_cache"] == false),
        "a cache that is not there holds nothing, got: {listing}"
    );
    assert!(
        !std::path::Path::new(&no_cache).exists(),
        "and a listing does not make it"
    );
}
