//! Reading U.S. Code citations out of text (#52).
//!
//! The fixtures are the `examples` array of the `U.S.C.` entry of
//! `reporters_db/data/laws.json` (BSD-2-Clause, Free Law Project). They are
//! published test data, so they are used exactly as they are written there,
//! awkward forms and all: `1 U.S.C.U. §§ 1-2`, `1 USC S. 1-2`,
//! `21, United States Code, Section 853`.
//!
//! The prose fixtures are the Federal Rules of Bankruptcy Procedure, in
//! `tests/test_data/usc/2025-07-30/usc11a.xml`. They are real published
//! judicial-style prose that cites the U.S. Code many hundreds of times.

use words_to_data::citation::usc;

/// The `examples` array of the `U.S.C.` entry of `laws.json`, verbatim, with the
/// title and the sections each one names.
///
/// A hyphenated form such as `1-2` stays as it is written. Whether it means one
/// section numbered `1-2` or sections 1 through 2 cannot be decided from the
/// text alone, and the U.S. Code does have sections such as `300gg-11`.
const LAWS_JSON_EXAMPLES: [(&str, &str, &[&str]); 11] = [
    ("1 U.S.C. § 1", "1", &["1"]),
    ("1 U.S.C.A. § 1", "1", &["1"]),
    ("1 U.S.C.S. § 1", "1", &["1"]),
    ("1 U.S.C.U. §§ 1-2", "1", &["1-2"]),
    ("1 U.S.C. sec. 1", "1", &["1"]),
    ("1 U.S.C. Sections 1-2", "1", &["1-2"]),
    ("1 USC S. 1-2", "1", &["1-2"]),
    ("1 U.S. Code §1", "1", &["1"]),
    ("21, United States Code, Section 853", "21", &["853"]),
    ("18, United States Code, Section 3500", "18", &["3500"]),
    (
        "18, United States Code, Section 981(a)(l)(C)",
        "18",
        &["981(a)(l)(C)"],
    ),
];

#[test]
fn should_read_the_title_and_the_section_when_the_text_is_a_plain_citation() {
    let found = usc::find("See 26 U.S.C. § 174 (2018).");

    assert_eq!(found.len(), 1, "one citation, got {found:?}");
    assert_eq!(found[0].title, "26");
    assert_eq!(found[0].sections, ["174"]);
    assert_eq!(
        found[0].text, "26 U.S.C. § 174",
        "the matched text is the evidence for the link, so it is kept verbatim"
    );
}

#[test]
fn should_cover_every_published_example_when_the_examples_come_from_laws_json() {
    let mut missed = Vec::new();

    for (example, title, sections) in LAWS_JSON_EXAMPLES {
        let found = usc::find(example);
        let matched = found.first().map(|citation| {
            (
                citation.title.as_str(),
                citation.sections.iter().map(String::as_str).collect(),
                citation.text.as_str(),
            )
        });
        if matched != Some((title, sections.to_vec(), example)) {
            missed.push(format!("{example:?} -> {found:?}"));
        }
    }

    assert!(
        missed.is_empty(),
        "every published example must be read whole:\n{}",
        missed.join("\n")
    );
}

/// Section numbers that end in a letter, with the title and the sections each
/// citation names.
///
/// Every one of these is a real section of the Code held in
/// `tests/test_data/usc`. A third of the Code is numbered this way — 10,085 of
/// 29,592 section numbers in the 2025-07-30 release end in a letter — so a
/// pattern that cannot name them cannot name a third of the law (#135).
const LETTERED_SECTIONS: [(&str, &str, &[&str]); 6] = [
    ("26 U.S.C. § 45X", "26", &["45X"]),
    ("21 U.S.C. § 355a", "21", &["355a"]),
    ("15 U.S.C. § 78aaa", "15", &["78aaa"]),
    // Four letters is the longest run the Code uses: 15 U.S.C. § 77bbbb.
    ("15 U.S.C. § 77bbbb", "15", &["77bbbb"]),
    // A letter run in the middle, with a numbered part after it.
    ("42 U.S.C. § 300gg-11", "42", &["300gg-11"]),
    // A letter run on both parts: 42 U.S.C. § 1395w-4a.
    ("42 U.S.C. § 1395w-4a", "42", &["1395w-4a"]),
];

