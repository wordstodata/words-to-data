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
fn through_sqlite(dataset: &Dataset<InMemoryStorage>) -> (tempfile::TempDir, Dataset<SqliteStorage>) {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let file = dir.path().join("dataset.sqlite");
    dataset.save_to_sqlite(&file).expect("save to sqlite");
    let sqlite = Dataset::open_sqlite(&file).expect("open sqlite");
    (dir, sqlite)
}

/// Round-trip a dataset through a W2D file.
fn through_w2d(dataset: &Dataset<InMemoryStorage>) -> (tempfile::TempDir, Dataset<InMemoryStorage>) {
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
