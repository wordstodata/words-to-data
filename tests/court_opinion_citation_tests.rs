//! Ten real court opinions, their citations to the U.S. Code, and the question
//! #53 exists to answer:
//!
//! > which of the ten opinions cite 26 USC 174, and did that provision change
//! > after the opinion was published?
//!
//! Every record here is a real CourtListener response, committed under
//! `tests/test_data/courtlistener`, and every path asserted is a path the
//! committed release points really publish. Nothing is written by hand.
//!
//! The ten were chosen so the answer is not uniform by construction. Nine cite
//! 26 U.S.C. § 174; *Obergefell v. Hodges* does not, and it is in the set so that
//! "which of them" has a negative in it. The text arrives from four different
//! fields, from four different donors, at three different levels of trust, so the
//! verification state of a stored opinion genuinely differs between records.

use std::collections::BTreeSet;

use words_to_data::citation::resolve::{Resolution, SectionPaths, resolve};
use words_to_data::citation::{Opinion, cites_links, usc};
use words_to_data::courtlistener::markup::text_of_markup;
use words_to_data::courtlistener::{
    CaseExpression, ClusterRecord, OpinionRecord, case_expression, writing_path,
};
use words_to_data::dataset::{Coverage, Dataset, DatasetMetadata, ExpressionId, WorkId};
use words_to_data::document::text_method;
use words_to_data::judicial::reliance::{self, Citing};
use words_to_data::judicial::{CaseFacts, OpinionFacts};
use words_to_data::link::VerificationState;
use words_to_data::storage::InMemoryStorage;

const CACHE: &str = "tests/test_data/courtlistener";
const TITLE_1: &str = "tests/test_data/usc/2025-07-18/usc01.xml";
const TITLE_26_EARLIER: &str = "tests/test_data/usc/2025-07-18/usc26.xml";
const TITLE_26_LATER: &str = "tests/test_data/usc/2025-07-30/usc26.xml";
const EARLIER: &str = "2025-07-18";
const LATER: &str = "2025-07-30";

/// Section 174 as both committed release points publish it.
const SECTION_174: &str = "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174";

/// The ten opinions, as `(opinion id, cluster id)`: the records the question
/// was first asked of.
///
/// *Boeing* appears twice: 122262 is the combined record of the whole case and
/// 9434365 is Justice Thomas's dissent, and they share cluster 122262. Since
/// #313 a case is a work and is read whole, so *Boeing* is stored from all three
/// of its records: the split majority, 9434364, and the dissent are its
/// writings, and the combined record is only a source ([`CASES`]).
const TEN: [(u64, u64); 10] = [
    (2812209, 2812209), // Obergefell v. Hodges — cites no U.S.C. section 174
    (109019, 109019),   // Snow v. Commissioner, 416 U.S. 500 (1974)
    (122262, 122262),   // Boeing Co. v. United States, 537 U.S. 437 (2003)
    (9434365, 122262),  // Boeing — Thomas, J., dissenting
    (2651100, 2651100), // Shami v. Commissioner (5th Cir. 2014)
    (6248, 6248),       // Harris v. Commissioner (5th Cir. 1994)
    (6931314, 7029371), // Hildebrand v. Commissioner (10th Cir. 1994)
    (8991218, 8998794), // Agro Science Co. v. Commissioner (5th Cir. 1991)
    (1527901, 1527901), // United Stationers, Inc. v. United States (N.D. Ill. 1997)
    (406879, 406879),   // Encyclopaedia Britannica v. Commissioner (7th Cir. 1982)
];

/// The nine cases the ten opinions belong to, by cluster id, in the order of
/// [`TEN`].
const CASES: [u64; 9] = [
    2812209, 109019, 122262, 2651100, 6248, 7029371, 8998794, 1527901, 406879,
];

/// One committed record, as the API returned it.
fn read(name: String) -> String {
    std::fs::read_to_string(format!("{CACHE}/{name}"))
        .unwrap_or_else(|error| panic!("{name} should be committed: {error}"))
}

