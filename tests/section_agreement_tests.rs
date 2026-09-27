//! Does an amendment's own text name the section its link points into?
//!
//! A cheap deterministic check, and a queue rather than a verdict. An amendment
//! that names § 263A and whose link points inside § 263 is suspect on its face,
//! and nothing reported it before (#239).
//!
//! **Three outcomes, not two.** A citation form the extractor declined is its
//! own case and never a disagreement: reporting it as one would manufacture a
//! false fault out of an extractor limitation (#140).
//!
//! Every link here is real. They are the committed output of one matching run
//! over the real corpus, so no model runs and nothing is invented. Only the
//! links are loaded: the check reads an amendment's words and a path, so the two
//! release points of title 26 are 112 MB of XML it never opens.

use std::process::Command;

use words_to_data::annotation::ChangeAnnotation;
use words_to_data::congress::BillDownload;
use words_to_data::dataset::{Dataset, DatasetMetadata, ExpressionId, Format, WorkId};
use words_to_data::document::DocumentNode;
use words_to_data::legislature::section_agreement::{self, Naming, Outcome};
use words_to_data::link::{Link, Named};
use words_to_data::query::LinkQuery;
use words_to_data::storage::{InMemoryStorage, LinkReader};

/// The committed output of one real matching run.
const REAL_ANNOTATIONS: &str = "tests/test_data/processed/annotations.json";
const BEFORE: &str = "2025-07-18";
const AFTER: &str = "2025-07-30";

/// § 263(a)(1)(B), which the bill reached by an amendment to § 263A(c)(2).
///
/// The disagreement this check exists to find: the amendment names 263A and the
/// link points inside 263, and the two are different sections of the Code.
const SECTION_263_A_1_B: &str = "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_IX/section_263/subsection_a/paragraph_1/subparagraph_B";

/// Every amendment link the committed run recorded, over one window.
///
/// Unfiltered by title: the check reads a path and an amendment's words, so a
/// link into title 7 or title 20 is as checkable as one into title 26. The
/// annotations name thirteen titles and the third outcome only happens outside
/// title 26, so filtering would hide it.
fn amendment_links() -> Dataset<InMemoryStorage> {
    let json = std::fs::read_to_string(REAL_ANNOTATIONS).expect("the fixture should be readable");
    let annotations: Vec<ChangeAnnotation> =
        serde_json::from_str(&json).expect("the fixture should parse as annotations");

    let mut dataset = Dataset::new(DatasetMetadata::default());
    for annotation in &annotations {
        // A link's subject is a change to one work, so each annotation is
        // recorded against the work its own paths sit in.
        for path in &annotation.paths {
            let Some(work) = work_of(path) else { continue };
            let from = ExpressionId::new(work.clone(), BEFORE);
            let to = ExpressionId::new(work, AFTER);
            let mut one = annotation.clone();
            one.paths = vec![path.clone()];
            for link in Link::from_annotation(&one, &from, &to) {
                dataset.add_link(link).expect("the link should be added");
            }
        }
    }
    dataset
}

/// `uscode/title_26` out of a path inside title 26.
fn work_of(path: &str) -> Option<WorkId> {
    let mut segments = path.split('/');
    let container = segments.next()?;
    let title = segments.next()?;
    Some(WorkId::new(format!("{container}/{title}")))
}

/// The disagreement the ticket names: § 263A cited, § 263 pointed at.
#[test]
fn should_report_the_link_when_the_amendment_names_a_different_section_from_its_path() {
    let dataset = amendment_links();
    let report = section_agreement::section_agreement(&dataset, &LinkQuery::new())
        .expect("the check should read the dataset");

    let row = report
        .rows
        .iter()
        .find(|row| row.path == SECTION_263_A_1_B)
        .expect("the run recorded a link at § 263(a)(1)(B)");

    assert_eq!(row.outcome, Outcome::Disagrees);
    assert_eq!(row.named_section.as_deref(), Some("263A"));
    assert_eq!(row.path_section.as_deref(), Some("263"));
}

