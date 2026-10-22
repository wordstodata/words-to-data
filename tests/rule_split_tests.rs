//! A record that holds every writing of a case in one text, split into its
//! writings by rule, and a `plain_text` writing read into paragraphs by rule
//! (`docs/adr/0014-a-case-is-a-work-and-a-citation-runs-paragraph-to-subdivision.md`,
//! decisions 2 and 3; #315).
//!
//! A writing that a rule finds is `MachineSuggested`, and its provenance names
//! the rule. A record the rule cannot split stays one writing, and its node
//! says so. Every record here is a real CourtListener response committed under
//! `tests/test_data/courtlistener`.

use words_to_data::courtlistener::{ClusterRecord, OpinionRecord, case_expression};
use words_to_data::dataset::Expression;
use words_to_data::document::DocumentNode;
use words_to_data::judicial::OpinionFacts;
use words_to_data::link::VerificationState;

const CACHE: &str = "tests/test_data/courtlistener";

/// One committed record, as the API returned it.
fn read(name: &str) -> String {
    std::fs::read_to_string(format!("{CACHE}/{name}"))
        .unwrap_or_else(|error| panic!("{name} should be committed: {error}"))
}

fn opinion(id: u64) -> OpinionRecord {
    OpinionRecord::from_json(&read(&format!("opinion_{id}.json"))).expect("the opinion should read")
}

fn cluster(id: u64) -> ClusterRecord {
    ClusterRecord::from_json(&read(&format!("cluster_{id}.json"))).expect("the cluster should read")
}

/// One case built from its cluster and the records given.
fn case_from(cluster_id: u64, opinions: &[OpinionRecord]) -> Expression {
    case_expression(&cluster(cluster_id), opinions)
        .expect("the case should build")
        .expression
}

/// The writings of a case, in order.
fn writings(case: &Expression) -> Vec<&DocumentNode> {
    case.root
        .children
        .iter()
        .filter(|child| child.data.node_type.as_str() == "judicial.opinion")
        .collect()
}

fn facts(writing: &DocumentNode) -> OpinionFacts {
    OpinionFacts::of(&writing.data).expect("a writing carries judicial facts")
}

/// What a node's provenance says it was based on.
fn reasoning(node: &DocumentNode) -> String {
    node.data
        .provenance
        .as_ref()
        .and_then(|provenance| provenance.evidence.as_ref())
        .and_then(|evidence| evidence.reasoning.clone())
        .unwrap_or_else(|| panic!("{} should say what it was based on", node.data.path))
}

// --- Combined markup: "JUSTICE X, dissenting" lines ------------------------

/// CourtListener holds *Obergefell* as one combined record. Harvard's text of
/// it is one `<opinion type="majority">`, and the four dissents are inside it,
/// each opened by a line such as "Chief Justice ROBERTS, with whom Justice
/// SCALIA and Justice THOMAS join, dissenting."
#[test]
fn should_split_obergefell_into_its_five_writings_when_the_combined_record_has_justice_lines() {
    let case = case_from(2812209, &[opinion(2812209)]);

    let found: Vec<(String, Option<String>, Option<String>)> = writings(&case)
        .into_iter()
        .map(|writing| {
            let facts = facts(writing);
            (
                writing.data.path.to_string(),
                facts.opinion_type,
                facts.author,
            )
        })
        .collect();
    let expected = [
        ("1", "020lead", "Kennedy"),
        ("2", "040dissent", "Roberts"),
        ("3", "040dissent", "Scalia"),
        ("4", "040dissent", "Thomas"),
        ("5", "040dissent", "Alito"),
    ]
    .map(|(n, kind, author)| {
        (
            format!("judicial/case_2812209/opinion_2812209-{n}"),
            Some(kind.to_string()),
            Some(author.to_string()),
        )
    });
    assert_eq!(found, expected);
}

