//! The inside of a writing: pages of the official reporter, the paragraphs that
//! start on each page, and the footnotes
//! (`docs/adr/0014-a-case-is-a-work-and-a-citation-runs-paragraph-to-subdivision.md`,
//! decisions 3 and 4; #314).
//!
//! *Boeing Co. v. United States*, 537 U.S. 437 (2003), is the case most of these
//! tests read. CourtListener holds its two writings as split records from
//! Harvard's Caselaw Access Project, in `xml_harvard`, with the page labels of
//! the U.S. Reports and each paragraph's own CAP id. Every record here is a real
//! CourtListener response committed under `tests/test_data/courtlistener`.

use words_to_data::courtlistener::{ClusterRecord, OpinionRecord, case_expression};
use words_to_data::dataset::Expression;
use words_to_data::document::DocumentNode;

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

/// *Boeing*, built from all three records CourtListener holds for it.
fn boeing() -> Expression {
    case_expression(
        &cluster(122262),
        &[opinion(122262), opinion(9434364), opinion(9434365)],
    )
    .expect("the case should build")
    .expression
}

/// The Court's opinion in *Boeing*, by Justice Stevens.
const MAJORITY: &str = "judicial/case_122262/opinion_9434364";

/// The node at `path`, or a failure that names the path.
fn node<'a>(expression: &'a Expression, path: &str) -> &'a DocumentNode {
    expression
        .root
        .find(path)
        .unwrap_or_else(|| panic!("{path} should be a node"))
}

/// The words of the node at `path`.
fn words<'a>(expression: &'a Expression, path: &str) -> &'a str {
    node(expression, path)
        .data
        .content
        .as_deref()
        .unwrap_or_else(|| panic!("{path} should carry words"))
}

// --- Decision 3: structure comes first in the field order -----------------

/// *Obergefell* carries the court's own PDF text and Harvard's XML; *Snow*
/// carries a court website's HTML, Lawbox's HTML and Harvard's XML;
/// *Britannica* carries HTML and Harvard's XML. Only Harvard's field holds the
/// writings, the paragraphs and the page labels, so it is the one read. It is
/// OCR, so the text is `MachineSuggested` and says so.
#[test]
fn should_read_xml_harvard_first_when_a_record_carries_it_beside_other_fields() {
    for id in [2812209, 109019, 406879, 2651100, 6248, 1527901] {
        let (_, source, _) = opinion(id)
            .text()
            .unwrap_or_else(|| panic!("opinion {id} should carry text"));
        assert_eq!(source.field, "xml_harvard", "opinion {id}");
        assert_eq!(source.method, "ocr", "opinion {id}");
        assert_eq!(
            source.verification,
            words_to_data::link::VerificationState::MachineSuggested,
            "opinion {id}"
        );
    }
}

/// Without Harvard's field, the order is the rest of decision 3: Lawbox's
/// HTML comes before a court website's HTML.
#[test]
fn should_read_html_lawbox_before_html_when_a_record_carries_no_structure_field() {
    let (_, source, _) = opinion(122262).text().expect("the combined record has text");
    assert_eq!(source.field, "html_lawbox");
}

// --- Decision 4: paths inside a writing follow the reporter's pages -------

/// The majority opinion starts on page 440 of 537 U.S. Harvard marks each page
/// break with `<page-number label="441">`. Paragraph `b676-8` starts on 440 and
/// runs over the break onto 441, so it belongs to 440. Paragraph `b678-4` starts
/// with the mark for 442, so it is the first paragraph that starts on 442.
#[test]
fn should_put_each_paragraph_under_the_reporter_page_it_starts_on_when_harvard_labels_the_pages()
{
    let case = boeing();

    let over_the_break = words(&case, &format!("{MAJORITY}/page_440/paragraph_4"));
    assert!(
        over_the_break.starts_with("The 1971 statute provided special tax treatment"),
        "the fourth paragraph that starts on 440, got {over_the_break:.80}"
    );
    assert!(
        over_the_break.contains("but no tax is payable on the DISC"),
        "a paragraph keeps its words after a page break, got {over_the_break}"
    );
    assert!(
        words(&case, &format!("{MAJORITY}/page_441/paragraph_1"))
            .starts_with("The DISC statute does not, however"),
        "the first paragraph that starts on 441"
    );
    assert!(
        words(&case, &format!("{MAJORITY}/page_442/paragraph_1"))
            .contains("Soon after its enactment, the DISC statute"),
        "a paragraph that opens with the page mark starts on that page"
    );
    assert_eq!(
        node(&case, &format!("{MAJORITY}/page_442/paragraph_1"))
            .data
            .node_type
            .as_str(),
        "judicial.paragraph"
    );
    assert_eq!(
        node(&case, &format!("{MAJORITY}/page_442")).data.node_type.as_str(),
        "judicial.page"
    );
}
