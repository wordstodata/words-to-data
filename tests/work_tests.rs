//! Works and expressions, over the real corpus.
//!
//! The point of this shape is that it survives a document class with no release
//! cycle. These tests use statutes, because that is the corpus we have, but each
//! one is written so it would still make sense for a court opinion: a work with
//! a single expression is the normal case here, not a degenerate one.

use words_to_data::dataset::{Dataset, DatasetMetadata, ExpressionId, WorkId, works_between};

const EARLY: &str = "2025-07-18";
const LATE: &str = "2025-07-30";

/// Title 51 gained two sections between the two release points.
const AMENDED: &str = "usc51";
/// Title 9 did not change.
const UNCHANGED: &str = "usc09";

fn empty_dataset() -> Dataset<words_to_data::storage::InMemoryStorage> {
    Dataset::new(DatasetMetadata {
        name: "Works".to_string(),
        description: "real US Code titles".to_string(),
        author: "words_to_data tests".to_string(),
        source_urls: vec![],
        license: "MIT".to_string(),
        version: "1.0.0".to_string(),
        ..Default::default()
    })
}

fn dataset_of(title: &str, dates: &[&str]) -> Dataset<words_to_data::storage::InMemoryStorage> {
    let mut dataset = empty_dataset();
    for date in dates {
        dataset
            .add_uslm_xml(
                &format!("tests/test_data/usc/{date}/{title}.xml"),
                date,
                None,
            )
            .expect("the corpus should parse and load");
    }
    dataset
}

/// The ids of every expression of one work, oldest first.
fn ids_of(
    dataset: &Dataset<words_to_data::storage::InMemoryStorage>,
    work: &WorkId,
) -> Vec<ExpressionId> {
    dataset
        .expressions(work)
        .expect("expressions should list")
        .into_iter()
        .map(|info| info.id)
        .collect()
}

#[test]
fn should_name_every_work_the_dataset_holds() {
    let dataset = dataset_of(AMENDED, &[EARLY, LATE]);

    assert_eq!(
        dataset.works().expect("works should list"),
        vec![WorkId::new("uscode/title_51")],
        "two releases of one title are one work, not two"
    );
}

#[test]
fn should_list_an_expression_for_each_date_a_work_was_published() {
    let dataset = dataset_of(AMENDED, &[EARLY, LATE]);
    let work = WorkId::new("uscode/title_51");

    assert_eq!(
        ids_of(&dataset, &work),
        vec![
            ExpressionId::new(work.clone(), EARLY),
            ExpressionId::new(work, LATE),
        ]
    );
}

/// A work published once has one expression. For a court opinion that is the
/// only case there will ever be, so it must be ordinary rather than special.
#[test]
fn should_give_one_expression_for_a_work_published_once() {
    let dataset = dataset_of(UNCHANGED, &[EARLY]);
    let work = WorkId::new("uscode/title_9");

    assert_eq!(
        ids_of(&dataset, &work),
        vec![ExpressionId::new(work, EARLY)]
    );
}

#[test]
fn should_read_the_text_of_one_expression() {
    let dataset = dataset_of(AMENDED, &[EARLY, LATE]);
    let id = ExpressionId::new(WorkId::new("uscode/title_51"), LATE);

    let expression = dataset
        .get_expression(&id)
        .expect("reading should work")
        .expect("the expression should be there");

    assert_eq!(expression.id, id);
    assert_eq!(expression.element.data.path.to_string(), "uscode/title_51");
}

#[test]
fn should_find_no_expression_for_a_date_the_work_was_not_published() {
    let dataset = dataset_of(AMENDED, &[EARLY, LATE]);
    let id = ExpressionId::new(WorkId::new("uscode/title_51"), "1999-01-01");

    assert!(
        dataset
            .get_expression(&id)
            .expect("reading should work")
            .is_none()
    );
}

/// The identifier prints in the form Akoma Ntoso and ELI use, so it can be
/// written in a citation or a URL without translation.
#[test]
fn should_print_an_expression_as_work_at_date() {
    let id = ExpressionId::new(WorkId::new("uscode/title_9"), EARLY);

    assert_eq!(id.to_string(), "uscode/title_9@2025-07-18");
}

/// A form that can be written must also be readable, or it cannot come back in
/// on a command line, in a citation, or as the target of a link.
#[test]
fn should_read_an_expression_written_as_work_at_date() {
    let id: ExpressionId = "uscode/title_9@2025-07-18"
        .parse()
        .expect("the printed form should parse");

    assert_eq!(id.work, WorkId::new("uscode/title_9"));
    assert_eq!(id.at, EARLY);
}

#[test]
fn should_round_trip_an_expression_through_its_printed_form() {
    let id = ExpressionId::new(WorkId::new("uscode/title_51"), LATE);

    assert_eq!(
        id.to_string()
            .parse::<ExpressionId>()
            .expect("should parse"),
        id
    );
}