/// A writing a rule found is a machine's reading, so it is `MachineSuggested`,
/// and its provenance names the rule and the line it found. Its paragraphs say
/// the same.
#[test]
fn should_name_the_rule_and_suggest_the_writing_when_a_rule_splits_a_combined_record() {
    let case = case_from(2812209, &[opinion(2812209)]);
    let dissent = writings(&case)[1];

    let provenance = dissent.data.provenance.as_ref().expect("provenance");
    assert_eq!(provenance.verification, VerificationState::MachineSuggested);
    assert_eq!(
        provenance.source, "courtlistener:opinion/2812209:xml_harvard",
        "the record and the field the words came from"
    );
    assert_eq!(
        reasoning(dissent),
        "justice-headings@1: the writing opens with \"Chief Justice ROBERTS, with whom Justice \
         SCALIA and Justice THOMAS join, dissenting.\""
    );
    let paragraph = &dissent.children[1];
    assert_eq!(reasoning(paragraph), reasoning(dissent));
    assert!(
        paragraph
            .data
            .content
            .as_deref()
            .is_some_and(|text| text.starts_with("Petitioners make strong arguments")),
        "the dissent's own words, got {:?}",
        paragraph.data.content
    );
}

/// The writings of one case number their footnotes each from 1, and Harvard's
/// text prints every footnote of the case at its end. Each goes to the writing
/// that refers to it: in *Obergefell*, 26 to Justice Scalia's dissent, 8 to
/// Justice Thomas's and 2 to Justice Alito's. So no two nodes share a path.
#[test]
fn should_give_each_footnote_to_the_writing_that_refers_to_it_when_a_rule_splits_a_record() {
    let case = case_from(2812209, &[opinion(2812209)]);

    let footnotes: Vec<usize> = writings(&case)
        .iter()
        .map(|writing| {
            writing
                .children
                .iter()
                .filter(|child| child.data.node_type.as_str() == "judicial.footnote")
                .count()
        })
        .collect();
    assert_eq!(footnotes, [0, 0, 26, 8, 2]);

    let mut paths = Vec::new();
    every_path(&case.root, &mut paths);
    let count = paths.len();
    paths.sort();
    paths.dedup();
    assert_eq!(paths.len(), count, "every path names one node");
}

fn every_path(node: &DocumentNode, into: &mut Vec<String>) {
    into.push(node.data.path.to_string());
    for child in &node.children {
        every_path(child, into);
    }
}

/// *Boeing*'s combined record is Lawbox's HTML of the whole case. CourtListener
/// also holds the two writings as split records from Harvard, so the rule can
/// be checked against them: the writing the rule finds after "JUSTICE THOMAS,
/// with whom JUSTICE SCALIA joins, dissenting." holds the words of the split
/// dissent, and the Court's opinion holds none of them.
#[test]
fn should_split_boeing_like_its_split_records_when_only_the_lawbox_record_is_given() {
    let case = case_from(122262, &[opinion(122262)]);
    let found = writings(&case);

    let kinds: Vec<(Option<String>, Option<String>)> = found
        .iter()
        .map(|writing| {
            let facts = facts(writing);
            (facts.opinion_type, facts.author)
        })
        .collect();
    assert_eq!(
        kinds,
        [
            (Some("020lead".into()), Some("Stevens".into())),
            (Some("040dissent".into()), Some("Thomas".into()))
        ]
    );

    let text = |writing: &DocumentNode| writing.data.content.as_deref().unwrap_or("").to_string();
    let (majority, dissent) = (text(found[0]), text(found[1]));
    assert!(dissent.starts_with("JUSTICE THOMAS, with whom JUSTICE SCALIA joins, dissenting."));
    for words in [
        "Before placing its hand in the taxpayer's pocket",
        "I respectfully dissent",
    ] {
        assert!(dissent.contains(words), "the dissent holds {words:?}");
        assert!(!majority.contains(words), "the Court's opinion does not");
    }
    assert!(majority.contains("This suit concerns tax provisions enacted by Congress in 1971"));
}

/// *Snow*'s combined record holds one writing: Justice Douglas's opinion for
/// the Court. The rule finds no second writing, so the record stays one
/// writing at its own path, and its node says so.
#[test]
fn should_keep_one_writing_and_say_so_when_the_rule_finds_no_second_writing() {
    let case = case_from(109019, &[opinion(109019)]);
    let found = writings(&case);

    assert_eq!(found.len(), 1);
    assert_eq!(
        found[0].data.path.as_ref(),
        "judicial/case_109019/opinion_109019"
    );
    assert_eq!(facts(found[0]).opinion_type.as_deref(), Some("010combined"));
    assert_eq!(
        reasoning(found[0]),
        "justice-headings@1: no rule found a second writing in this combined record, so it is \
         one writing"
    );
}