/// One case, built from its cluster and every opinion record the cluster lists,
/// as `add-opinions` builds it.
fn case(cluster_id: u64) -> CaseExpression {
    let cluster = ClusterRecord::from_json(&read(format!("cluster_{cluster_id}.json")))
        .expect("the cluster record should read");
    let opinions: Vec<OpinionRecord> = cluster
        .opinion_ids()
        .into_iter()
        .map(|id| {
            OpinionRecord::from_json(&read(format!("opinion_{id}.json")))
                .expect("the opinion record should read")
        })
        .collect();
    case_expression(&cluster, &opinions).expect("the case should build")
}

/// The two committed records for one opinion, as the API returned them.
fn records(opinion_id: u64, cluster_id: u64) -> (OpinionRecord, ClusterRecord) {
    let opinion = OpinionRecord::from_json(&read(format!("opinion_{opinion_id}.json")))
        .expect("the opinion record should read");
    let cluster = ClusterRecord::from_json(&read(format!("cluster_{cluster_id}.json")))
        .expect("the cluster record should read");
    (opinion, cluster)
}

fn metadata() -> DatasetMetadata {
    DatasetMetadata {
        name: "Ten opinions and the U.S. Code".to_string(),
        description: "CourtListener opinions with their U.S.C. citations as links".to_string(),
        author: "words_to_data tests".to_string(),
        source_urls: vec!["https://www.courtlistener.com/api/rest/v4/".to_string()],
        license: "Public Domain".to_string(),
        version: "1.0".to_string(),
        ..Default::default()
    }
}

/// A dataset holding the nine cases of the ten opinions, the titles named, and every citation link
/// the opinions' own text supports.
///
/// The text the citations are read out of is the text that was stored, so a
/// link's evidence names something a reader of the dataset can find.
fn dataset_holding_the_ten(titles: &[(&str, &str, &str)]) -> Dataset<InMemoryStorage> {
    let mut dataset = Dataset::new(metadata());

    for (file, date, _) in titles {
        dataset
            .add_uslm_xml(file, date, None)
            .unwrap_or_else(|error| panic!("{file} should parse: {error}"));
    }

    let mut index = SectionPaths::new();
    for (_, date, work) in titles {
        // The latest printing only. Indexing a work twice would report every
        // section in it twice.
        if *date != titles.last().expect("at least one title").1 {
            continue;
        }
        let id = ExpressionId::new(WorkId::new(*work), *date);
        let expression = dataset
            .get_expression(&id)
            .expect("storage should answer")
            .unwrap_or_else(|| panic!("{id} should be held"));
        index.add_work(&expression.root);
    }

    let scope = dataset.scope().expect("the scope should derive");
    let mut texts = Vec::new();

    for cluster_id in CASES {
        let case = case(cluster_id);
        let display = CaseFacts::of(&case.expression.root.data)
            .expect("a case carries its facts")
            .case_name;
        for writing in &case.writings {
            let text = case
                .expression
                .root
                .find(&writing.path)
                .and_then(|node| node.data.content.as_deref())
                .expect("a writing carries text")
                .to_string();
            texts.push((
                Opinion::held(
                    writing.path.clone(),
                    writing.opinion_id.to_string(),
                    display.clone(),
                ),
                text,
            ));
        }
        dataset
            .add_expression(case.expression)
            .expect("a case should store");
    }

    for (opinion, text) in &texts {
        for citation in usc::find_with_report(text).0 {
            let cited = resolve(&citation, &scope, &index);
            for link in cites_links(opinion, &citation, &cited) {
                dataset.add_link(link).expect("a link should store");
            }
        }
    }

    dataset
}

// --- What the record says, and where the text came from -------------------

