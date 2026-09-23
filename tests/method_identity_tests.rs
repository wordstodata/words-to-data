//! A method has an identity that can change: `{name, version}` (#182).
//!
//! A link already says who made a statement and how. What it could not say is
//! whether that *how* is still the current *how*: the name stayed the same
//! while the answers changed underneath. A method therefore carries a version,
//! chosen by a person when the method's answers change. A hash of the method's
//! parameters was considered and rejected, because it churns on cosmetic edits
//! and trains everybody to ignore it (#179, decision 10).
//!
//! The dataset under test is the real U.S. Code, title 1 of the 2025-07-30
//! release point, from `tests/test_data/usc`.

use words_to_data::citation::resolve::{SectionPaths, resolve};
use words_to_data::citation::{Opinion, cites_links, usc};
use words_to_data::dataset::{Dataset, DatasetMetadata, ExpressionId, Format, WorkId};
use words_to_data::link::Link;
use words_to_data::method::Method;
use words_to_data::storage::{InMemoryStorage, LinkReader, SqliteStorage};

const RELEASE: &str = "2025-07-30";
const TITLE_1: &str = "tests/test_data/usc/2025-07-30/usc01.xml";
const EARLIER_RELEASE: &str = "2025-07-18";
const TITLE_1_EARLIER: &str = "tests/test_data/usc/2025-07-18/usc01.xml";
const WORK_1: &str = "uscode/title_1";

/// The rule that reads a U.S.C. citation out of an opinion, as it stands now.
const CITATION_RULE: &str = "reporters-db laws.json U.S.C. patterns";

/// A dataset holding title 1, and an index of where its sections are.
fn dataset_holding_title_1() -> (Dataset<InMemoryStorage>, SectionPaths) {
    let mut dataset = Dataset::new(DatasetMetadata {
        name: "Method identity fixture".to_string(),
        ..Default::default()
    });
    dataset
        .add_uslm_xml(TITLE_1, RELEASE, None)
        .expect("title 1 should parse");

    let id = ExpressionId::new(WorkId::new(WORK_1), RELEASE);
    let expression = dataset
        .get_expression(&id)
        .expect("storage should answer")
        .expect("title 1 should be held");
    let mut paths = SectionPaths::new();
    paths.add_work(&expression.root);

    (dataset, paths)
}

/// One real citation link: Obergefell citing 1 U.S.C. § 1.
fn citation_link(dataset: &Dataset<InMemoryStorage>, paths: &SectionPaths) -> Link {
    let scope = dataset.scope().expect("scope should derive");
    let found = usc::find("Words of one gender include the other, 1 U.S.C. § 1 (2018).");
    let citation = found.first().expect("one citation");
    let cited = resolve(citation, &scope, paths);
    let opinion = Opinion::new("2812209", "Obergefell v. Hodges, 576 U.S. 644 (2015)");

    cites_links(&opinion, citation, &cited)
        .into_iter()
        .next()
        .expect("the citation should make one link")
}

/// Round-trip a dataset through SQLite, so both backends are exercised.
///
/// The caller must keep the returned directory in scope: dropping it removes
/// the database.
fn through_sqlite(
    dataset: &Dataset<InMemoryStorage>,
) -> (tempfile::TempDir, Dataset<SqliteStorage>) {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let file = dir.path().join("dataset.sqlite");
    dataset.save_to_sqlite(&file).expect("save to sqlite");
    let sqlite = Dataset::open_sqlite(&file).expect("open sqlite");
    (dir, sqlite)
}

/// Round-trip a dataset through a W2D file.
fn through_w2d(
    dataset: &Dataset<InMemoryStorage>,
) -> (tempfile::TempDir, Dataset<InMemoryStorage>) {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let file = dir.path().join("dataset.w2d");
    let file = file.to_str().expect("a temporary path is UTF-8");
    dataset
        .save(file, Format::Compact)
        .expect("save to a W2D file");
    let reloaded = Dataset::load(file, Format::Compact).expect("load the W2D file");
    (dir, reloaded)
}