/// A path holds slashes and a date holds dashes, so the last `@` is the only
/// unambiguous split point. Refusing the rest keeps a typo from becoming a
/// lookup that quietly finds nothing.
#[test]
fn should_refuse_text_that_does_not_name_a_work_and_a_date() {
    for bad in [
        "uscode/title_9",           // no date
        "@2025-07-18",              // no work
        "uscode/title_9@",          // empty date
        "uscode/title_9@last-july", // not a date
        "",
    ] {
        assert!(
            bad.parse::<ExpressionId>().is_err(),
            "{bad:?} should not parse as an expression"
        );
    }
}

// --- Two documents that share no date ---
//
// This is what #69 exists for. Statutes are the corpus we have, so title 9 is
// taken at one release point and title 51 at the other, which gives a dataset
// whose two documents have nothing in common but the file they live in. Ten
// court opinions would look the same and read the same.

fn two_works_no_shared_date() -> Dataset<words_to_data::storage::InMemoryStorage> {
    let mut dataset = empty_dataset();
    dataset
        .add_uslm_xml(
            &format!("tests/test_data/usc/{EARLY}/{UNCHANGED}.xml"),
            EARLY,
            None,
        )
        .expect("title 9 should load");
    dataset
        .add_uslm_xml(
            &format!("tests/test_data/usc/{LATE}/{AMENDED}.xml"),
            LATE,
            None,
        )
        .expect("title 51 should load");
    dataset
}

#[test]
fn should_hold_two_works_that_share_no_publication_date() {
    let dataset = two_works_no_shared_date();

    assert_eq!(
        dataset.works().expect("works should list"),
        vec![
            WorkId::new("uscode/title_51"),
            WorkId::new("uscode/title_9"),
        ],
        "both documents are held, though neither shares the other's date"
    );
}

#[test]
fn should_give_each_work_only_its_own_date() {
    let dataset = two_works_no_shared_date();

    assert_eq!(
        ids_of(&dataset, &WorkId::new("uscode/title_9")),
        vec![ExpressionId::new(WorkId::new("uscode/title_9"), EARLY)]
    );
    assert_eq!(
        ids_of(&dataset, &WorkId::new("uscode/title_51")),
        vec![ExpressionId::new(WorkId::new("uscode/title_51"), LATE)]
    );
}

/// The old global list made this question answerable across documents: "the
/// version after 2025-07-18" would hand back title 51, which is not a later
/// reading of title 9 but a different document entirely.
#[test]
fn should_find_no_next_expression_across_a_different_work() {
    let dataset = two_works_no_shared_date();
    let title_9 = ExpressionId::new(WorkId::new("uscode/title_9"), EARLY);

    assert!(
        dataset
            .next_expression(&title_9)
            .expect("reading should work")
            .is_none(),
        "title 51 is a different document, not the next reading of title 9"
    );
}

#[test]
fn should_refuse_a_diff_across_two_works() {
    let dataset = two_works_no_shared_date();

    let refused = dataset.compute_diff(
        &ExpressionId::new(WorkId::new("uscode/title_9"), EARLY),
        &ExpressionId::new(WorkId::new("uscode/title_51"), LATE),
    );

    assert!(
        matches!(
            refused,
            Err(words_to_data::dataset::DatasetError::WorkMismatch { .. })
        ),
        "expected WorkMismatch, got {refused:?}"
    );
}

// --- Covering a whole corpus, one work at a time ---
//
// A diff is per work, so a job that used to be one call over a global tree is
// now one call per document. Deciding which documents that is belongs here
// rather than in each command that needs it.

/// Both titles at both release points: every work is diffable between them.
fn two_works_both_dates() -> Dataset<words_to_data::storage::InMemoryStorage> {
    let mut dataset = empty_dataset();
    for title in [UNCHANGED, AMENDED] {
        for date in [EARLY, LATE] {
            dataset
                .add_uslm_xml(
                    &format!("tests/test_data/usc/{date}/{title}.xml"),
                    date,
                    None,
                )
                .expect("the corpus should load");
        }
    }
    dataset
}

#[test]
fn should_pair_every_work_published_on_both_dates() {
    let dataset = two_works_both_dates();

    let between = works_between(&dataset, EARLY, LATE).expect("works should pair");

    assert_eq!(
        between.pairs,
        vec![
            (
                ExpressionId::new(WorkId::new("uscode/title_51"), EARLY),
                ExpressionId::new(WorkId::new("uscode/title_51"), LATE),
            ),
            (
                ExpressionId::new(WorkId::new("uscode/title_9"), EARLY),
                ExpressionId::new(WorkId::new("uscode/title_9"), LATE),
            ),
        ]
    );
    assert!(between.skipped.is_empty());
}

/// A work published on only one of the two dates cannot be diffed between
/// them. It is reported rather than dropped: a corpus run that quietly covered
/// none of the works would otherwise print success for a job it did not do.
#[test]
fn should_report_works_that_are_not_held_on_both_dates() {
    let dataset = two_works_no_shared_date();

    let between = works_between(&dataset, EARLY, LATE).expect("works should pair");

    assert!(between.pairs.is_empty(), "neither title spans both dates");
    assert_eq!(
        between.skipped,
        vec![
            WorkId::new("uscode/title_51"),
            WorkId::new("uscode/title_9"),
        ]
    );
}