/// The field the text came from decides what can be claimed about it, and the ten
/// records really do differ. Asserted per record rather than in aggregate, so a
/// change in the preference order names the case it moved.
#[test]
fn should_take_the_text_from_the_field_the_record_carries_and_say_which() {
    let expected = [
        // A court's own PDF, read from its text layer. The publisher's characters.
        (
            2812209,
            "plain_text",
            text_method::TEXT_LAYER,
            VerificationState::Asserted,
        ),
        (
            2651100,
            "plain_text",
            text_method::TEXT_LAYER,
            VerificationState::Asserted,
        ),
        (
            6248,
            "plain_text",
            text_method::TEXT_LAYER,
            VerificationState::Asserted,
        ),
        // Editor-prepared HTML from the Lawbox donation.
        (
            109019,
            "html_lawbox",
            text_method::MARKUP,
            VerificationState::Asserted,
        ),
        // The combined record of *Boeing*. Its text is read here, and it is
        // not stored: the case's split records are its writings.
        (
            122262,
            "html_lawbox",
            text_method::MARKUP,
            VerificationState::Asserted,
        ),
        (
            1527901,
            "html_lawbox",
            text_method::MARKUP,
            VerificationState::Asserted,
        ),
        // A court website's HTML, or Resource.org's.
        (
            406879,
            "html",
            text_method::MARKUP,
            VerificationState::Asserted,
        ),
        // Harvard's Caselaw Access Project: raw OCR, no human review.
        (
            9434365,
            "xml_harvard",
            text_method::OCR,
            VerificationState::MachineSuggested,
        ),
        (
            6931314,
            "xml_harvard",
            text_method::OCR,
            VerificationState::MachineSuggested,
        ),
        (
            8991218,
            "xml_harvard",
            text_method::OCR,
            VerificationState::MachineSuggested,
        ),
    ];

    for (opinion_id, field, method, verification) in expected {
        let cluster_id = TEN
            .iter()
            .find(|(id, _)| *id == opinion_id)
            .expect("the id should be one of the ten")
            .1;
        let (opinion, cluster) = records(opinion_id, cluster_id);
        let (text, source, _) = opinion
            .text()
            .unwrap_or_else(|| panic!("opinion {opinion_id} should carry text"));

        assert_eq!(source.field, field, "opinion {opinion_id}");
        assert_eq!(source.method, method, "opinion {opinion_id}");
        assert_eq!(source.verification, verification, "opinion {opinion_id}");
        assert!(
            !text.trim().is_empty(),
            "opinion {opinion_id} should have words"
        );

        let case = case(cluster.id);
        if opinion_id == 122262 {
            let facts = CaseFacts::of(&case.expression.root.data).expect("case facts");
            assert_eq!(facts.combined_records, [122262], "a source, not a writing");
            continue;
        }
        let provenance = case
            .expression
            .root
            .find(&writing_path(cluster.id, opinion_id))
            .unwrap_or_else(|| panic!("opinion {opinion_id} should be a writing"))
            .data
            .provenance
            .as_ref()
            .expect("an opinion records where its text came from");
        assert_eq!(
            provenance.source,
            format!("courtlistener:opinion/{opinion_id}:{field}"),
            "the provenance names the record and the field, so a reader can look"
        );
        assert_eq!(provenance.verification, verification);
    }
}

/// The issue asks for at least three provenances, including one Harvard CAP record
/// and one derived from a court PDF, so the verification state differs between
/// records rather than being uniform by construction. Counted, not claimed.
#[test]
fn should_span_more_than_two_provenances_across_the_ten_opinions() {
    let mut fields = BTreeSet::new();
    let mut methods = BTreeSet::new();
    let mut states = BTreeSet::new();
    let mut from_a_court_document = 0;

    for (opinion_id, cluster_id) in TEN {
        let (opinion, _) = records(opinion_id, cluster_id);
        let (_, source, _) = opinion.text().expect("the record should carry text");
        fields.insert(source.field);
        methods.insert(source.method);
        states.insert(format!("{:?}", source.verification));
        if source.field == "plain_text" && opinion.download_url.is_some() {
            from_a_court_document += 1;
        }
    }

    assert!(
        fields.len() >= 3,
        "at least three provenances, got {fields:?}"
    );
    assert!(
        fields.contains("xml_harvard"),
        "one record must come from Harvard's Caselaw Access Project, got {fields:?}"
    );
    assert!(
        from_a_court_document >= 1,
        "one record must be derived from a document the court published"
    );
    assert_eq!(
        methods.len(),
        3,
        "the text was obtained three different ways, got {methods:?}"
    );
    assert_eq!(
        states.len(),
        2,
        "so the verification state is not uniform, got {states:?}"
    );
}

