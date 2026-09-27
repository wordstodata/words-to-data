//! Full-text search, asked with a [`Locator`] and a limit.
//!
//! `search_text` took a query string and nothing else, so one word over the
//! maintainer's corpus answered with 1135 lines and 224 KB — the same provision
//! returned once per release point, each hit carrying a whole section's text
//! (#235).
//!
//! The vocabulary is the one #234 settled: a `Locator` says where to look, an
//! `Answer` says how much matched, and `DEFAULT_LIMIT` says what a screenful is.
//! Nothing new is invented here.

use std::io::BufRead;
use std::sync::OnceLock;

use words_to_data::dataset::{Dataset, DatasetMetadata, SearchResult};
use words_to_data::query::{DEFAULT_LIMIT, Locator, PathMatch};
use words_to_data::storage::{DocumentReader, InMemoryStorage, SqliteStorage};

/// The two release points the corpus holds.
const EARLY: &str = "2025-07-18";
const LATE: &str = "2025-07-30";

/// Title 9 (Arbitration), 110 KB, held here at both release points.
const ARBITRATION: &str = "uscode/title_9";
/// Title 4 (Flag and Seal), 290 KB, held here at the earlier one only.
const FLAG: &str = "uscode/title_4";

/// A word both titles use, so a scoped run has something to leave out.
const SHARED_WORD: &str = "United States";

/// A word title 9 uses throughout, including inside [`SECTION_16`].
const ARBITRATION_WORD: &str = "arbitration";

/// Section 16 of title 9, which carries subsections, paragraphs and
/// subparagraphs — so a subtree has something beneath it.
const SECTION_16: &str = "uscode/title_9/chapter_1/section_16";

/// Section 1 of title 9, and a raw string prefix of [`SECTION_16`].
///
/// The segment trap: title 9 holds sections 1, 10, 11 and 16, so a prefix match
/// that ignores the `/` would answer for all four.
const SECTION_1: &str = "uscode/title_9/chapter_1/section_1";