// --- plain_text: paragraphs by rule ----------------------------------------

/// A record as it would read with only its `plain_text`: the court's own PDF
/// text, as a text layer. The other fields are dropped, and nothing is added.
fn plain_text_only(mut record: OpinionRecord) -> OpinionRecord {
    record.xml_harvard = None;
    record.html_columbia = None;
    record.html_anon_2020 = None;
    record.html_lawbox = None;
    record.html = None;
    record
}

/// The paragraphs and the footnotes under a writing, in order, as words.
fn passages(writing: &DocumentNode) -> (Vec<String>, Vec<(String, String)>) {
    let mut paragraphs = Vec::new();
    let mut footnotes = Vec::new();
    let mut every = Vec::new();
    collect(writing, &mut every);
    for node in every {
        let words = node.data.content.as_deref().unwrap_or("").to_string();
        match node.data.node_type.as_str() {
            "judicial.paragraph" => paragraphs.push(words),
            "judicial.footnote" => {
                let label = node
                    .data
                    .path
                    .rsplit_once("footnote_")
                    .unwrap()
                    .1
                    .to_string();
                footnotes.push((label, words));
            }
            _ => {}
        }
    }
    (paragraphs, footnotes)
}

fn collect<'a>(node: &'a DocumentNode, into: &mut Vec<&'a DocumentNode>) {
    into.push(node);
    for child in &node.children {
        collect(child, into);
    }
}

/// The first words of a passage, in lower case, without punctuation or the
/// star page marks Harvard prints: what two readings of one paragraph share
/// although one is OCR and the other a text layer.
fn opening(words: &str) -> String {
    let letters: String = words
        .split_whitespace()
        .filter(|word| !word.starts_with('*'))
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .filter(|c| !matches!(c, '\'' | '’'))
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .flat_map(char::to_lowercase)
        .collect();
    letters
        .split_whitespace()
        .take(4)
        .collect::<Vec<_>>()
        .join(" ")
}

/// *Harris* (5th Cir. 1994) as its `plain_text` prints it is double spaced, so
/// a blank line falls between every two lines: 374 blank-line blocks for the
/// 36 paragraphs and 12 footnotes Harvard's XML of the same opinion marks. The
/// rule joins the lines of each paragraph, takes off the page numbers, and
/// takes the footnotes out of the paragraphs they interrupt. Harvard's
/// paragraphs are the measure: nearly every one of them opens a paragraph of
/// the plain text, and from the first of them on the plain text has few more.
#[test]
fn should_give_harris_the_paragraphs_harvard_marks_when_its_text_is_read_from_plain_text() {
    let harvard = case_from(6248, &[opinion(6248)]);
    let (harvard_paragraphs, harvard_footnotes) = passages(writings(&harvard)[0]);

    let plain = case_from(6248, &[plain_text_only(opinion(6248))]);
    let writing = writings(&plain)[0];
    let (paragraphs, footnotes) = passages(writing);

    let openings: Vec<String> = paragraphs.iter().map(|words| opening(words)).collect();
    let missed: Vec<&String> = harvard_paragraphs
        .iter()
        .filter(|words| !openings.contains(&opening(words)))
        .collect();
    assert!(
        missed.len() * 10 <= harvard_paragraphs.len(),
        "{} of Harvard's {} paragraphs open no paragraph of the plain text: {missed:#?}\n\
         plain text paragraphs: {openings:#?}",
        missed.len(),
        harvard_paragraphs.len()
    );
    // The plain text opens with the caption, which Harvard keeps in the
    // cluster's head matter. So the count starts where Harvard's does.
    let start = openings
        .iter()
        .position(|words| *words == opening(&harvard_paragraphs[0]))
        .expect("the plain text holds the paragraph Harvard opens with");
    let body = paragraphs.len() - start;
    assert!(
        body * 10 <= harvard_paragraphs.len() * 11,
        "{body} paragraphs for Harvard's {}: {openings:#?}",
        harvard_paragraphs.len()
    );

    let labels: Vec<&str> = footnotes.iter().map(|(label, _)| label.as_str()).collect();
    let harvard_labels: Vec<&str> = harvard_footnotes
        .iter()
        .map(|(label, _)| label.as_str())
        .collect();
    assert_eq!(labels, harvard_labels, "the same footnotes");

    for words in &paragraphs {
        assert!(
            !words.trim().chars().all(|c| c.is_ascii_digit()),
            "a page number is no paragraph"
        );
    }
    let first = paragraphs
        .iter()
        .find(|words| words.starts_with("The taxpayer appeals"))
        .expect("the opinion's first paragraph");
    assert!(
        first.starts_with(
            "The taxpayer appeals from the Tax Court's denial of his deduction of his \
             distributive share of payments made by his partnership"
        ),
        "the lines of a paragraph are joined, one space between two words, got {first:?}"
    );
}