/// § 1070a(b)(7)(A)(iii), which the bill reached through the Higher Education
/// Act of 1965.
///
/// The third outcome, on real data. The amendment names *section 401 of the
/// Act*, and an Act's own numbering is not the Code's, so the section is only
/// knowable from the citation beside it — `20 U.S.C. 1070a(b)(7)(A)(iii)`,
/// which the extractor declines because it carries no section marker. Reading
/// the bare `401` and comparing it against `1070a` would manufacture a fault out
/// of that limitation (#140).
const SECTION_1070A_B_7_A_III: &str = "uscode/title_20/chapter_28/subchapter_IV/part_A/subpart_1/section_1070a/subsection_b/paragraph_7/subparagraph_A/clause_iii";

/// A declined citation is its own case, and never a disagreement.
#[test]
fn should_report_its_own_case_when_the_extractor_declined_the_amendments_citation() {
    let dataset = amendment_links();
    let report = section_agreement::section_agreement(&dataset, &LinkQuery::new())
        .expect("the check should read the dataset");

    let row = report
        .rows
        .iter()
        .find(|row| row.path == SECTION_1070A_B_7_A_III)
        .expect("the run recorded a link at § 1070a(b)(7)(A)(iii)");

    assert_eq!(row.outcome, Outcome::CouldNotBeRead);
    // Why, in the extractor's own words, so a reviewer meets a named extractor
    // limit rather than a silent gap.
    let reason = row.reason.as_deref().expect("a third case says why");
    assert!(
        reason.contains("20 U.S.C. 1070a") && reason.contains("no section marker"),
        "the reason should name the declined citation and the extractor's own \
         words, found {reason:?}"
    );
}

/// The figure is reported per window, because the split between windows is the
/// signal that filed the ticket: one dataset showed 4% of its first window's
/// links disagreeing and 29% of its second window's.
///
/// **The committed corpus holds amendment links in one window only.** It holds
/// three release points, so it holds two windows, and the committed matching run
/// covers `2025-07-18 -> 2025-07-30` alone. So this proves the tally is taken
/// per window and that each window's share is computed from that window's own
/// links; reproducing the 4%-versus-29% split needs a dataset with a second
/// matching run in it.
#[test]
fn should_report_a_share_for_each_window_when_the_dataset_holds_amendment_links() {
    let dataset = amendment_links();
    let report = section_agreement::section_agreement(&dataset, &LinkQuery::new())
        .expect("the check should read the dataset");

    let [window] = &report.windows[..] else {
        panic!(
            "the committed run covers one window, found {}",
            report.windows.len()
        );
    };
    assert_eq!(window.window.from_date, BEFORE);
    assert_eq!(window.window.to_date, AFTER);

    // Three counts that measure three different things, so the share is taken
    // from this window's own links and not from the dataset's total.
    assert_eq!(
        window.checked,
        window.agrees + window.disagrees + window.could_not_be_read
    );
    assert!(window.disagrees > 0, "the corpus holds disagreements");
    assert!(
        (window.disagreeing_share - window.disagrees as f64 / window.checked as f64).abs()
            < f64::EPSILON,
        "the share should be this window's disagreements over its own links"
    );
}

/// The report is a queue, ordered so a reviewer meets the suspect links first,
/// and each row names the link a reviewer would act on.
///
/// The id leads because this list is the review queue and the id is the one
/// field a reviewer copies out of it, into `settle` (#227). A link that agrees
/// is counted in its window's tally and is not queued: it asks nothing of
/// anybody.
#[test]
fn should_queue_the_disagreements_first_and_name_each_link_when_the_report_is_read() {
    let dataset = amendment_links();
    let report = section_agreement::section_agreement(&dataset, &LinkQuery::new())
        .expect("the check should read the dataset");

    assert!(
        report.rows.iter().all(|row| row.outcome != Outcome::Agrees),
        "a link that agrees asks nothing of a reviewer and is not queued"
    );

    let first_unread = report
        .rows
        .iter()
        .position(|row| row.outcome == Outcome::CouldNotBeRead)
        .expect("the corpus holds links whose citation was declined");
    assert!(
        report.rows[..first_unread]
            .iter()
            .all(|row| row.outcome == Outcome::Disagrees),
        "every disagreement should come before the first case that could not be read"
    );
    assert!(
        report.rows[first_unread..]
            .iter()
            .all(|row| row.outcome == Outcome::CouldNotBeRead),
        "nothing should follow the cases that could not be read"
    );

    // The short id `settle` accepts, so a reviewer copies the row's own field.
    let row = &report.rows[0];
    let link = dataset
        .link_by_id_prefix(&row.id)
        .expect("the id should look up");
    assert!(
        matches!(link, Named::One(_)),
        "a row's id should name exactly one link, found {link:?}"
    );
}

