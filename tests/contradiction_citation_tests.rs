//! `contradictions` on citation links: a passage that cites several provisions
//! is not a disagreement (#397).
//!
//! Paragraph 88 of *Patricia A. Torres* (T.C. 2025) cites 15 U.S.C. § 78u-6 in
//! full, and § 199A and its parts as bare references under the opinion's
//! default code. Two rules read those citations, so the paragraph has links from
//! two makers that name different provisions. Before #397 `contradictions`
//! reported that as a disagreement. A court that cites two sections in one
//! paragraph says two true things.
//!
//! The fixture is titles 15 and 26 as they read on 2025-07-30, and the real
//! CourtListener record of Torres committed under `tests/test_data/courtlistener`,
//! added by the real binary with `--offline`.

use std::process::{Command, Output};
use std::sync::OnceLock;

use words_to_data::dataset::{Dataset, DatasetMetadata};
use words_to_data::link::LinkKind;
use words_to_data::storage::{InMemoryStorage, LinkReader};

const RELEASE: &str = "2025-07-30";

/// The paragraph of the Court's opinion in Torres that cites two sections.
const PARAGRAPH_88: &str = "judicial/case_10670048/opinion_11136635-1/paragraph_88";

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

/// Titles 15 and 26 and Torres, in one SQLite file. No test here writes.
fn torres() -> &'static str {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| {
        let path = format!(
            "{}/contradiction_citations.sqlite",
            env!("CARGO_TARGET_TMPDIR")
        );
        let _ = std::fs::remove_file(&path);
        let mut code: Dataset<InMemoryStorage> = Dataset::new(DatasetMetadata {
            name: "Titles 15 and 26 and Torres".to_string(),
            ..Default::default()
        });
        for title in ["usc15.xml", "usc26.xml"] {
            code.add_uslm_xml(
                &format!("tests/test_data/usc/{RELEASE}/{title}"),
                RELEASE,
                None,
            )
            .expect("the title should parse");
        }
        code.save_to_sqlite(&path).expect("the fixture should save");
        stdout_of(&run(&[
            "add-opinions",
            &path,
            "--offline",
            "--cache-dir",
            "tests/test_data",
            "--opinions",
            "11136635",
        ]));
        path
    })
}

/// The objects of the citation links whose subject is `passage`.
fn cited_by(passage: &str) -> Vec<String> {
    Dataset::open_sqlite(torres())
        .expect("the fixture should open")
        .links_by_kind(LinkKind::CITES)
        .expect("the links should read")
        .into_iter()
        .filter(|link| link.subject.name() == passage)
        .map(|link| link.object.name())
        .collect()
}

#[test]
fn should_report_no_disagreement_when_a_paragraph_cites_two_sections() {
    // The fixture holds the case the issue names: one paragraph, links to two
    // sections in two titles.
    let cited = cited_by(PARAGRAPH_88);
    assert!(
        cited
            .iter()
            .any(|path| path.starts_with("uscode/title_15/"))
            && cited
                .iter()
                .any(|path| path.starts_with("uscode/title_26/")),
        "paragraph 88 cites a section of title 15 and one of title 26, got {cited:?}"
    );

    let report: serde_json::Value =
        serde_json::from_str(&stdout_of(&run(&["contradictions", torres(), "--json"])))
            .expect("the command should emit json");

    let disagreements = report["disagreement"]
        .as_array()
        .expect("a list of disagreements");
    assert!(
        disagreements
            .iter()
            .all(|group| group["subject"] != PARAGRAPH_88),
        "a paragraph that cites two sections is not a disagreement, got {disagreements:#?}"
    );
}