// --- plain_text: Supreme Court running heads --------------------------------

/// The writings of a case, each as (path, kind, author).
fn kinds(case: &Expression) -> Vec<(String, Option<String>, Option<String>)> {
    writings(case)
        .into_iter()
        .map(|writing| {
            let facts = facts(writing);
            (
                writing.data.path.to_string(),
                facts.opinion_type,
                facts.author,
            )
        })
        .collect()
}

/// *Obergefell*'s `plain_text` is the slip opinion the Court published. Each
/// page carries a running head that names the writing it belongs to:
/// "Syllabus", "Opinion of the Court", "ROBERTS, C. J., dissenting". The rule
/// splits the record where the running head changes, and takes the running
/// heads and page numbers out of the words.
#[test]
fn should_split_obergefell_by_its_running_heads_when_its_text_is_read_from_plain_text() {
    let case = case_from(2812209, &[plain_text_only(opinion(2812209))]);

    let expected = [
        ("1", "020lead", "Kennedy"),
        ("2", "040dissent", "Roberts"),
        ("3", "040dissent", "Scalia"),
        ("4", "040dissent", "Thomas"),
        ("5", "040dissent", "Alito"),
    ]
    .map(|(n, kind, author)| {
        (
            format!("judicial/case_2812209/opinion_2812209-{n}"),
            Some(kind.to_string()),
            Some(author.to_string()),
        )
    });
    assert_eq!(kinds(&case), expected);

    let dissent = writings(&case)[1];
    assert_eq!(
        dissent.data.provenance.as_ref().map(|p| p.verification),
        Some(VerificationState::MachineSuggested)
    );
    assert_eq!(
        reasoning(dissent),
        "running-heads@1: the writing's pages carry the running head \"ROBERTS, C. J., dissenting\""
    );

    let (paragraphs, _) = passages(dissent);
    assert!(
        paragraphs.iter().any(|words| words
            == "CHIEF JUSTICE ROBERTS, with whom JUSTICE SCALIA and JUSTICE THOMAS join, dissenting."),
        "the line that opens the dissent is its own paragraph"
    );
    for writing in writings(&case) {
        let (paragraphs, _) = passages(writing);
        for words in paragraphs {
            assert!(
                !words.contains("Cite as: 576 U. S.") && !words.contains("OBERGEFELL v. HODGES"),
                "a running head is no part of a paragraph, got {words:?}"
            );
        }
    }
}

/// The slip opinion opens with a syllabus that says it "constitutes no part of
/// the opinion of the Court". It is its own child of the case, as the head
/// matter is, and no writing holds its words.
#[test]
fn should_store_the_syllabus_as_its_own_child_of_the_case_when_the_slip_opinion_prints_one() {
    let case = case_from(2812209, &[plain_text_only(opinion(2812209))]);

    let syllabus = case
        .root
        .find("judicial/case_2812209/syllabus")
        .expect("the syllabus is a child of the case");
    assert_eq!(syllabus.data.node_type.as_str(), "judicial.syllabus");
    let words = syllabus.data.content.as_deref().unwrap_or("");
    assert!(words.contains("The syllabus constitutes no part of the opinion of the Court"));
    assert!(words.contains("Held: The Fourteenth Amendment requires a State to license"));
    assert_eq!(
        reasoning(syllabus),
        "running-heads@1: the pages carry the running head \"Syllabus\""
    );

    for writing in writings(&case) {
        let (paragraphs, _) = passages(writing);
        assert!(
            !paragraphs
                .iter()
                .any(|words| words.contains("Held: The Fourteenth Amendment")),
            "no writing holds the syllabus"
        );
    }
}