/// The command a reviewer runs, over a dataset saved to disk.
///
/// `--json` is the surface an agent reads, and it survives a change to the
/// human-readable text.
#[test]
fn should_report_the_queue_and_each_windows_share_when_the_command_runs() {
    let path = format!("{}/section_agreement.json", env!("CARGO_TARGET_TMPDIR"));
    amendment_links()
        .save(&path, Format::Compact)
        .expect("the fixture should save");

    let run = Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args(["section-agreement", &path, "--json"])
        .output()
        .expect("the binary should run");
    assert!(
        run.status.success(),
        "the command should exit zero, stderr: {}",
        String::from_utf8_lossy(&run.stderr)
    );

    let report: serde_json::Value =
        serde_json::from_slice(&run.stdout).expect("--json should emit json");

    let window = &report["windows"][0];
    assert_eq!(window["window"]["from_date"], BEFORE);
    assert_eq!(window["window"]["to_date"], AFTER);
    assert!(
        window["disagrees"].as_u64().expect("a count") > 0,
        "the corpus holds disagreements"
    );
    assert!(
        window["disagreeing_share"].as_f64().expect("a share") > 0.0,
        "the share is the figure the ticket asks for, per window"
    );

    let rows = report["rows"].as_array().expect("the queue is an array");
    assert_eq!(rows[0]["outcome"], "disagrees", "the suspect links lead");
    assert!(
        rows.iter().all(|row| row["outcome"] != "agrees"),
        "a link that agrees asks nothing of a reviewer and is not queued"
    );
}

// --- The quote rule -------------------------------------------------------
//
// Amendment language quotes three kinds of thing, and none of them is the
// provision being amended: the text struck, the text inserted, and the
// positional anchor an insertion is placed after. A section read out of a
// quotation is therefore not a section the amendment names, and comparing one
// against the link's path reports a correct link as suspect.

/// The committed public law, which is the real record of what the bill wrote.
const BILL_DIR: &str = "tests/test_data/congress_client_cache/bill/119/hr/1";
const BILL_ID_HR1: &str = "119-hr-1";
/// The work the stored bill is held under.
const BILL_WORK: &str = "publiclawdocument_119-21";

/// The clause the bill writes at § 83001(a)(2)(B), which quotes the section it
/// searches **for** and names no section it amends:
///
/// > by inserting ", as in effect for such academic year," after
/// > "section 479A(b)(1)(B)(v)"
///
/// The maintainer's own dataset carries a link on these words pointing inside
/// § 1070a, and `settle --explain` shows the change landed exactly where the
/// link says. The row read `479A` against `1070a`, and the link was right all
/// along.
const QUOTED_ANCHOR_CLAUSE: &str = "as in effect for such academic year";

/// The bill as the Congress client would hand it over, read from the committed
/// cache.
fn committed_bill_download() -> BillDownload {
    let read = |name: &str| {
        std::fs::read_to_string(format!("{BILL_DIR}/{name}"))
            .unwrap_or_else(|e| panic!("{name} should be committed: {e}"))
    };
    BillDownload {
        bill_id: BILL_ID_HR1.to_string(),
        bill_xml: read("public_law.xml"),
        bill_metadata_json: read("metadata.json"),
        cosponsors_json: read("cosponsors.json"),
        votes_json: None,
        member_jsons: std::collections::HashMap::new(),
    }
}