#[test]
fn should_name_the_version_of_the_method_that_made_a_link_in_both_stored_forms() {
    let (mut dataset, paths) = dataset_holding_title_1();
    let link = citation_link(&dataset, &paths);
    dataset
        .add_link(link.clone())
        .expect("the link should be stored");

    let (_db_dir, sqlite) = through_sqlite(&dataset);
    let (_file_dir, w2d) = through_w2d(&dataset);

    // Both stored forms must agree. A record readable from a database and
    // absent from a W2D file, or the reverse, is worse than not having it.
    for (label, stored) in [
        ("memory", dataset.links_by_kind("judicial.cites").unwrap()),
        ("sqlite", sqlite.links_by_kind("judicial.cites").unwrap()),
        ("w2d", w2d.links_by_kind("judicial.cites").unwrap()),
    ] {
        let found = stored
            .iter()
            .find(|stored| stored.id() == link.id())
            .unwrap_or_else(|| panic!("{label} lost the citation link"));

        assert_eq!(
            found.provenance.method.as_ref(),
            Some(&Method::new(CITATION_RULE, 1)),
            "{label} should name the method and the version it was at"
        );
    }
}

/// A dataset could not say what had been done to it, so a missing step read as
/// a complete file. It now records which method, at which version, ran over
/// which window (#179, decision 11).
///
/// Which **method**, not which step. "`redesignations` has run here" stays true
/// for ever while the thing it means changes underneath. "This reasoning was
/// applied to this window" is what an agent can act on.
///
/// The record passes the honesty test that kept #153 out of this break: "method
/// M at version V ran over window W" is a record of something that happened,
/// and it stays true however much the dataset grows
/// (`docs/adr/0007-a-record-is-what-was-said-everything-else-is-derived.md`).
#[test]
fn should_say_which_method_at_which_version_ran_over_which_window_in_both_stored_forms() {
    let mut dataset = Dataset::new(DatasetMetadata {
        name: "What has run over this dataset".to_string(),
        ..Default::default()
    });
    dataset
        .add_uslm_xml(TITLE_1_EARLIER, EARLIER_RELEASE, None)
        .expect("title 1 should parse at the earlier release point");
    dataset
        .add_uslm_xml(TITLE_1, RELEASE, None)
        .expect("title 1 should parse at the later release point");

    let from = ExpressionId::new(WorkId::new(WORK_1), EARLIER_RELEASE);
    let to = ExpressionId::new(WorkId::new(WORK_1), RELEASE);
    // A method this build really runs, rather than a name invented here.
    let method = words_to_data::legislature::redesignation::reading_method();
    dataset
        .record_method_run(method.clone(), &from, &to)
        .expect("the run should record");

    let (_db_dir, sqlite) = through_sqlite(&dataset);
    let (_file_dir, w2d) = through_w2d(&dataset);

    for (label, runs) in [
        ("memory", dataset.method_runs().to_vec()),
        ("sqlite", sqlite.method_runs().to_vec()),
        ("w2d", w2d.method_runs().to_vec()),
    ] {
        assert_eq!(runs.len(), 1, "{label} should hold one run");
        let run = &runs[0];
        assert_eq!(run.method, method, "{label} should name the method");
        assert_eq!(run.method.version, 1, "{label} should name the version");
        assert_eq!(run.work.as_str(), WORK_1, "{label} should name the work");
        assert_eq!(run.from_date, EARLIER_RELEASE, "{label}: window start");
        assert_eq!(run.to_date, RELEASE, "{label}: window end");
    }
}

/// Recording the same run twice leaves one record.
///
/// A run is identified by what it says — this method, at this version, over
/// this window — exactly as a link is
/// (`docs/adr/0004-links-are-stored-and-identified-by-what-they-say.md`). A
/// rebuild that runs a step twice must not grow the file.
#[test]
fn should_leave_one_record_when_the_same_run_is_recorded_twice() {
    let mut dataset = Dataset::new(DatasetMetadata::default());
    dataset
        .add_uslm_xml(TITLE_1_EARLIER, EARLIER_RELEASE, None)
        .expect("title 1 should parse at the earlier release point");
    dataset
        .add_uslm_xml(TITLE_1, RELEASE, None)
        .expect("title 1 should parse at the later release point");

    let from = ExpressionId::new(WorkId::new(WORK_1), EARLIER_RELEASE);
    let to = ExpressionId::new(WorkId::new(WORK_1), RELEASE);
    let method = words_to_data::legislature::redesignation::reading_method();

    dataset
        .record_method_run(method.clone(), &from, &to)
        .expect("the first run should record");
    dataset
        .record_method_run(method.clone(), &from, &to)
        .expect("the second run should record");

    assert_eq!(dataset.method_runs().len(), 1);

    // A later version of the same method is a different record, because it is
    // a different reasoning and a reader has to be able to tell them apart.
    dataset
        .record_method_run(Method::new(&method.name, method.version + 1), &from, &to)
        .expect("the later version should record");

    assert_eq!(dataset.method_runs().len(), 2);
}