/// Harvard's XML of *Obergefell* is split by its Justice lines, and the slip
/// opinion by its running heads. Each writing of one is the same writing of
/// the other, and Harvard's paragraphs are the measure for the paragraphs the
/// rule reads out of the slip opinion's lines.
#[test]
fn should_give_each_writing_of_obergefell_the_paragraphs_harvard_marks_when_read_from_plain_text() {
    let harvard = case_from(2812209, &[opinion(2812209)]);
    let plain = case_from(2812209, &[plain_text_only(opinion(2812209))]);

    for (harvard_writing, plain_writing) in writings(&harvard).into_iter().zip(writings(&plain)) {
        let (expected, _) = passages(harvard_writing);
        let (paragraphs, _) = passages(plain_writing);
        let openings: Vec<String> = paragraphs.iter().map(|words| opening(words)).collect();
        let missed: Vec<String> = expected
            .iter()
            .map(|words| opening(words))
            .filter(|words| !openings.contains(words))
            .collect();
        assert!(
            missed.len() * 10 <= expected.len(),
            "{}: {} of Harvard's {} paragraphs open no paragraph of the plain text: {missed:#?}",
            plain_writing.data.path,
            missed.len(),
            expected.len()
        );
        let start = openings
            .iter()
            .position(|words| *words == opening(&expected[0]))
            .expect("the plain text holds the paragraph Harvard opens with");
        assert!(
            (paragraphs.len() - start) * 10 <= expected.len() * 11,
            "{}: {} paragraphs for Harvard's {}",
            plain_writing.data.path,
            paragraphs.len() - start,
            expected.len()
        );
    }
}

// --- plain_text: a court staff summary --------------------------------------

/// The Ninth Circuit opens a published opinion with a "SUMMARY" that court
/// staff write. Its footnote says: "This summary constitutes no part of the
/// opinion of the court. It has been prepared by court staff for the
/// convenience of the reader." *United States v. Anderson* (9th Cir. 2026) is
/// held only as the court's `plain_text`. The summary is its own child of the
/// case, as the syllabus of a slip opinion is, and no writing holds its words.
#[test]
fn should_store_the_staff_summary_as_its_own_child_of_the_case_when_a_ninth_circuit_opinion_opens_with_one()
 {
    let case = case_from(11038158, &[opinion(11505854)]);

    let summary = case
        .root
        .find("judicial/case_11038158/summary")
        .expect("the staff summary is a child of the case");
    assert_eq!(summary.data.node_type.as_str(), "judicial.summary");
    assert_eq!(
        summary.data.provenance.as_ref().map(|p| p.verification),
        Some(VerificationState::MachineSuggested)
    );
    assert_eq!(
        reasoning(summary),
        "staff-summary@1: the summary says \"This summary constitutes no part of the opinion \
         of the court.\""
    );
    let words = summary.data.content.as_deref().unwrap_or("");
    for said in [
        "SUMMARY",
        "28 U.S.C. § 2255",
        "The panel vacated the district court’s order denying",
        "remanded for an evidentiary hearing.",
        "This summary constitutes no part of the opinion of the court.",
    ] {
        assert!(
            words.contains(said),
            "the summary holds {said:?}, got {words:?}"
        );
    }
    assert!(
        !words.contains("COUNSEL"),
        "the summary ends where the next heading starts"
    );

    let found = writings(&case);
    assert_eq!(found.len(), 1, "the summary is no writing");
    let (paragraphs, footnotes) = passages(found[0]);
    for words in paragraphs
        .iter()
        .chain(footnotes.iter().map(|(_, words)| words))
    {
        assert!(
            !words.contains("The panel vacated") && !words.contains("constitutes no part"),
            "no writing holds the summary, got {words:?}"
        );
    }
    assert!(
        paragraphs
            .iter()
            .any(|words| words.starts_with("Patrick Anderson moved to vacate")),
        "the writing holds the opinion of the court"
    );
    assert!(
        paragraphs
            .iter()
            .any(|words| words.contains("Christopher A. Booker")),
        "the names of counsel stay with the writing, as other captions do"
    );
}