/// The words of the one node in the committed bill whose own content holds
/// `phrase`.
///
/// Read out of the stored bill rather than typed in here, so the sentence under
/// test is the sentence the publisher wrote
/// (`docs/adr/0009-a-source-is-parsed-once-a-bill-is-a-document.md`).
fn bill_words_holding(phrase: &str) -> String {
    let mut dataset = Dataset::new(DatasetMetadata::default());
    dataset
        .load_bill_download(&committed_bill_download())
        .expect("the committed bill should load");
    let held = dataset
        .expressions(&WorkId::new(BILL_WORK))
        .expect("the dataset should answer for the bill's work");
    let bill = dataset
        .get_expression(&held[0].id)
        .expect("the expression should read")
        .expect("the expression should be there");

    let mut found = Vec::new();
    collect_words_holding(&bill.root, phrase, &mut found);
    assert_eq!(
        found.len(),
        1,
        "exactly one node of the bill should hold {phrase:?}"
    );
    found.remove(0)
}

fn collect_words_holding(node: &DocumentNode, phrase: &str, found: &mut Vec<String>) {
    if let Some(content) = &node.data.content
        && content.contains(phrase)
    {
        found.push(content.to_string());
    }
    for child in &node.children {
        collect_words_holding(child, phrase, found);
    }
}

/// A section inside a quotation is not a section the amendment names.
#[test]
fn should_not_name_a_section_inside_a_quotation_when_an_amendment_quotes_one() {
    let words = bill_words_holding(QUOTED_ANCHOR_CLAUSE);

    // Guards. Without them the case could pass on a clause that quotes nothing.
    assert!(
        words.contains("479A"),
        "the clause should quote § 479A, found {words:?}"
    );
    assert!(
        words.to_lowercase().contains("section"),
        "the clause should carry the word `section`, found {words:?}"
    );

    let naming = section_agreement::section_named_in(&words);
    assert_eq!(
        naming.section(),
        None,
        "the only section here sits inside a quotation, so these words name \
         none; found {naming:?} in {words:?}"
    );
}

/// A row a reviewer meets must say the section was quoted, and not that the
/// words named none.
///
/// The two are different facts. *"The words name no section"* is already 668 of
/// 685 unread rows on the maintainer's dataset, and folding the quoted ones into
/// it would hide how much of that bucket is this one cause — which is the
/// question `#211` is parked on.
#[test]
fn should_say_the_section_was_quoted_when_the_only_one_named_sits_in_a_quotation() {
    let words = bill_words_holding(QUOTED_ANCHOR_CLAUSE);

    let naming = section_agreement::section_named_in(&words);
    let Naming::Unread(reason) = &naming else {
        panic!("these words name no section outside a quotation, found {naming:?}");
    };
    assert!(
        reason.contains("479A") && reason.contains("quotation"),
        "the reason should name the quoted section and say it was quoted, \
         found {reason:?}"
    );
}

/// The clause the bill writes at § 70421(a)(3):
///
/// > Section 1400Z-1(b) is amended by striking paragraph (3).
///
/// A section number that carries a dash after a letter. The Code numbers whole
/// families that way — `1400Z-1`, `479a-1`, `300gg-11` — and reading only the
/// part before the dash names a real but **different** provision, which is the
/// fault `#135` and `#141` are about. The maintainer's dataset carries a row
/// reading `1400Z` against a path in `1400Z–2` for exactly this reason.
const DASHED_SECTION_CLAUSE: &str = "1400Z-1(b) is amended by striking paragraph";

/// A dashed section number is read whole, not down to its first dash.
#[test]
fn should_read_a_section_number_whole_when_it_carries_a_dash_after_a_letter() {
    let words = bill_words_holding(DASHED_SECTION_CLAUSE);

    let naming = section_agreement::section_named_in(&words);
    assert_eq!(
        naming.section(),
        Some("1400Z-1"),
        "the number runs past the dash, and § 1400Z is a different provision; \
         found {naming:?} in {words:?}"
    );
}

/// § 50(a)(5), which the bill reached by an amendment to § 1371(d)(1).
///
/// The guard that stops the quote rule being over-applied, and the committed
/// corpus's own copy of the shape the maintainer found at link `9ffa2e4992a3`:
/// a target stated plainly **outside** any quotation, differing from the path.
///
/// > Section 1371(d)(1) is amended by striking "section 50(a)(5)" and inserting
/// > "section 50(a)(6)".
///
/// It is sharp in both directions. § 1371 is outside the quotations and § 50 is
/// inside them, and § 50 is the section the path sits in — so a rule that read
/// the quotations instead of ignoring them would call this row *agrees* and
/// lose a real finding in silence.
const SECTION_50_A_5: &str = "uscode/title_26/subtitle_A/chapter_1/subchapter_A/part_IV/subpart_E/section_50/subsection_a/paragraph_5";

