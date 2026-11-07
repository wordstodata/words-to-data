//! `words_to_data text` prints a Code provision as the law reads (#385).
//!
//! Before, the only way to read the words of a provision was `search <dataset>
//! "" --path <path> --at <date> --snippet 0 --all`. For 26 U.S.C. § 6041 that
//! gave one hit for each field, each with the full path, no numbers, and the
//! continuation of subsection (d) before its paragraphs.
//!
//! The fixture is real: title 26 at the printings 2025-07-18 and 2025-07-30.
//! Between the two, Pub. L. 119-21 (H.R. 1) changed § 6041(a) from "$600" in a
//! "taxable year" to "$2,000" in a "calendar year".

use std::process::{Command, Output};
use std::sync::OnceLock;

use words_to_data::dataset::{Dataset, DatasetMetadata};

const PRINTINGS: [(&str, &str); 2] = [
    ("tests/test_data/usc/2025-07-18/usc26.xml", "2025-07-18"),
    ("tests/test_data/usc/2025-07-30/usc26.xml", "2025-07-30"),
];

const SECTION_6041: &str =
    "uscode/title_26/subtitle_F/chapter_61/subchapter_A/part_III/subpart_B/section_6041";

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args(args)
        .output()
        .expect("the binary should run")
}

/// Title 26 at both printings, built once for the file.
fn dataset_path() -> &'static str {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| {
        let path = format!("{}/cli_text.sqlite", env!("CARGO_TARGET_TMPDIR"));
        let _ = std::fs::remove_file(&path);
        let mut dataset = Dataset::new(DatasetMetadata::default());
        for (file, date) in PRINTINGS {
            dataset
                .add_uslm_xml(file, date, None)
                .unwrap_or_else(|error| panic!("{file} should parse: {error}"));
        }
        dataset
            .save_to_sqlite(&path)
            .expect("the fixture should save");
        path
    })
}