/// The trap this whole exercise can fail on quietly.
///
/// An opinion record carries `date_created`, which is the day CourtListener
/// ingested it, and it is the field that looks right. *Snow* was filed in 1974 and
/// ingested in the 2010s, so the two are decades apart and the assertion has
/// something to bite on.
#[test]
fn should_date_the_opinion_from_the_cluster_and_never_from_the_ingest_record() {
    let (_, cluster) = records(109019, 109019);
    let expression = case(109019).expression;

    assert_eq!(expression.id.at, "1974-05-13", "the day the court filed it");
    assert_eq!(expression.root.data.date.to_string(), "1974-05-13");
    let writing = expression
        .root
        .find(&writing_path(109019, 109019))
        .expect("Snow's one writing");
    assert_eq!(writing.data.date.to_string(), "1974-05-13");
    assert_eq!(
        cluster.date_filed.as_deref(),
        Some("1974-05-13"),
        "and that is the cluster's field, not the opinion's"
    );
}

/// Every judicial fact the two records supply, on one case, including the four
/// #53 adds: the case record, the docket, the bench, and the document the
/// publisher took the text from.
#[test]
fn should_keep_the_free_metadata_the_two_records_supply() {
    let expression = case(2651100).expression;
    let case_facts = CaseFacts::of(&expression.root.data).expect("case facts");
    let facts = OpinionFacts::of(
        &expression
            .root
            .find(&writing_path(2651100, 2651100))
            .expect("Shami's one writing")
            .data,
    )
    .expect("writing facts");

    assert_eq!(case_facts.case_name, "Shami v. Commissioner");
    assert_eq!(facts.opinion_type.as_deref(), Some("010combined"));
    assert_eq!(facts.author.as_deref(), Some("Owen"));
    assert_eq!(facts.per_curiam, Some(false));
    assert_eq!(case_facts.precedential_status.as_deref(), Some("Published"));
    assert_eq!(case_facts.cluster_id, 2651100);
    assert!(
        case_facts.docket_id.is_some(),
        "the docket is the way to the court"
    );
    assert!(
        case_facts.panel.is_some(),
        "the cluster names the judges who sat, and that is free"
    );
    assert!(
        facts
            .download_url
            .as_deref()
            .is_some_and(|url| url.ends_with(".pdf")),
        "this text came from a court PDF, and the record says which, got {:?}",
        facts.download_url
    );
    assert!(
        case_facts
            .citations
            .iter()
            .any(|printed| printed.contains("F.3d")),
        "the reporters' citations are free, got {:?}",
        case_facts.citations
    );
}

/// A dissent is its own writing and its own node, a child of the case it
/// dissents in, beside the Court's opinion. Separate opinions need no new class.
#[test]
fn should_store_a_dissent_as_its_own_writing_in_the_same_case() {
    let expression = case(122262).expression;

    let facts = |opinion_id| {
        OpinionFacts::of(
            &expression
                .root
                .find(&writing_path(122262, opinion_id))
                .unwrap_or_else(|| panic!("opinion {opinion_id} should be a writing"))
                .data,
        )
        .expect("judicial facts")
    };
    assert_eq!(
        facts(9434365).opinion_type.as_deref(),
        Some("040dissent"),
        "the publisher's vocabulary, kept verbatim"
    );
    assert_eq!(facts(9434365).author.as_deref(), Some("Thomas"));
    assert_eq!(facts(9434364).opinion_type.as_deref(), Some("020lead"));
    assert_eq!(expression.id.work.as_str(), "judicial/case_122262");
}