/// A section stated outside the quotations is still named, and still reported.
#[test]
fn should_still_report_the_link_when_the_amendment_states_its_section_outside_the_quotations() {
    let dataset = amendment_links();
    let report = section_agreement::section_agreement(&dataset, &LinkQuery::new())
        .expect("the check should read the dataset");

    let row = report
        .rows
        .iter()
        .find(|row| row.path == SECTION_50_A_5)
        .expect("the run recorded a link at § 50(a)(5)");

    assert_eq!(row.outcome, Outcome::Disagrees);
    assert_eq!(row.named_section.as_deref(), Some("1371"));
    assert_eq!(row.path_section.as_deref(), Some("50"));
}

/// § 41(d)(1)(A), which the bill reached by an amendment that inserts a whole
/// new § 174A.
///
/// > Part VI of subchapter B of chapter 1 is amended by inserting after section
/// > 174 the following new section:"SEC. 174A. 26 USC 174A.DOMESTIC RESEARCH OR
/// > EXPERIMENTAL EXPENDITURES."…
///
/// § 174 here is the **anchor** the new section is placed after, not the
/// section the amendment acts on, and an anchor is no more the target than a
/// quoted string is. The section the amendment names is the new one, and the
/// bill states its number inside the text it inserts.
const SECTION_41_D_1_A: &str = "uscode/title_26/subtitle_A/chapter_1/subchapter_A/part_IV/subpart_D/section_41/subsection_d/paragraph_1/subparagraph_A";

/// An insertion names the new section, not the one it is placed after.
#[test]
fn should_name_the_new_section_when_an_amendment_inserts_one_after_another() {
    let dataset = amendment_links();
    let report = section_agreement::section_agreement(&dataset, &LinkQuery::new())
        .expect("the check should read the dataset");

    let row = report
        .rows
        .iter()
        .find(|row| row.path == SECTION_41_D_1_A)
        .expect("the run recorded a link at § 41(d)(1)(A)");

    assert_eq!(row.outcome, Outcome::Disagrees);
    assert_eq!(
        row.named_section.as_deref(),
        Some("174A"),
        "the new section is what the amendment names; § 174 is only the anchor"
    );
    assert_eq!(row.path_section.as_deref(), Some("41"));
}

// --- Collapsed quotes -----------------------------------------------------
//
// The stored words often write a closing quote and the next opening quote as
// **one** character: `…for 'clause (ii)'."(D) Special rule…`. A reader that
// pairs quote marks by position loses step there, and every section after the
// join lands in a stretch it wrongly takes for prose. The count of marks cannot
// say when this happened: an even count breaks the same way as an odd one.
//
// So the section an amendment names is read only from the words **before its
// first quotation mark**. Amendment language names its target before it starts
// to quote.

/// § 529A(b)(2)(B), which carries a link to the amendment that adds the new
/// part on Trump accounts:
///
/// > Subchapter F of chapter 1 is amended by adding at the end the following
/// > new part:"PART IX--… TRUMP ACCOUNTS"Sec. 530A. Trump accounts."SEC. 530A.
/// > …"(a) General Rule.--… an individual retirement account under section
/// > 408(a)."(b) …
///
/// Nothing before the first quotation mark names a section. § 408 sits inside
/// the inserted text, after the collapsed marks `"Sec.` and `."(a)`, and a
/// reader that paired the marks by position took it for prose.
const SECTION_529A_B_2_B: &str = "uscode/title_26/subtitle_A/chapter_1/subchapter_F/part_VIII/section_529A/subsection_b/paragraph_2/subparagraph_B";