fn stdout_of(output: &Output) -> String {
    assert!(
        output.status.success(),
        "the command should exit zero, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout.clone()).expect("the output should be UTF-8")
}

/// The line number of the first line that holds `words`, after line `after`.
fn line_of(text: &str, words: &str, after: usize) -> usize {
    text.lines()
        .enumerate()
        .skip(after)
        .find(|(_, line)| line.contains(words))
        .map(|(number, _)| number)
        .unwrap_or_else(|| panic!("no line after {after} holds {words:?} in:\n{text}"))
}

#[test]
fn should_print_each_subsection_with_its_number_and_heading_in_order_when_text_reads_a_section() {
    let text = stdout_of(&run(&[
        "text",
        dataset_path(),
        SECTION_6041,
        "--at",
        "2025-07-30",
    ]));

    let mut after = 0;
    for head in [
        "(a) Payments exceeding threshold",
        "(b) Collection of foreign items",
        "(c) Recipient to furnish name and address",
        "(d) Statements to be furnished to persons with respect to whom information is required",
        "(1)",
        "(4)",
        "qualified overtime compensation",
        // The continuation of (d) comes after (d)(4), as the law prints it.
        "The written statement required under the preceding sentence",
        "(e) Section does not apply to certain tips",
        "(f) Section does not apply to certain health arrangements",
        "(g) Nonqualified deferred compensation",
        "(h) Inflation adjustment",
    ] {
        after = line_of(&text, head, after) + 1;
    }
}

/// H.R. 1 raised the threshold of § 6041(a) and made its year a calendar year.
/// Each date reads its own expression.
#[test]
fn should_print_the_words_of_each_date_when_text_reads_one_section_at_two_dates() {
    let subsection_a = format!("{SECTION_6041}/subsection_a");
    let before = stdout_of(&run(&[
        "text",
        dataset_path(),
        &subsection_a,
        "--at",
        "2025-07-18",
    ]));
    let after = stdout_of(&run(&[
        "text",
        dataset_path(),
        &subsection_a,
        "--at",
        "2025-07-30",
    ]));

    assert!(
        before.contains("$600") && before.contains("taxable year"),
        "{before}"
    );
    assert!(
        after.contains("$2,000") && after.contains("calendar year"),
        "{after}"
    );
    assert!(!after.contains("$600 or more"), "{after}");
}

/// 26 U.S.C. 7701(b)(5)(A)(iv)(II), whose comma the OLRC footnotes. The
/// footnote is not the law (ADR 0015), so it prints under its provision, marked.
#[test]
fn should_print_a_footnote_under_its_provision_marked_when_the_code_prints_one() {
    let subclause = "uscode/title_26/subtitle_F/chapter_79/section_7701/subsection_b/paragraph_5/subparagraph_A/clause_iv/subclause_II";
    let text = stdout_of(&run(&[
        "text",
        dataset_path(),
        subclause,
        "--at",
        "2025-07-18",
    ]));

    let words = line_of(&text, "all of the net proceeds of which", 0);
    let footnote = line_of(
        &text,
        "[footnote] So in original. The comma probably should not appear.",
        words,
    );
    assert!(footnote > words, "{text}");
}

/// The words of a whole title are many thousand lines. The default stops at a
/// limit and says what it did not show, so a reader asks again for less.
#[test]
fn should_stop_at_the_line_limit_and_say_so_when_text_reads_a_whole_title() {
    let text = stdout_of(&run(&[
        "text",
        dataset_path(),
        "uscode/title_26",
        "--at",
        "2025-07-30",
    ]));

    let lines: Vec<&str> = text.lines().collect();
    let last = lines.last().expect("the answer should have lines");
    let more = last
        .strip_prefix("… ")
        .and_then(|rest| {
            rest.strip_suffix(" more node(s) not shown; give --depth or a deeper path")
        })
        .unwrap_or_else(|| panic!("the last line should say what is not shown: {last}"));
    assert!(more.parse::<usize>().expect("a count") > 0, "{last}");
    // The first line names the path; the last says what was left out.
    assert!(lines.len() <= 400 + 2, "{} lines", lines.len());
    assert!(lines.len() > 300, "{} lines", lines.len());
}

/// `--limit` changes the line limit. Five lines of § 6041 hold its heading and
/// the first two subsections; the rest is counted, not shown.
#[test]
fn should_stop_at_the_given_number_of_lines_when_text_is_given_a_limit() {
    let text = stdout_of(&run(&[
        "text",
        dataset_path(),
        SECTION_6041,
        "--at",
        "2025-07-30",
        "--limit",
        "5",
    ]));

    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 1 + 5 + 1, "{text}");
    assert!(
        lines[2].contains("(a) Payments exceeding threshold"),
        "{text}"
    );
    assert!(
        lines[4].contains("(b) Collection of foreign items"),
        "{text}"
    );
    assert!(!text.contains("(c) Recipient"), "{text}");
    assert!(
        lines[6].ends_with("more node(s) not shown; give --depth or a deeper path"),
        "{text}"
    );
}

/// `--json` gives the tree. `nodes_total` and `truncated` come before `node`,
/// so a reader that cuts the JSON still sees them (#447).
#[test]
fn should_give_the_tree_with_its_counts_first_when_text_is_asked_for_json() {
    let json = stdout_of(&run(&[
        "text",
        dataset_path(),
        SECTION_6041,
        "--at",
        "2025-07-30",
        "--json",
    ]));
    let answer: serde_json::Value = serde_json::from_str(&json).expect("the answer should be JSON");

    assert_eq!(answer["expression"], "uscode/title_26@2025-07-30");
    assert_eq!(answer["path"], SECTION_6041);
    assert_eq!(answer["at"], "2025-07-30");
    assert_eq!(answer["truncated"], false);
    let node = &answer["node"];
    assert_eq!(node["heading"], "Information at source");
    let subsection_d = &node["children"][3];
    assert_eq!(subsection_d["num"], "(d)");
    assert!(
        subsection_d["continuation"]
            .as_str()
            .is_some_and(|words| words.starts_with("The written statement required")),
        "{subsection_d}"
    );
    assert_eq!(subsection_d["children"][3]["num"], "(4)");
    // An empty field is left out: (d)(4) has no heading and no children.
    assert!(subsection_d["children"][3].get("heading").is_none());
    assert!(subsection_d["children"][3].get("children").is_none());
    // § 6041, (a) to (h), the four paragraphs of (d), and two each of (f),
    // (g) and (h).
    assert_eq!(answer["nodes_total"], 1 + 8 + 4 + 2 + 2 + 2);

    let order = [
        "expression",
        "path",
        "at",
        "nodes_total",
        "truncated",
        "node",
    ]
    .map(|key| {
        json.find(&format!("\"{key}\""))
            .expect("each key is in the answer")
    });
    assert!(order.is_sorted(), "{order:?}");
}

