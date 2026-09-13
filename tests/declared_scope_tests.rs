//! A dataset can declare what it means to cover, so a gap can be reported (#71).
//!
//! A derived scope says what a dataset holds. It cannot say what the dataset
//! *meant* to hold, so "title 26 is missing" and "title 26 was never wanted"
//! look the same. A declaration separates them, and the difference is the one a
//! researcher needs told rather than left to discover.

use words_to_data::dataset::{Coverage, Dataset, DatasetMetadata, Declaration, Exclusion};
use words_to_data::storage::InMemoryStorage;

const TITLE_9: &str = "tests/test_data/usc/2025-07-18/usc09.xml";
const EARLY: &str = "2025-07-18";

/// A real dataset holding title 9 at one release point, declaring whatever the
/// caller says. Only the declaration is authored; the law is the committed corpus.
fn dataset_declaring(declaration: Option<Declaration>) -> Dataset<InMemoryStorage> {
    let mut dataset = Dataset::new(DatasetMetadata {
        name: "Declared scope fixture".to_string(),
        declaration,
        ..Default::default()
    });
    dataset
        .add_uslm_xml(TITLE_9, EARLY, None)
        .expect("title 9 should parse");
    dataset
}

#[test]
fn should_report_a_declared_work_it_does_not_hold_as_a_gap() {
    let dataset = dataset_declaring(Some(Declaration {
        intends: vec!["uscode/title_9".to_string(), "uscode/title_26".to_string()],
        ..Default::default()
    }));

    let scope = dataset.scope().expect("scope should derive");

    assert_eq!(
        scope.gaps(),
        vec!["uscode/title_26".to_string()],
        "title 26 was declared and is not held, so it is an unexplained absence"
    );
    assert_eq!(
        scope.covers("uscode/title_26"),
        Coverage::Gap,
        "a declared work that is absent means the dataset is incomplete, \
         which is a different answer from being out of its lane"
    );
    assert_eq!(scope.covers("uscode/title_9"), Coverage::InScope);
}

#[test]
fn should_not_report_a_stated_hole_as_a_gap() {
    let dataset = dataset_declaring(Some(Declaration {
        intends: vec!["uscode/title_9".to_string(), "uscode/title_26".to_string()],
        excludes: vec![Exclusion {
            path: "uscode/title_26".to_string(),
            reason: "the source failed to download".to_string(),
        }],
        ..Default::default()
    }));

    let scope = dataset.scope().expect("scope should derive");

    assert!(
        scope.gaps().is_empty(),
        "a hole the producer stated, with a reason, is documented rather than \
         a fault: {:?}",
        scope.gaps()
    );
    assert_eq!(
        scope.covers("uscode/title_26"),
        Coverage::OutOfScope,
        "deliberately left out is out of the lane, not an incomplete build"
    );

    // The reason has to survive, or the exclusion says no more than silence.
    let declared = scope.declared.as_ref().expect("a declaration was made");
    assert_eq!(declared.excludes[0].reason, "the source failed to download");
}

#[test]
fn should_answer_in_scope_for_material_it_holds_but_never_declared() {
    // Declares title 26 only, holds title 9. Under-declaring is careless, but
    // saying we do not hold title 9 would be a false statement about our own
    // contents.
    let dataset = dataset_declaring(Some(Declaration {
        intends: vec!["uscode/title_26".to_string()],
        ..Default::default()
    }));

    let scope = dataset.scope().expect("scope should derive");

    assert_eq!(scope.covers("uscode/title_9"), Coverage::InScope);
    assert_eq!(scope.covers("uscode/title_26"), Coverage::Gap);
}

#[test]
fn should_answer_exactly_as_before_when_nothing_is_declared() {
    let dataset = dataset_declaring(None);

    let scope = dataset.scope().expect("scope should derive");

    // This feature must not change what an undeclared dataset says. Every
    // dataset in existence is undeclared until it is rebuilt.
    assert!(scope.declared.is_none());
    assert!(scope.gaps().is_empty());
    assert_eq!(scope.covers("uscode/title_9"), Coverage::InScope);
    assert_eq!(
        scope.covers("uscode/title_26"),
        Coverage::OutOfScope,
        "with nothing declared there is no third answer to give"
    );
}

