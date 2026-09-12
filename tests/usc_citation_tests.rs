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
fn should_find_every_citation_in_real_prose_except_a_section_ending_in_a_letter() {
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

    // Every mention in the document must be accounted for. The one kind that is
    // allowed to be missed is a section number ending in a letter, such as
    // `15 U.S.C. § 78aaa`: the published `law.section` pattern has no room for
    // it, and naming section 78 instead would name a different provision.
    let mention =
        regex::Regex::new(r"\d+ U\.S\.C\. §\s*(?P<section>[0-9A-Za-z\-.:]+)").expect("a pattern");
    let mut mentions = 0;
    for hit in mention.captures_iter(&rules) {
        mentions += 1;
        let at = hit.get(0).expect("a whole match").start();
        if found
            .iter()
            .any(|citation| citation.start <= at && citation.end() > at)
        {
            continue;
        }
        let section = hit.name("section").expect("a section").as_str();
        assert!(
            section.ends_with(|last: char| last.is_ascii_alphabetic()),
            "citation at {at} was missed and its section {section:?} does not end in a letter"
        );
    }
    assert!(mentions > 190, "the document should be full of mentions");

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
