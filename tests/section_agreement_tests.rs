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

use std::collections::HashMap;
use std::process::Command;

use words_to_data::annotation::ChangeAnnotation;
use words_to_data::congress::BillDownload;
use words_to_data::dataset::{Dataset, DatasetMetadata, ExpressionId, Format, WorkId};
use words_to_data::legislature::section_agreement::{self, Outcome};
use words_to_data::link::{Link, Named};
use words_to_data::query::LinkQuery;
use words_to_data::storage::{InMemoryStorage, LinkReader};

/// The committed output of one real matching run, by the model method
/// `match-amendments` ran before it was removed (#252).
///
/// Kept, and not replaced by the evidence method's links, because a check that
/// finds disagreements needs links that disagree. The evidence method places
/// each link from the address the markup names, which is the same reader this
/// check uses, so all 1195 of its links over the committed corpus agree. A
/// dataset built before #252 still holds model links like these
/// (`docs/adr/0005`), and they are what this check is for.
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
///
/// The dataset holds the committed bill as well, because the check reads the
/// section an amendment acts on out of the bill's own markup (#248).
///
/// **Each link is keyed to the amendment the committed bill states.** The run
/// was recorded before an amendment's id stopped depending on how its source
/// was laid out (#219), from a printing of the bill whose whitespace differs,
/// so none of its ids is a key of the committed bill. Its own words still name
/// the amendment: every one of its 317 amendments has exactly one amendment of
/// the committed bill with the same words once whitespace, quotes and dashes
/// are folded ([`words_key`]). So each link is recorded against that amendment.
fn amendment_links() -> Dataset<InMemoryStorage> {
    let json = std::fs::read_to_string(REAL_ANNOTATIONS).expect("the fixture should be readable");
    let annotations: Vec<ChangeAnnotation> =
        serde_json::from_str(&json).expect("the fixture should parse as annotations");

    let mut dataset = Dataset::new(DatasetMetadata::default());
    dataset
        .load_bill_download(&committed_bill_download())
        .expect("the committed bill should load");
    let bill = dataset
        .get_bill(BILL_ID_HR1)
        .expect("the dataset should answer for the bill")
        .expect("the bill should be there");
    let amendment_saying: HashMap<String, String> = bill
        .amendments
        .iter()
        .map(|(id, amendment)| (words_key(&amendment.amending_text), id.clone()))
        .collect();
    assert_eq!(
        amendment_saying.len(),
        bill.amendments.len(),
        "no two amendments of the bill may share their folded words"
    );

    for annotation in &annotations {
        let mut annotation = annotation.clone();
        annotation.source_bill.amendment_id = amendment_saying
            .get(&words_key(&annotation.source_bill.causative_text))
            .expect("every amendment of the run is stated by the committed bill")
            .clone();
        annotation.source_bill.bill_id = BILL_ID_HR1.to_string();
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

/// An amendment's words with the differences between two printings folded
/// away: curly quotes, every run of dashes, and all whitespace.
///
/// The two printings of `119-hr-1` differ in exactly these. One writes an em
/// dash and the other `--`, and one puts a space where the other does not.
fn words_key(words: &str) -> String {
    let quotes_folded = words
        .replace(['\u{2018}', '\u{2019}'], "'")
        .replace(['\u{201C}', '\u{201D}'], "\"");
    let dashes_folded = words_to_data::citation::usc::fold_dashes(&quotes_folded);
    let mut key = String::new();
    for character in dashes_folded.chars().filter(|c| !c.is_whitespace()) {
        if character == '-' && key.ends_with('-') {
            continue;
        }
        key.push(character);
    }
    key
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

    // The run recorded more than one link at this path — the new § 174A is
    // another — so the row is found by what it names, not by its place in the
    // queue.
    let row = report
        .rows
        .iter()
        .find(|row| row.path == SECTION_263_A_1_B && row.named_section.as_deref() == Some("263A"))
        .expect("the run recorded a link at § 263(a)(1)(B) on the § 263A amendment");

    assert_eq!(row.outcome, Outcome::Disagrees);
    assert_eq!(row.path_section.as_deref(), Some("263"));
}

/// § 1070a(b)(7)(A)(iii), which the bill reached through the Higher Education
/// Act of 1965.
///
/// The amendment names *section 401 of the Act*, and an Act's own numbering is
/// not the Code's. The prose reader this check had before #248 could read the
/// Code's place only from the citation beside it — `20 U.S.C.
/// 1070a(b)(7)(A)(iii)` — which the citation extractor declines, so the link
/// came out as *could not be read*. The publisher's own `<ref>` beside that
/// citation says `/us/usc/t20/s1070a`, and the address resolver reads it.
const SECTION_1070A_B_7_A_III: &str = "uscode/title_20/chapter_28/subchapter_IV/part_A/subpart_1/section_1070a/subsection_b/paragraph_7/subparagraph_A/clause_iii";

/// A section of an Act is compared at the place the publisher codified it.
#[test]
fn should_agree_when_the_publisher_codifies_the_acts_section_at_the_links_section() {
    let dataset = amendment_links();
    let report = section_agreement::section_agreement(&dataset, &LinkQuery::new())
        .expect("the check should read the dataset");

    let window = &report.windows[0];
    assert!(window.checked > 0, "the run recorded links in its window");
    let queued: Vec<_> = report
        .rows
        .iter()
        .filter(|row| row.path == SECTION_1070A_B_7_A_III)
        .collect();
    assert!(
        queued.is_empty(),
        "the amendment names § 401 of the Act, codified at § 1070a, where the \
         link sits; a link that agrees is not queued, found {queued:?}"
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

// --- The bill's markup -----------------------------------------------------
//
// The section an amendment acts on is read out of the bill the dataset holds,
// by the address resolver. Amendment language quotes the text struck, the text
// inserted and the anchor an insertion is placed after, and none of those is
// the provision acted on. The markup sets the quoted text apart, so the
// resolver never reads it as the amendment's own words. The cases that pin the
// reading itself — a quoted anchor, a dashed number, a cross-reference — are in
// `tests/amendment_address_tests.rs`.

/// The committed public law, which is the real record of what the bill wrote.
const BILL_DIR: &str = "tests/test_data/congress_client_cache/bill/119/hr/1";
const BILL_ID_HR1: &str = "119-hr-1";

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

/// § 529A(b)(2)(B), which carries a link to the amendment that adds the new
/// part on Trump accounts:
///
/// > Subchapter F of chapter 1 is amended by adding at the end the following
/// > new part:"PART IX--… TRUMP ACCOUNTS"Sec. 530A. Trump accounts."SEC. 530A.
/// > …"(a) General Rule.--… an individual retirement account under section
/// > 408(a)."(b) …
///
/// The amending line names a subchapter and no section. § 408 sits inside the
/// inserted text, after the collapsed marks `"Sec.` and `."(a)`, and a prose
/// reader that paired the marks by position took it for the amendment's own
/// words. A new *part* holds more than one section, so no one section is its
/// address.
const SECTION_529A_B_2_B: &str = "uscode/title_26/subtitle_A/chapter_1/subchapter_F/part_VIII/section_529A/subsection_b/paragraph_2/subparagraph_B";

/// A section inside the text an amendment inserts is not the section it names.
#[test]
fn should_not_name_a_section_when_it_sits_only_in_the_inserted_text() {
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
                    .is_some_and(|reason| reason.contains("no section under amendment"))),
        "the Trump-accounts row should be unread, and say that the amendment \
         names no section; found {at_529a:?}"
    );
}