// --- The character that decides the answer --------------------------------

/// The failure that would have made this whole query lie.
///
/// *Encyclopaedia Britannica v. Commissioner* is a leading § 174 case, and the
/// only text CourtListener holds for it writes every section sign as the character
/// reference `&#167;` — the literal character appears nowhere in the field. The
/// U.S.C. extractor will not read a section number with no marker before it, on
/// purpose, so a text pass that strips tags and leaves the references alone finds
/// no citation at all, and the query answers "Britannica does not cite § 174",
/// confidently and wrongly.
///
/// *Snow*, in the same set, writes the character itself in the field its text is
/// taken from. Two donors, two conventions, one citation each; nothing warns you.
#[test]
fn should_read_the_citation_when_the_section_sign_is_a_character_reference() {
    let (opinion, _) = records(406879, 406879);
    let (text, source, _) = opinion.text().expect("the record carries text");

    assert_eq!(source.field, "html", "this record's text comes from `html`");
    let raw = opinion.html.as_deref().expect("and the field is there");
    assert!(
        raw.contains("&#167;"),
        "the field really does write the section sign as a reference"
    );
    assert!(
        !raw.contains('\u{a7}'),
        "and never as the character itself, so decoding is the only way through"
    );

    assert!(
        text.contains("26 U.S.C. \u{a7} 174"),
        "the reference must become the character it names"
    );

    let found = usc::find(&text);
    assert!(
        found.iter().any(|citation| citation.title == "26"
            && citation.sections.iter().any(|s| s.starts_with("174"))),
        "so the citation is read, got {:?}",
        found.iter().map(|c| &c.text).collect::<Vec<_>>()
    );
}

/// A reference this build cannot decode changes a word of the opinion, so it is
/// reported rather than left to be noticed.
#[test]
fn should_name_every_character_reference_it_could_not_decode() {
    let (text, report) = text_of_markup("<p>a rate of &permil; and &#8240; both</p>");

    assert!(
        text.contains("&permil;"),
        "an undecoded reference stays exactly as written, got {text:?}"
    );
    assert!(
        text.contains('\u{2030}'),
        "the numeric form of the same character is decoded, got {text:?}"
    );
    assert_eq!(report.unknown_entities.get("permil"), Some(&1));
    assert_eq!(
        report.summary().len(),
        1,
        "one line per reason, not one per occurrence"
    );
}

// --- The pass condition ---------------------------------------------------