#[test]
fn should_say_truncated_when_text_is_asked_for_json_of_a_whole_title() {
    let json = stdout_of(&run(&[
        "text",
        dataset_path(),
        "uscode/title_26",
        "--at",
        "2025-07-30",
        "--json",
    ]));
    let answer: serde_json::Value = serde_json::from_str(&json).expect("the answer should be JSON");

    assert_eq!(answer["truncated"], true);
    assert!(
        answer["nodes_total"].as_u64().unwrap() > 1000,
        "{}",
        answer["nodes_total"]
    );
}

/// Chapter 79 of title 26 (Definitions) holds its sections with no level
/// between. `--depth 1` lists them, each with its number and heading only.
#[test]
fn should_list_the_sections_with_their_headings_only_when_text_reads_a_chapter_at_depth_one() {
    let text = stdout_of(&run(&[
        "text",
        dataset_path(),
        "uscode/title_26/subtitle_F/chapter_79",
        "--at",
        "2025-07-30",
        "--depth",
        "1",
    ]));

    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[1], "CHAPTER 79—DEFINITIONS", "{text}");
    // The OLRC prints a narrow no-break space after the section sign.
    assert_eq!(lines[2], "  §\u{202f}7701. Definitions", "{text}");
    for line in &lines[2..] {
        assert!(
            line.starts_with("  §\u{202f}77"),
            "only sections are listed: {line:?}\n{text}"
        );
    }
    assert!(lines.len() > 4, "{text}");
    assert!(!text.contains("not shown"), "{text}");
}

fn stderr_of_refusal(output: &Output) -> String {
    assert!(!output.status.success(), "the command should refuse");
    String::from_utf8(output.stderr.clone()).expect("stderr should be UTF-8")
}

/// Court opinions and bills are out of scope for now (#385).
#[test]
fn should_refuse_with_a_clear_message_when_text_is_given_a_path_outside_the_code() {
    for path in [
        "judicial/case_109019/opinion_109019",
        "publiclawdocument_119-21/title_VII/subtitle_A",
    ] {
        let refused =
            stderr_of_refusal(&run(&["text", dataset_path(), path, "--at", "2025-07-30"]));
        assert!(
            refused.contains("text supports Code paths only"),
            "{path}: {refused}"
        );
    }
}

/// A path no expression holds at the date gets the notice `search` gives for
/// the same path and date, so the two commands say one thing.
#[test]
fn should_refuse_as_search_does_when_no_expression_holds_the_path_at_the_date() {
    let guessed = format!("{SECTION_6041}/subsection_z");
    for (path, at) in [
        // A date the dataset does not hold.
        (SECTION_6041, "2025-08-14"),
        // A held date, and a path no expression holds.
        (guessed.as_str(), "2025-07-30"),
    ] {
        let refused = stderr_of_refusal(&run(&["text", dataset_path(), path, "--at", at]));
        let search = stderr_of_refusal(&run(&[
            "search",
            dataset_path(),
            "",
            "--path",
            path,
            "--exact",
            "--at",
            at,
        ]));
        assert_eq!(refused, search, "{path} at {at}");
    }
}