/// Two works, one of them at two dates, built from the real release points.
///
/// Small on purpose: the two titles together are 400 KB of XML, and the scope
/// questions here need two works and two dates rather than a big corpus.
fn two_works() -> Dataset<InMemoryStorage> {
    let mut dataset = Dataset::new(DatasetMetadata::default());
    for (title, date) in [("usc09", EARLY), ("usc09", LATE), ("usc04", EARLY)] {
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

/// The same dataset in SQLite. `name` names this test's database file.
fn two_works_in_sqlite(name: &str) -> Dataset<SqliteStorage> {
    let path = format!("{}/search_scope_{name}.sqlite", env!("CARGO_TARGET_TMPDIR"));
    let _ = std::fs::remove_file(&path);
    two_works()
        .save_to_sqlite(&path)
        .expect("the fixture should save");
    Dataset::open_sqlite(&path).expect("the fixture should open")
}

/// Ask one question of both backends.
///
/// SQLite answers a locator from its element index and the in-memory store
/// answers it by walking the tree. Two backends that disagree about a scope are
/// two datasets wearing one name, so every scope question here is asked twice.
fn on_both_backends(name: &str, ask: impl Fn(&str, &dyn DocumentReader)) {
    ask("in-memory", &two_works());
    ask("sqlite", &two_works_in_sqlite(name));
}

/// A locator naming a work keeps that work's hits and drops every other one.
///
/// The measured defect: a three-release-point corpus returned one provision
/// three times, and there was no way to ask for one work.
#[test]
fn should_return_hits_from_one_work_only_when_a_locator_names_a_work() {
    on_both_backends("one_work", |backend, reader| {
        let everywhere = reader
            .search_text_in(SHARED_WORD, &Locator::new(), None)
            .expect("the search should run");
        let scoped = reader
            .search_text_in(SHARED_WORD, &Locator::new().in_work(ARBITRATION), None)
            .expect("the search should run");

        assert!(
            !scoped.rows.is_empty(),
            "{backend}: title 9 should use {SHARED_WORD:?}"
        );
        assert!(
            scoped
                .rows
                .iter()
                .all(|hit| hit.expression.work.as_str() == ARBITRATION),
            "{backend}: every hit should name title 9"
        );
        assert!(
            everywhere.total > scoped.total,
            "{backend}: naming one work should leave something out, got {} of {}",
            scoped.total,
            everywhere.total
        );
        assert!(
            everywhere
                .rows
                .iter()
                .any(|hit| hit.expression.work.as_str() == FLAG),
            "{backend}: the unscoped run should reach title 4 too"
        );
    });
}

/// A locator naming a window keeps the expressions published inside it.
///
/// Title 9 sits here at both release points and did not change between them, so
/// the unscoped run returns every one of its provisions twice. Asking for one
/// release point is how the reader stops repeating itself.
#[test]
fn should_return_hits_from_one_release_point_when_a_locator_names_a_window() {
    on_both_backends("one_window", |backend, reader| {
        let title_9 = Locator::new().in_work(ARBITRATION);
        let both_dates = reader
            .search_text_in(SHARED_WORD, &title_9, None)
            .expect("the search should run");
        let one_date = reader
            .search_text_in(SHARED_WORD, &title_9.clone().in_window(EARLY, EARLY), None)
            .expect("the search should run");

        assert!(
            !one_date.rows.is_empty(),
            "{backend}: title 9 is held at {EARLY}"
        );
        assert!(
            one_date.rows.iter().all(|hit| hit.expression.at == EARLY),
            "{backend}: every hit should come from {EARLY}"
        );
        assert_eq!(
            both_dates.total,
            one_date.total * 2,
            "{backend}: title 9 is held twice and did not change, \
             so one release point should be half the hits"
        );
        assert!(
            both_dates.rows.iter().any(|hit| hit.expression.at == LATE),
            "{backend}: the unwindowed run should reach {LATE} too"
        );
    });
}

/// A locator naming a path keeps that path and what sits beneath it, and nothing
/// that merely begins with the same characters.
#[test]
fn should_return_hits_from_one_subtree_only_when_a_locator_names_a_path() {
    on_both_backends("one_subtree", |backend, reader| {
        let subtree = reader
            .search_text_in(
                ARBITRATION_WORD,
                &Locator::new().at_path(SECTION_16, PathMatch::Subtree),
                None,
            )
            .expect("the search should run");

        assert!(
            !subtree.rows.is_empty(),
            "{backend}: section 16 should use {ARBITRATION_WORD:?}"
        );
        let beneath = format!("{SECTION_16}/");
        assert!(
            subtree
                .rows
                .iter()
                .all(|hit| hit.path == SECTION_16 || hit.path.starts_with(&beneath)),
            "{backend}: every hit should sit at or beneath section 16, got {:?}",
            paths(&subtree.rows)
        );
        assert!(
            subtree.rows.iter().any(|hit| hit.path != SECTION_16),
            "{backend}: the subtree should reach beneath the section itself"
        );

        // Section 1 is a raw prefix of section 16. A match that ignored the
        // separator would hand back sections 10, 11 and 16 as well.
        let sibling = reader
            .search_text_in(
                ARBITRATION_WORD,
                &Locator::new().at_path(SECTION_1, PathMatch::Subtree),
                None,
            )
            .expect("the search should run");
        assert!(
            sibling
                .rows
                .iter()
                .all(|hit| !hit.path.starts_with(SECTION_16)),
            "{backend}: section 1 should not answer for section 16, got {:?}",
            paths(&sibling.rows)
        );
    });
}

/// An exact path keeps the provision named and nothing beneath it.
///
/// Section 16 carries its matches in its paragraphs rather than at the section
/// itself, so asking for the section exactly answers with none of them. That is
/// the right answer, and it is why the subtree is the default.
#[test]
fn should_return_no_hit_beneath_a_path_when_a_locator_matches_it_exactly() {
    on_both_backends("exact_path", |backend, reader| {
        let exact = reader
            .search_text_in(
                ARBITRATION_WORD,
                &Locator::new().at_path(SECTION_16, PathMatch::Exact),
                None,
            )
            .expect("the search should run");

        assert!(
            exact.rows.iter().all(|hit| hit.path == SECTION_16),
            "{backend}: an exact path should reach nothing beneath it, got {:?}",
            paths(&exact.rows)
        );

        let subtree = reader
            .search_text_in(
                ARBITRATION_WORD,
                &Locator::new().at_path(SECTION_16, PathMatch::Subtree),
                None,
            )
            .expect("the search should run");
        assert!(
            subtree.total > exact.total,
            "{backend}: the subtree should hold more than the path alone, got {} and {}",
            subtree.total,
            exact.total
        );
    });
}

/// A limit bounds the rows and never the count.
///
/// This is what lets a caller say how many it did not show. Reporting the row
/// count as the answer is how a truncated listing comes to read as a complete
/// one (#220, #235).
#[test]
fn should_count_every_match_when_a_limit_returns_only_some_of_them() {
    on_both_backends("limited", |backend, reader| {
        let everything = reader
            .search_text_in(SHARED_WORD, &Locator::new(), None)
            .expect("the search should run");
        assert!(
            everything.total > 3,
            "{backend}: the fixture should hold more than three matches"
        );
        assert_eq!(
            everything.dropped(),
            0,
            "{backend}: an unlimited run drops nothing"
        );

        let limited = reader
            .search_text_in(SHARED_WORD, &Locator::new(), Some(3))
            .expect("the search should run");
        assert_eq!(
            limited.rows.len(),
            3,
            "{backend}: three rows were asked for"
        );
        assert_eq!(
            limited.total, everything.total,
            "{backend}: the total counts matches, not rows"
        );
        assert_eq!(
            limited.dropped(),
            everything.total - 3,
            "{backend}: the answer should say how many it left out"
        );
        assert_eq!(
            paths(&limited.rows),
            paths(&everything.rows[..3]),
            "{backend}: a limit takes the first rows, it does not reorder them"
        );
    });
}

/// The three flags compose: a work, a date and a path, all applied.
///
/// The measured defect: `search` took a dataset and a query, so reading one
/// section of one release point meant grepping 224 KB of output.
#[test]
fn should_apply_every_filter_when_search_is_given_a_work_a_date_and_a_path() {
    let dataset = cli_fixture();

    let everything = search_json(&["search", dataset, ARBITRATION_WORD, "--json"]);
    let scoped = search_json(&[
        "search",
        dataset,
        ARBITRATION_WORD,
        "--work",
        ARBITRATION,
        "--at",
        EARLY,
        "--path",
        SECTION_16,
        "--json",
    ]);

    assert!(!scoped.is_empty(), "section 16 should hold a hit");
    assert!(
        scoped.len() < everything.len(),
        "three filters should leave something out, got {} of {}",
        scoped.len(),
        everything.len()
    );
    for hit in &scoped {
        assert_eq!(hit["expression"]["work"], ARBITRATION);
        assert_eq!(hit["expression"]["at"], EARLY);
        let path = hit["path"].as_str().expect("a path");
        assert!(
            path == SECTION_16 || path.starts_with(&format!("{SECTION_16}/")),
            "{path} should sit at or beneath section 16"
        );
    }
}

/// Human output stops at a screenful and says what it left out.
///
/// The pattern `annotations` and `redesignation-report` already follow. A cap
/// that says nothing reads as "that is all there is", which is the
/// reader-that-lies defect of #220.
#[test]
fn should_say_how_many_it_did_not_show_when_a_limit_truncates_the_search() {
    let dataset = cli_fixture();

    // Read the total from `--json`, which carries every hit, rather than
    // counting the rows a limited run printed. Counting rows is the mistake a
    // limit makes possible.
    let total = search_json(&["search", dataset, ARBITRATION_WORD, "--json"]).len();
    assert!(
        total > DEFAULT_LIMIT,
        "the fixture should hold more than a screenful, got {total}"
    );

    let output = run(&["search", dataset, ARBITRATION_WORD]);
    assert!(output.status.success(), "search should exit zero");
    let said = String::from_utf8_lossy(&output.stdout);

    assert!(
        said.contains(&format!("{} more", total - DEFAULT_LIMIT)),
        "the run should name the {} hits it did not show, said:\n{said}",
        total - DEFAULT_LIMIT
    );
    assert!(
        said.contains(&format!("{total} match(es)")),
        "the run should still report every match it found, said:\n{said}"
    );

    // `--all` lifts the cap, so nothing is left out to report.
    let every = run(&["search", dataset, ARBITRATION_WORD, "--all"]);
    assert!(every.status.success(), "search --all should exit zero");
    let said = String::from_utf8_lossy(&every.stdout);
    assert!(
        !said.contains("not shown"),
        "--all should leave nothing out, said:\n{said}"
    );
}

/// A hit prints a window around the match, not the whole field.
///
/// One hit in the measured run was 10,228 characters — a whole section's text —
/// because `snippet` carried the field as recorded. A field is not a snippet.
#[test]
fn should_print_a_bounded_window_around_the_match_when_a_field_is_long() {
    let dataset = cli_fixture();

    let whole_field = search_json(&[
        "search",
        dataset,
        ARBITRATION_WORD,
        "--json",
        "--snippet",
        "0",
    ]);
    let longest = whole_field
        .iter()
        .map(|hit| hit["snippet"].as_str().expect("a snippet").chars().count())
        .max()
        .expect("the fixture should hold a hit");
    assert!(
        longest > 200,
        "the fixture should hold a field longer than a window, got {longest}"
    );

    let windowed = search_json(&["search", dataset, ARBITRATION_WORD, "--json"]);
    for hit in &windowed {
        let snippet = hit["snippet"].as_str().expect("a snippet");
        // The window plus the two elision marks it may carry.
        assert!(
            snippet.chars().count() <= 162,
            "a snippet should be bounded, got {} characters at {}",
            snippet.chars().count(),
            hit["path"]
        );
    }

    // The window is centred on the match, so the term is still in what it prints.
    assert!(
        windowed.iter().all(|hit| hit["snippet"]
            .as_str()
            .expect("a snippet")
            .to_lowercase()
            .contains(ARBITRATION_WORD)),
        "every window should hold the term it was cut around"
    );
}

/// A reader that closes the pipe early stops the run, it does not break it.
///
/// `search | head` exited 101 on the broken pipe where the same run without a
/// pipe exited 0, so an agent that piped the output read a crash rather than a
/// clean stop (#235).
#[test]
fn should_exit_zero_when_the_reader_closes_the_pipe_early() {
    let dataset = cli_fixture();

    // The reader takes one line and closes. A shell pipeline would report the
    // reader's own status, which is the mistake that hid this, so the run's
    // status is read from the process itself.
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args(["search", dataset, ARBITRATION_WORD, "--all"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("the binary should run");

    let mut first_line = String::new();
    {
        let stdout = child.stdout.take().expect("the run should write stdout");
        std::io::BufReader::new(stdout)
            .read_line(&mut first_line)
            .expect("the run should write a first line");
        // Dropping the reader closes the pipe while the run is still writing.
    }

    let finished = child
        .wait_with_output()
        .expect("the run should finish after the pipe closes");
    let said = String::from_utf8_lossy(&finished.stderr);
    assert!(
        !said.contains("panicked"),
        "a closed pipe should not panic, said:\n{said}"
    );
    assert_eq!(
        finished.status.code(),
        Some(0),
        "a closed pipe is a clean stop, not a failure; stderr said:\n{said}"
    );
}

/// The paths an answer returned, for a message that says what went wrong.
fn paths(rows: &[SearchResult]) -> Vec<&str> {
    rows.iter().map(|hit| hit.path.as_str()).collect()
}

/// One SQLite dataset on disk for the command-line tests, built once.
fn cli_fixture() -> &'static str {
    static FIXTURE: OnceLock<String> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let path = format!("{}/search_scope_cli.sqlite", env!("CARGO_TARGET_TMPDIR"));
        let _ = std::fs::remove_file(&path);
        two_works()
            .save_to_sqlite(&path)
            .expect("the fixture should save");
        path
    })
}

/// The command as a subprocess, the way an agent or a shell would run it.
fn run(args: &[&str]) -> std::process::Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args(args)
        .output()
        .expect("the binary should run")
}

/// The hits one `--json` run reported.
fn search_json(args: &[&str]) -> Vec<serde_json::Value> {
    let output = run(args);
    assert!(
        output.status.success(),
        "{args:?} should exit zero, said:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let hits: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("search --json should emit json");
    hits.get("hits")
        .and_then(|hits| hits.as_array())
        .cloned()
        .expect("search --json should carry a `hits` array")
}