/// The question #53 is a pass condition for, answered whole.
///
/// Nine of the ten cite 26 U.S.C. § 174 and *Obergefell* does not. Section 174
/// changed between the two committed release points — H.R. 1 rewrote "specified
/// research" as "foreign research" — so the answer to the second half is yes, for
/// the one window this dataset covers.
///
/// And the part that makes the answer honest: every one of these opinions was
/// filed years before the first printing held, and the dataset says **out of
/// scope** about that period rather than "no change".
#[test]
fn should_answer_which_opinions_cite_section_174_and_whether_it_changed_after_each() {
    let dataset = dataset_holding_the_ten(&[
        (TITLE_26_EARLIER, EARLIER, "uscode/title_26"),
        (TITLE_26_LATER, LATER, "uscode/title_26"),
    ]);

    let cases = reliance::cases_citing(&dataset, SECTION_174).expect("the query should run");

    let citing: BTreeSet<String> = cases
        .iter()
        .map(|case| match &case.citing {
            Citing::Held { path, .. } => path.clone(),
            Citing::NotHeld { reference, .. } => reference.clone(),
        })
        .collect();
    // Nine writings: *Boeing*'s combined record is a source now, and its
    // majority, 9434364, cites the section in its own words.
    let expected: BTreeSet<String> = [
        (109019, 109019),
        (122262, 9434364),
        (122262, 9434365),
        (2651100, 2651100),
        (6248, 6248),
        (7029371, 6931314),
        (8998794, 8991218),
        (1527901, 1527901),
        (406879, 406879),
    ]
    .into_iter()
    .map(|(cluster, opinion)| writing_path(cluster, opinion))
    .collect();
    assert_eq!(
        citing, expected,
        "every writing but Obergefell's cites section 174; Obergefell is in the \
         set because it does not"
    );

    for case in &cases {
        let name = case.citing.display();
        assert_eq!(
            case.cites.len(),
            1,
            "{name} cites the one section asked about"
        );
        let cited = &case.cites[0];

        assert_eq!(
            cited.verification,
            VerificationState::MachineSuggested,
            "{name}: a rule matched some text and no person has looked at it"
        );
        assert!(
            cited
                .citation_text
                .as_deref()
                .is_some_and(|matched| matched.contains("174")),
            "{name}: the matched text travels with the link as evidence, got {:?}",
            cited.citation_text
        );

        assert_eq!(cited.since.coverage, Coverage::InScope);
        assert_eq!(
            cited.since.windows.len(),
            1,
            "{name}: the dataset holds two printings, so one window"
        );
        let window = &cited.since.windows[0];
        assert_eq!((window.from.as_str(), window.to.as_str()), (EARLIER, LATER));

        // The window is read where the opinion pointed (#302). Section 174 did
        // change between these printings, at (a), (a)(2)(B), (b) and (d), and
        // not at the (a)(1) that three of the nine wrote.
        if window.at.ends_with("section_174/subsection_a/paragraph_1") {
            assert!(
                !window.changed,
                "{name}: (a)(1) did not change between these printings, got {:?}",
                window.changed_paths
            );
        } else {
            assert!(
                window.changed,
                "{name}: section 174 really did change between these printings"
            );
            assert!(
                window
                    .changed_paths
                    .iter()
                    .any(|path| path.ends_with("section_174/subsection_a")),
                "{name}: subsection (a) is one of the parts that changed, got {:?}",
                window.changed_paths
            );
            assert!(cited.since.changed_in_a_covered_window());
        }

        // The honest half. Every one of these cases predates the earliest
        // printing held, and the dataset must say so rather than imply that
        // nothing happened in between.
        let uncovered =
            cited.since.uncovered.as_ref().unwrap_or_else(|| {
                panic!("{name}: the period before the first printing is uncovered")
            });
        assert_eq!(uncovered.to, EARLIER);
        assert!(
            uncovered.from.as_str() < EARLIER,
            "{name}: the opinion was filed before the first printing held"
        );
    }

    // And the stored opinions really do differ in how far their text is trusted,
    // so the verification state is a fact about each record rather than a
    // constant.
    let states: BTreeSet<String> = cases
        .iter()
        .filter_map(|case| match &case.citing {
            Citing::Held {
                text_verification, ..
            } => text_verification.map(|state| format!("{state:?}")),
            Citing::NotHeld { .. } => None,
        })
        .collect();
    assert_eq!(
        states.len(),
        2,
        "the citing opinions' text is not all trusted alike, got {states:?}"
    );
}

/// A dataset that does not carry the title says so, and never "not found".
///
/// The same ten opinions, beside title 1 alone. Every citation to title 26 is out
/// of scope, no link is written, and the reason is a statement about our coverage
/// rather than about whether such a section exists.
#[test]
fn should_say_out_of_scope_when_the_dataset_does_not_carry_the_cited_title() {
    let dataset = dataset_holding_the_ten(&[(TITLE_1, EARLIER, "uscode/title_1")]);
    let scope = dataset.scope().expect("the scope should derive");

    assert_eq!(
        scope.covers("uscode/title_26"),
        Coverage::OutOfScope,
        "this dataset holds title 1 and knows nothing about title 26"
    );

    let citation = usc::find("26 U.S.C. \u{a7} 174")
        .into_iter()
        .next()
        .expect("one citation");
    let cited = resolve(&citation, &scope, &SectionPaths::new());
    assert_eq!(
        cited[0].resolution,
        Resolution::OutOfScope,
        "not `Absent`, which would say the law has no such section"
    );

    assert!(
        reliance::cases_citing(&dataset, SECTION_174)
            .expect("the query should run")
            .is_empty(),
        "a link into material the dataset does not hold could not be checked by \
         the party reading it, so none was written"
    );
}

