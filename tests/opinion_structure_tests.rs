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

use words_to_data::courtlistener::{ClusterRecord, OpinionRecord};

const CACHE: &str = "tests/test_data/courtlistener";

/// One committed record, as the API returned it.
fn read(name: &str) -> String {
    std::fs::read_to_string(format!("{CACHE}/{name}"))
        .unwrap_or_else(|error| panic!("{name} should be committed: {error}"))
}

fn opinion(id: u64) -> OpinionRecord {
    OpinionRecord::from_json(&read(&format!("opinion_{id}.json"))).expect("the opinion should read")
}

#[allow(dead_code)]
fn cluster(id: u64) -> ClusterRecord {
    ClusterRecord::from_json(&read(&format!("cluster_{id}.json"))).expect("the cluster should read")
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