/// A section after the first quotation mark is not a section the amendment
/// names, even where the marks have collapsed together.
#[test]
fn should_not_name_a_section_when_it_comes_only_after_the_first_quotation_mark() {
    let dataset = amendment_links();
    let report = section_agreement::section_agreement(&dataset, &LinkQuery::new())
        .expect("the check should read the dataset");

    let at_529a: Vec<_> = report
        .rows
        .iter()
        .filter(|row| row.path == SECTION_529A_B_2_B)
        .collect();

    assert!(
        at_529a
            .iter()
            .all(|row| row.named_section.as_deref() != Some("408")),
        "§ 408 sits inside the inserted part, so no row may name it; found {at_529a:?}"
    );
    assert!(
        at_529a
            .iter()
            .any(|row| row.outcome == Outcome::CouldNotBeRead
                && row
                    .reason
                    .as_deref()
                    .is_some_and(|reason| reason.contains("before its first quotation"))),
        "the Trump-accounts row should be unread, and say that nothing before \
         the first quotation names a section; found {at_529a:?}"
    );
}

/// The clause the bill writes to add the definitions of prohibited foreign
/// entities to § 7701(a):
///
/// > Section 7701(a) is amended by adding at the end the following new
/// > paragraphs:
///
/// The maintainer's dataset carries two links on these words, `46de57fb0c0f`
/// and `324e65f1e549`, and both point inside § 48E. The words state their
/// target plainly, before any quotation, and it is not § 48E: the model matched
/// the amendment to a provision that only **uses** the new definitions. That is
/// a real matching error, and the prefix rule must still see it.
const SECTION_7701_A_CLAUSE: &str =
    "Section 7701(a) is amended by adding at the end the following new paragraphs";

/// A target stated before the first quotation mark is still named.
#[test]
fn should_name_the_section_when_the_amendment_states_it_before_any_quotation() {
    let words = bill_words_holding(SECTION_7701_A_CLAUSE);

    let naming = section_agreement::section_named_in(&words);
    assert_eq!(
        naming.section(),
        Some("7701"),
        "the words name § 7701 before they quote anything, and a link inside \
         § 48E on them must disagree; found {naming:?} in {words:?}"
    );
}

// --- Cross-references -----------------------------------------------------

/// The new § 4968(c) the bill writes, as the maintainer's dataset stores it at
/// link `3a9530032fb5`: one quoted run of the inserted text, with no quotation
/// mark left in it at all, because the excerpt starts inside the quotation.
///
/// > (c) Applicable Educational Institution.—For purposes of this subchapter,
/// > the term 'applicable educational institution' means an eligible
/// > educational institution (as defined in section 25A(f)(2))—
///
/// That link points at § 4968(c), and the row read `25A` against `4968`. § 25A
/// is only the place a term is defined. A section after *as defined in*,
/// *within the meaning of* or *described in* is a cross-reference, never the
/// section acted on.
const CROSS_REFERENCE_RUN: &str =
    "means an eligible educational institution (as defined in section 25A(f)(2))";

/// The one quoted run of the committed bill's words that holds `phrase`,
/// without its marks — the shape a stored excerpt takes when it starts inside
/// the inserted text.
fn quoted_run_holding(phrase: &str) -> String {
    let words = bill_words_holding(phrase);
    let runs: Vec<&str> = words
        .split(['"', '\u{201C}', '\u{201D}'])
        .filter(|run| run.contains(phrase))
        .collect();
    let [run] = runs[..] else {
        panic!("exactly one quoted run should hold {phrase:?}, found {runs:?}");
    };
    run.to_string()
}

/// A section named only as a cross-reference is not the section acted on.
#[test]
fn should_not_name_a_section_when_it_is_only_a_cross_reference() {
    let words = quoted_run_holding(CROSS_REFERENCE_RUN);

    // Guard. Without it the case could pass on words that name a target too.
    assert!(
        words.starts_with("(c) Applicable Educational Institution"),
        "the run should be the new § 4968(c) alone, found {words:?}"
    );

    let naming = section_agreement::section_named_in(&words);
    let Naming::Unread(reason) = &naming else {
        panic!("§ 25A is only where a term is defined, found {naming:?} in {words:?}");
    };
    assert!(
        reason.contains("25A") && reason.contains("cross-reference"),
        "the reason should name the section and say it is a cross-reference, \
         found {reason:?}"
    );
}