/// An opinion the dataset holds is named by its path, so a reader can follow the
/// link to the text that made the citation. One the dataset does not hold is named
/// as external, and the difference is a statement about the file.
#[test]
fn should_name_a_held_opinion_by_its_path_and_an_unheld_one_as_external() {
    let dataset = dataset_holding_the_ten(&[(TITLE_1, EARLIER, "uscode/title_1")]);

    let citation = usc::find("under 1 U.S.C. \u{a7} 1")
        .into_iter()
        .next()
        .expect("one citation");
    let scope = dataset.scope().expect("the scope should derive");
    let id = ExpressionId::new(WorkId::new("uscode/title_1"), EARLIER);
    let title_1 = dataset
        .get_expression(&id)
        .expect("storage should answer")
        .expect("title 1 should be held");
    let mut index = SectionPaths::new();
    index.add_work(&title_1.root);
    let cited = resolve(&citation, &scope, &index);

    let held = cites_links(
        &Opinion::held(
            writing_path(109019, 109019),
            "109019",
            "Snow v. Commissioner, 416 U.S. 500",
        ),
        &citation,
        &cited,
    );
    let unheld = cites_links(
        &Opinion::new("11103682", "United States v. Somebody"),
        &citation,
        &cited,
    );

    assert_eq!(
        held[0].subject,
        words_to_data::link::Target::Node("judicial/case_109019/opinion_109019".to_string()),
        "the dataset holds this opinion, so the link points at the node"
    );
    assert_eq!(
        unheld[0].subject,
        words_to_data::link::Target::External {
            reference: "judicial.opinion:11103682".to_string(),
            display: "United States v. Somebody".to_string(),
        },
        "the dataset does not hold this one, and a reader is told so"
    );

    // And the held form is what makes "what does this case cite" a query.
    let mut dataset = dataset;
    for link in held {
        dataset.add_link(link).expect("a link should store");
    }
    let from_the_case = words_to_data::storage::LinkReader::links_for_path(
        &dataset,
        "judicial/case_109019/opinion_109019",
    )
    .expect("links should read back");
    assert_eq!(from_the_case.len(), 1);
}

/// A link from a court opinion must name its subject with a word that is true
/// of a court opinion (#147).
///
/// `CONTEXT.md` defines a Provision as a unit of law that stays the same thing
/// across versions. A court opinion is not a unit of law and does not change
/// across versions: it is a fixed document. So `Target::Node`, the core's
/// only word for "a node in this dataset", said in the core's own vocabulary
/// that an opinion is a provision. The shape was right — the variant carries a
/// path, which is what is needed — and the stored word was false, and `Target`
/// is serialized into every W2D file, so the falsehood travelled.
#[test]
fn should_name_a_court_opinion_with_a_word_that_is_true_of_a_court_opinion() {
    let dataset = dataset_holding_the_ten(&[(TITLE_26_LATER, LATER, "uscode/title_26")]);
    let links = words_to_data::storage::LinkReader::links_by_kind(
        &dataset,
        words_to_data::link::LinkKind::CITES,
    )
    .expect("the citation links should read back");
    assert!(!links.is_empty(), "the ten opinions cite the U.S. Code");

    for link in &links {
        assert!(
            matches!(link.subject, words_to_data::link::Target::Node(_)),
            "a citing opinion is a node in this dataset, got {:?}",
            link.subject
        );
    }

    // The word travels, so the stored spelling is asserted too.
    let stored = serde_json::to_string(&links[0].subject).expect("a target should serialize");
    assert!(
        stored.starts_with("{\"node\":"),
        "the stored word should be true of an opinion, got {stored}"
    );
}
