//! `settle` on a citation link, and the record id a verdict prints (#393).
//!
//! `settle --explain` said "This dataset holds the provision at neither end"
//! for every `judicial.cites` link, also where the dataset held the cited
//! provision. A reviewer starts each review with `--explain`, so the reviewer got
//! a false statement and no evidence. And `settle --verdict` printed no id for
//! the record it wrote, where `link-amendment` prints "record <id>".
//!
//! The fixture is title 26 as it read on 2025-07-30, and the real CourtListener
//! record of *Boeing* (case 122262, added by opinion 9434365), committed
//! under `tests/test_data/courtlistener` and added by the real binary with
//! `--offline`. Its writings cite 26 U.S.C. § 174(a)(1).

use std::process::{Command, Output};
use std::sync::OnceLock;

use words_to_data::dataset::{Dataset, DatasetMetadata};
use words_to_data::link::{Link, LinkKind};
use words_to_data::storage::{InMemoryStorage, LinkReader};

const RELEASE: &str = "2025-07-30";

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args(args)
        .output()
        .expect("the binary should run")
}

fn stdout_of(output: &Output) -> String {
    assert!(
        output.status.success(),
        "the command should exit zero, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).to_string()
}

/// Title 26 and one opinion, in one SQLite file that no test writes.
fn boeing() -> &'static str {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| {
        let path = format!("{}/settle_citation.sqlite", env!("CARGO_TARGET_TMPDIR"));
        let _ = std::fs::remove_file(&path);
        let mut code: Dataset<InMemoryStorage> = Dataset::new(DatasetMetadata {
            name: "Title 26 and Boeing".to_string(),
            ..Default::default()
        });
        code.add_uslm_xml(
            &format!("tests/test_data/usc/{RELEASE}/usc26.xml"),
            RELEASE,
            None,
        )
        .expect("title 26 should parse");
        code.save_to_sqlite(&path).expect("the fixture should save");
        stdout_of(&run(&[
            "add-opinions",
            &path,
            "--offline",
            "--cache-dir",
            "tests/test_data",
            "--opinions",
            "9434365",
        ]));
        path
    })
}

/// The citation link to § 174(a)(1), which the fixture's opinion writes.
fn citation_of_174_a_1(path: &str) -> Link {
    Dataset::open_sqlite(path)
        .expect("the fixture should open")
        .links_by_kind(LinkKind::CITES)
        .expect("the links should read")
        .into_iter()
        .find(|link| {
            link.object
                .name()
                .ends_with("/section_174/subsection_a/paragraph_1")
        })
        .expect("the opinion cites § 174(a)(1)")
}

#[test]
fn should_show_the_citing_words_and_the_cited_provision_when_explain_reads_a_citation_link() {
    let link = citation_of_174_a_1(boeing());
    let cited = link.object.name();
    let payload = &link
        .payload
        .as_ref()
        .expect("a citation has a payload")
        .value;
    let citing = payload["occurrences"][0]["words"]
        .as_str()
        .expect("the place records the words the rule matched");
    let printing = payload["resolved_in"]
        .as_str()
        .expect("the link records the printing it resolved in");

    // The words of the cited provision, as the dataset holds them at that
    // printing.
    let held = Dataset::open_sqlite(boeing())
        .expect("the fixture should open")
        .find_nodes(&cited)
        .expect("the nodes should read")
        .into_iter()
        .find(|(id, _)| id.at == printing)
        .map(|(_, node)| node)
        .expect("the dataset holds the cited provision at that printing");
    let first_words = &words_to_data::inspect::provision_words(&held)[0].text;
    let opening: String = first_words.chars().take(40).collect();

    let printed = stdout_of(&run(&[
        "settle",
        boeing(),
        "--link",
        &link.id()[..12],
        "--explain",
    ]));

    assert!(
        !printed.contains("neither end"),
        "the dataset holds the cited provision, so it must not say otherwise, got:\n{printed}"
    );
    assert!(
        printed.contains(citing),
        "it shows the words the court wrote, {citing:?}, got:\n{printed}"
    );
    assert!(
        printed.contains(&format!("uscode/title_26@{printing}")),
        "it names the printing the link resolved in, got:\n{printed}"
    );
    assert!(
        printed.contains(&opening),
        "it shows the cited provision's words, {opening:?}, got:\n{printed}"
    );
}

#[test]
fn should_print_the_record_id_and_the_id_should_find_the_review_when_a_verdict_is_recorded() {
    // A copy, so the fixture the other test reads is never written.
    let copy = format!(
        "{}/settle_citation_verdict.sqlite",
        env!("CARGO_TARGET_TMPDIR")
    );
    std::fs::copy(boeing(), &copy).expect("the fixture should copy");
    let link = citation_of_174_a_1(&copy);

    let printed = stdout_of(&run(&[
        "settle",
        &copy,
        "--link",
        &link.id()[..12],
        "--verdict",
        "confirmed",
        "--reviewer",
        "human:test",
        "--reason",
        "The paragraph writes § 174(a)(1), and the link names it.",
    ]));

    let id = printed
        .split("record ")
        .nth(1)
        .and_then(|rest| rest.split_whitespace().next())
        .unwrap_or_else(|| panic!("the run prints `record <id>`, got:\n{printed}"));
    let reviews = Dataset::open_sqlite(&copy)
        .expect("the written dataset should open")
        .reviews_of(&link.id())
        .expect("the reviews should read");
    assert!(
        reviews.iter().any(|review| review.id().starts_with(id)),
        "the printed id {id} names the review the run recorded, got {reviews:#?}"
    );
}