#[test]
fn should_read_the_whole_number_when_a_section_number_ends_in_a_letter() {
    let mut missed = Vec::new();

    for (citation, title, sections) in LETTERED_SECTIONS {
        let found = usc::find(citation);
        let matched = found.first().map(|read| {
            (
                read.title.as_str(),
                read.sections.iter().map(String::as_str).collect(),
                read.text.as_str(),
            )
        });
        if matched != Some((title, sections.to_vec(), citation)) {
            missed.push(format!("{citation:?} -> {found:?}"));
        }
    }

    assert!(
        missed.is_empty(),
        "a lettered section number must be read whole:\n{}",
        missed.join("\n")
    );
}

#[test]
fn should_keep_the_subsection_when_a_lettered_section_also_names_one() {
    // `26 U.S.C. § 45X(b)(1)` is the shape a bill amends. Losing the letter would
    // name section 45, and losing the brackets would lose what was cited.
    let found = usc::find("credit under 26 U.S.C. § 45X(b)(1)(A)");

    assert_eq!(found.len(), 1, "one citation, got {found:?}");
    assert_eq!(found[0].sections, ["45X(b)(1)(A)"]);
    assert_eq!(found[0].uslm_id("45X(b)(1)(A)"), "/us/usc/t26/s45X");
}

#[test]
fn should_read_both_sections_when_a_list_mixes_a_lettered_number_with_a_plain_one() {
    // A letter suffix must not break the list continuation logic.
    let found = usc::find("see 21 U.S.C. §§ 355a, 360 and 355b");

    assert_eq!(found.len(), 1, "one citation, got {found:?}");
    assert_eq!(found[0].sections, ["355a", "360", "355b"]);
}

#[test]
fn should_read_both_sections_when_a_citation_names_a_list_of_them() {
    // eyecite reads this as one citation to section 1983 and loses 1988
    // (docs/research/courtlistener-formats.md, section 8.1). A lost section is
    // a lost link.
    let found = usc::find("under 42 U.S.C. §§ 1983, 1988");

    assert_eq!(found.len(), 1, "one citation, not two, got {found:?}");
    assert_eq!(found[0].title, "42");
    assert_eq!(found[0].sections, ["1983", "1988"]);
    assert_eq!(
        found[0].text, "42 U.S.C. §§ 1983, 1988",
        "the evidence covers the whole list, because the whole list is the citation"
    );
}

#[test]
fn should_stop_the_list_when_the_next_number_is_the_title_of_another_citation() {
    // The comma here separates two citations. Reading 42 as a further section of
    // title 26 would invent a provision and lose a real one.
    let found = usc::find("26 U.S.C. § 174, 42 U.S.C. § 1983");

    assert_eq!(found.len(), 2, "two citations, got {found:?}");
    assert_eq!(found[0].sections, ["174"]);
    assert_eq!(found[1].title, "42");
    assert_eq!(found[1].sections, ["1983"]);
}

#[test]
fn should_read_every_section_when_real_prose_lists_three_of_them() {
    // From the Federal Rules of Bankruptcy Procedure, in usc11a.xml.
    let found = usc::find("11 U.S.C. §§ 727(a)(11), 1328(g)(2), 1141(d)(3)(C). ");

    assert_eq!(found.len(), 1);
    assert_eq!(
        found[0].sections,
        ["727(a)(11)", "1328(g)(2)", "1141(d)(3)(C)"]
    );
}

#[test]
fn should_read_both_sections_when_real_prose_joins_them_with_and() {
    // Also from usc11a.xml. `and` joins a list as often as a comma does.
    let found = usc::find("28 U.S.C. §§ 1475 and 1477 and clarifies the procedure");

    assert_eq!(found.len(), 1, "got {found:?}");
    assert_eq!(found[0].sections, ["1475", "1477"]);
}

