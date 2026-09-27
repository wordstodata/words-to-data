//! `diff`, scoped to a subtree.
//!
//! `diff` printed 461 changed paths over the real corpus with no way to name the
//! part of the title being read, so the way through was to grep the output
//! (#235). The scope is the `Locator` #234 settled, not a flag of this command's
//! own, so `diff`, `search` and `annotations` mean the same thing by a path.
//!
//! The corpus is real: title 51 at two release points, which is the smallest
//! title that carries a real change between them.

use std::sync::OnceLock;

use words_to_data::dataset::{Dataset, DatasetMetadata};

const EARLY: &str = "2025-07-18";
const LATE: &str = "2025-07-30";
const SPACE: &str = "uscode/title_51";

/// The two subtitles title 51 gained a section in between the release points.
///
/// One change in each, so a scope naming one subtile has something to keep and
/// something to leave out.
const SUBTITLE_II: &str = "uscode/title_51/subtitle_II";
const SUBTITLE_V: &str = "uscode/title_51/subtitle_V";

/// Title 51 at both release points, written once for every test here.
fn amended_fixture() -> &'static str {
    static FIXTURE: OnceLock<String> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let path = format!("{}/scoped_report_usc51.sqlite", env!("CARGO_TARGET_TMPDIR"));
        let _ = std::fs::remove_file(&path);
        let mut dataset = Dataset::new(DatasetMetadata::default());
        for date in [EARLY, LATE] {
            dataset
                .add_uslm_xml(&format!("tests/test_data/usc/{date}/usc51.xml"), date, None)
                .expect("the corpus should parse and load");
        }
        dataset
            .save_to_sqlite(&path)
            .expect("the fixture should save");
        path
    })
}

/// A path keeps the changes inside it and drops the changes elsewhere.
#[test]
fn should_list_only_the_changes_inside_a_path_when_diff_names_one() {
    let whole_title = diff_json(&[]);
    assert!(
        changed_paths(&whole_title)
            .iter()
            .any(|p| p.starts_with(SUBTITLE_II)),
        "title 51 should change inside subtitle II, got {:?}",
        changed_paths(&whole_title)
    );
    assert!(
        changed_paths(&whole_title)
            .iter()
            .any(|p| p.starts_with(SUBTITLE_V)),
        "title 51 should change inside subtitle V too, got {:?}",
        changed_paths(&whole_title)
    );

    let scoped = diff_json(&["--path", SUBTITLE_II]);
    let found = changed_paths(&scoped);
    assert!(!found.is_empty(), "subtitle II should hold a change");
    assert!(
        found.iter().all(|path| path.starts_with(SUBTITLE_II)),
        "every path should sit inside subtitle II, got {found:?}"
    );
}

/// One `diff --json` run over the fixture, with any extra arguments.
fn diff_json(extra: &[&str]) -> serde_json::Value {
    let from = format!("{SPACE}@{EARLY}");
    let to = format!("{SPACE}@{LATE}");
    let mut args = vec![
        "diff",
        amended_fixture(),
        "--from",
        &from,
        "--to",
        &to,
        "--json",
    ];
    args.extend_from_slice(extra);

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args(&args)
        .output()
        .expect("the binary should run");
    assert!(
        output.status.success(),
        "{args:?} should exit zero, said:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("diff --json should emit json")
}

/// Every path a diff reported as changed, added, removed or moved.
///
/// A move is not a removal plus an addition, so the four lists are kept apart in
/// the report. A scope has to reach all of them, so the test reads them as one.
fn changed_paths(summary: &serde_json::Value) -> Vec<String> {
    let mut found = Vec::new();
    for list in ["changed_paths", "added_paths", "removed_paths"] {
        for path in summary[list].as_array().into_iter().flatten() {
            found.push(path.as_str().expect("a path").to_string());
        }
    }
    for moved in summary["moved_paths"].as_array().into_iter().flatten() {
        for end in ["from", "to"] {
            found.push(moved[end].as_str().expect("a path").to_string());
        }
    }
    found
}
