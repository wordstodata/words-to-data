//! What the citation extractor says about the text it could not read (#135).
//!
//! A run that meets a citation form the patterns do not cover must say so. The
//! alternative is silence, and silence cannot be told apart from "this text
//! cites no statute". `CONTEXT.md` calls the first an Exclusion and the second a
//! Gap, and the difference is the reason.
//!
//! The prose fixtures are real: the notes of the U.S. Code write a citation
//! without a section marker throughout, and `tests/test_data/usc/2025-07-30`
//! holds them.

use words_to_data::citation::usc::{self, SkipReason};

/// The notes to the Federal Rules of Criminal Procedure, which cite the Code both
/// ways: with a section marker, and the way the Code's own notes write it.
const RULES: &str = "tests/test_data/usc/2025-07-30/usc18a.xml";

#[test]
fn should_report_the_missing_marker_when_a_citation_has_no_section_mark() {
    // The form the Code's own notes use. It stays unread on purpose: with no `§`
    // and no "section", a number in that position cannot be told from a page or
    // a year.
    let text = "The Endangered Species Act, 16 U.S.C. 1533, applies.";

    let (found, report) = usc::find_with_report(text);

    assert!(found.is_empty(), "nothing is read, got {found:?}");
    assert_eq!(report.skipped.len(), 1, "one skip, got {report:?}");
    let skipped = &report.skipped[0];
    assert_eq!(skipped.text, "16 U.S.C. 1533");
    assert_eq!(skipped.start, text.find("16 U.S.C.").expect("the citation"));
    assert_eq!(skipped.reason, SkipReason::NoSectionMarker);
}

#[test]
fn should_read_nothing_and_say_so_when_a_dash_carries_a_lettered_number_on() {
    // Real prose from the notes to the Federal Rules of Civil Procedure, in
    // `tests/test_data/usc/2025-07-30/usc28a.xml`. The Code writes the dash part
    // of a lettered section number with an en dash, which the published separator
    // class `[\-.:]` does not read.
    //
    // Both `15 U.S.C. § 77z` and `15 U.S.C. § 77z-1` are sections of the release,
    // so reading the first when the text names the second would name a real but
    // different provision. Nothing is read, and the skip says why.
    let text = "the Private Securities Litigation Reform Act, 15 U.S.C. \u{a7}\u{a7}\u{202f}77z\u{2013}1, and";

    let (found, report) = usc::find_with_report(text);

    assert!(found.is_empty(), "nothing is read, got {found:?}");
    assert_eq!(report.skipped.len(), 1, "one skip, got {report:?}");
    assert_eq!(
        report.skipped[0].text, "15 U.S.C. \u{a7}\u{a7}\u{202f}77z\u{2013}1",
        "the evidence covers the whole number, not the part that was read"
    );
    assert_eq!(
        report.skipped[0].reason,
        SkipReason::SectionNumberNotRead,
        "a marker is there, so the missing marker is not the reason"
    );
}

#[test]
fn should_name_and_place_every_skip_when_real_prose_writes_the_marker_less_form() {
    let rules = std::fs::read_to_string(RULES).expect("the criminal rules should be extracted");

    let (found, report) = usc::find_with_report(&rules);

    assert!(
        report.skipped.len() > 80,
        "the notes write the marker-less form throughout, got {}",
        report.skipped.len()
    );
    for skipped in &report.skipped {
        assert_eq!(
            &rules[skipped.start..skipped.end()],
            skipped.text,
            "the recorded span must be where the text was found"
        );
        // A skip is a citation nobody read, so no citation may cover it. A skip
        // that sits inside a citation would be double counting.
        assert!(
            !found
                .iter()
                .any(|citation| citation.start <= skipped.start && citation.end() > skipped.start),
            "{skipped:?} sits inside a citation that was read"
        );
    }
    // `42 U.S.C. 2014(y)`, in the notes to Rule 6. Marker-less, and it names a
    // subsection, so a reason of "no section number" would be wrong too.
    assert!(
        report
            .skipped
            .iter()
            .any(|skipped| skipped.text.contains("42 U.S.C. 2014(y)")
                && skipped.reason == SkipReason::NoSectionMarker),
        "the notes to Rule 6 cite 42 U.S.C. 2014(y) with no marker"
    );
}

#[test]
fn should_say_the_count_and_an_example_for_each_reason_when_the_report_is_summarised() {
    let rules = std::fs::read_to_string(RULES).expect("the criminal rules should be extracted");

    let (_found, report) = usc::find_with_report(&rules);
    let summary = report.summary();

    // One line for each reason met, not one for each skip. A title of the Code
    // writes the marker-less form thousands of times.
    assert_eq!(summary.len(), 2, "two reasons met, got {summary:?}");
    assert!(summary.iter().all(|line| line.starts_with("warning: ")));
    assert!(
        summary[0].contains("no section marker"),
        "the first reason met must be named: {summary:?}"
    );
    assert!(
        summary[0].contains("89 U.S.C. citation"),
        "the count must say how much was declined: {summary:?}"
    );
    assert!(
        summary[1].contains("a section number that cannot be read whole"),
        "the second reason met must be named: {summary:?}"
    );
    // The first skip of each reason, so a reader can go and look at one.
    let first_not_read = report
        .skipped
        .iter()
        .find(|skipped| skipped.reason == SkipReason::SectionNumberNotRead)
        .expect("the notes to Rule 6 cite 25 U.S.C. \u{a7} 479a-1");
    assert_eq!(
        first_not_read.text, "25 U.S.C. \u{a7}\u{202f}479a\u{2013}1",
        "the number, and not the full stop that ends the sentence"
    );
    assert!(
        summary[1].contains(&format!(
            "{:?} at {}",
            first_not_read.text, first_not_read.start
        )),
        "the summary must point at a skip: {summary:?}"
    );
}

#[test]
fn should_report_nothing_when_every_citation_shaped_thing_was_read() {
    let (found, report) = usc::find_with_report("See 26 U.S.C. § 174 and 21 U.S.C. § 355a.");

    assert_eq!(found.len(), 2, "got {found:?}");
    assert!(report.is_empty(), "nothing was declined, got {report:?}");
    assert!(report.summary().is_empty());
}

#[test]
fn should_report_nothing_when_the_text_names_no_statute() {
    let (found, report) = usc::find_with_report("The Fourteenth Amendment requires no such thing.");

    assert!(found.is_empty());
    // The honest answer to "this text cites no statute". It is only worth
    // anything because the other tests show that a declined citation is not
    // silent.
    assert!(report.is_empty());
}