#[test]
fn should_find_every_citation_in_real_prose_when_each_one_carries_a_section_marker() {
    // Real prose rather than a hand-written string: the notes of the Federal
    // Rules of Bankruptcy Procedure cite the U.S. Code throughout.
    let rules = std::fs::read_to_string("tests/test_data/usc/2025-07-30/usc11a.xml")
        .expect("the bankruptcy rules should be in tests/test_data");

    let found = usc::find(&rules);

    assert!(
        found.len() > 190,
        "the rules cite the code throughout, got {}",
        found.len()
    );
    // `28 U.S.C. § 1334` from the notes to Rule 9001. The document sets the
    // space after the section mark as U+202F, a narrow no-break space, so the
    // text is compared by what it says rather than by byte.
    assert!(
        found
            .iter()
            .any(|citation| citation.title == "28" && citation.sections == ["1334"]),
        "`28 U.S.C. § 1334` is in the notes to Rule 9001"
    );

    // Every mention in the document must be read. Before the section number was
    // allowed a letter, one of the 199 was missed: `15 U.S.C. § 78aaa`, in the
    // notes to Rule 1002 (#135).
    let mention = regex::Regex::new(r"\d+\s+U\.S\.C\.\s*§+\s*(?P<section>[0-9A-Za-z\-.:]+)")
        .expect("a pattern");
    let mut mentions = 0;
    let mut missed = Vec::new();
    for hit in mention.captures_iter(&rules) {
        mentions += 1;
        let at = hit.get(0).expect("a whole match").start();
        if !found
            .iter()
            .any(|citation| citation.start <= at && citation.end() > at)
        {
            missed.push(format!(
                "{:?} at {at}",
                hit.get(0).expect("a whole").as_str()
            ));
        }
    }
    assert_eq!(mentions, 199, "the document holds 199 marked mentions");
    assert!(
        missed.is_empty(),
        "every marked mention must be read, {} were not:\n{}",
        missed.len(),
        missed.join("\n")
    );

    // Every citation must name a title and at least one section, or it is not a
    // citation and must not have been reported as one.
    for citation in &found {
        assert!(!citation.title.is_empty(), "no title in {citation:?}");
        assert!(!citation.sections.is_empty(), "no section in {citation:?}");
        assert_eq!(
            &rules[citation.start..citation.end()],
            citation.text,
            "the recorded span must be where the text was found"
        );
    }
}

#[test]
fn should_name_the_section_and_drop_the_subsection_when_asked_for_an_identifier() {
    let found = usc::find("18, United States Code, Section 981(a)(l)(C)");
    let citation = found.first().expect("one citation");

    assert_eq!(citation.work().as_str(), "uscode/title_18");
    // The `(l)` in the published example is a lower-case L where the provision
    // has a paragraph (1). Text that cannot be trusted at that depth must not be
    // resolved at that depth, so the identifier names the section.
    assert_eq!(citation.uslm_id("981(a)(l)(C)"), "/us/usc/t18/s981");
}

#[test]
fn should_find_nothing_when_the_text_has_no_citation() {
    assert!(usc::find("The Fourteenth Amendment requires no such thing.").is_empty());
}

#[test]
fn should_name_no_provision_the_opinion_did_not_cite_when_a_whole_opinion_is_read() {
    // The false-positive gate (#135). *Obergefell v. Hodges*, CourtListener
    // opinion 2812209, as the API returned it: 209,682 bytes of real judicial
    // prose holding 27 section marks, most of them state statutes and none of
    // them a U.S. Code citation. Widening the section number must not turn any of
    // those 27 into a citation, because a wrong citation is worse than a missed
    // one.
    let record: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string("tests/test_data/courtlistener/opinion_2812209.json")
            .expect("the cached opinion should read"),
    )
    .expect("the cached opinion should be JSON");
    let opinion = record["results"][0]["plain_text"]
        .as_str()
        .expect("the opinion should carry plain text");

    assert_eq!(opinion.len(), 209_682, "the cached opinion, unchanged");
    assert_eq!(opinion.matches('\u{a7}').count(), 27, "27 section marks");

    let (found, report) = usc::find_with_report(opinion);

    // Exactly two, and both are in the opinion. The first is the section of the
    // Defense of Marriage Act the case is about. The second reads only because a
    // section number may now end in a letter: before that, `§2000bb` was silently
    // nothing.
    let read: Vec<_> = found
        .iter()
        .map(|citation| (citation.title.as_str(), citation.sections.clone()))
        .collect();
    assert_eq!(
        read,
        [
            ("1", vec!["7".to_string()]),
            ("42", vec!["2000bb".to_string()])
        ],
        "no provision the opinion did not cite, got {found:?}"
    );
    // Nothing citation-shaped is left over either, so the count above is the
    // whole answer for this opinion.
    assert!(report.is_empty(), "nothing was declined, got {report:?}");
}