#[test]
fn should_carry_the_declaration_through_sqlite() {
    let declaration = Declaration {
        intends: vec!["uscode/title_9".to_string(), "uscode/title_26".to_string()],
        excludes: vec![Exclusion {
            path: "uscode/title_26".to_string(),
            reason: "the source failed to download".to_string(),
        }],
        dates: None,
        namespaces: vec!["legislature".to_string()],
    };
    let memory = dataset_declaring(Some(declaration.clone()));

    let dir = tempfile::tempdir().expect("a temporary directory");
    let file = dir.path().join("declaration.sqlite");
    memory.save_to_sqlite(&file).expect("save to sqlite");
    let sqlite = Dataset::open_sqlite(&file).expect("open sqlite");

    // A declaration that does not survive the file it travels in cannot be
    // read by the party it was written for.
    assert_eq!(
        sqlite.metadata().declaration.as_ref(),
        Some(&declaration),
        "the declaration must round-trip unchanged"
    );
    assert_eq!(
        sqlite.scope().expect("scope").gaps(),
        memory.scope().expect("scope").gaps(),
        "both backends must agree on what is missing"
    );
}

#[test]
fn should_hold_a_legislature_it_declared_before_it_has_any_bills() {
    // Deciding from contents alone, a legislative dataset that has not been
    // given bills yet reports that it holds no legislature at all. That is a
    // wrong answer we shipped knowingly; the declaration is what fixes it.
    let dataset = dataset_declaring(Some(Declaration {
        namespaces: vec!["legislature".to_string()],
        ..Default::default()
    }));

    assert!(
        dataset.legislature().is_some(),
        "a dataset that declares the legislature namespace holds a legislature, \
         even before any bill is added"
    );
}

#[test]
fn should_hold_no_legislature_when_it_declares_none_and_has_none() {
    let dataset = dataset_declaring(Some(Declaration {
        intends: vec!["uscode/title_9".to_string()],
        ..Default::default()
    }));

    assert!(
        dataset.legislature().is_none(),
        "declaring documents only, and holding no bills, means no legislature"
    );
}

/// Save a dataset and run `info` against it the way a person or an agent would.
fn info_output(dataset: &Dataset<InMemoryStorage>) -> String {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let file = dir.path().join("dataset.sqlite");
    dataset.save_to_sqlite(&file).expect("save to sqlite");

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args(["info", file.to_str().expect("a utf-8 path")])
        .output()
        .expect("the binary should run");
    assert!(output.status.success(), "info should exit zero");
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn should_print_an_incomplete_line_when_the_build_left_a_gap() {
    let dataset = dataset_declaring(Some(Declaration {
        intends: vec!["uscode/title_9".to_string(), "uscode/title_26".to_string()],
        ..Default::default()
    }));

    let stdout = info_output(&dataset);

    // A silent gap is the failure mode this feature exists to remove, so the
    // one thing `info` must not do is print a dataset like this as if it were
    // whole.
    assert!(
        stdout.contains("INCOMPLETE"),
        "info should say the dataset is incomplete, got:\n{stdout}"
    );
    assert!(
        stdout.contains("uscode/title_26"),
        "it should name what is missing, got:\n{stdout}"
    );
}

#[test]
fn should_print_no_declaration_lines_when_nothing_was_declared() {
    let stdout = info_output(&dataset_declaring(None));

    // Every dataset is undeclared until it is rebuilt, so this output is what
    // almost every reader sees and it must not have changed.
    for absent in ["INCOMPLETE", "Declared:", "Excluded:", "Namespaces:"] {
        assert!(
            !stdout.contains(absent),
            "an undeclared dataset should not print {absent:?}, got:\n{stdout}"
        );
    }
    assert!(stdout.contains("Covers:"), "it still reports what it holds");
}
