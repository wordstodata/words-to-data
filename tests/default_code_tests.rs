//! Bare section references, read when the opinion names its default code
//! (`docs/adr/0014-a-case-is-a-work-and-a-citation-runs-paragraph-to-subdivision.md`,
//! decision 8; #318).
//!
//! Tax opinions write "section 199A(c)(1)" or "I.R.C. § 199A" with no title,
//! and say once which code such a reference is to. When the opinion has that
//! sentence, the references are title 26 citations, `MachineSuggested`, with
//! the citation and the sentence as evidence. When it has none, nothing bare is
//! read.
//!
//! Every record here is a real CourtListener response committed under
//! `tests/test_data/courtlistener`:
//!
//! - *Patricia A. Torres* (T.C. 2025), opinion 11136635, cluster 10670048. Its
//!   footnote 1: "Unless otherwise indicated, statutory references are to the
//!   Internal Revenue Code, Title 26 U.S.C. (I.R.C. or Code), in effect at all
//!   relevant times".
//! - *Khalaf v. Department of Revenue* (Or. T.C. 2020), opinion 10607252,
//!   cluster 10140651. Its footnote 1: "references to the Internal Revenue Code
//!   (IRC, or the "Code") are to the IRC as in effect and operative for the 2013
//!   calendar year".
//! - *Shami v. Commissioner* (5th Cir. 2014), opinion 2651100, which has no such
//!   sentence.

use words_to_data::citation::default_code::{self, DefaultCode};
use words_to_data::citation::resolve::{SectionPaths, resolve};
use words_to_data::citation::{Opinion, cites_links, usc};
use words_to_data::courtlistener::{ClusterRecord, OpinionRecord, case_expression};
use words_to_data::dataset::{Dataset, DatasetMetadata, ExpressionId, WorkId};
use words_to_data::link::Link;
use words_to_data::storage::InMemoryStorage;

const CACHE: &str = "tests/test_data/courtlistener";
const TITLE_26: &str = "tests/test_data/usc/2025-07-30/usc26.xml";
const RELEASE: &str = "2025-07-30";

fn read(name: &str) -> String {
    std::fs::read_to_string(format!("{CACHE}/{name}"))
        .unwrap_or_else(|error| panic!("{name} should be committed: {error}"))
}

/// The path and the whole text of each writing of a case, as `add-opinions`
/// reads them.
fn writings(opinion_id: u64, cluster_id: u64) -> Vec<(String, String)> {
    let opinion = OpinionRecord::from_json(&read(&format!("opinion_{opinion_id}.json")))
        .expect("the opinion should read");
    let cluster = ClusterRecord::from_json(&read(&format!("cluster_{cluster_id}.json")))
        .expect("the cluster should read");
    case_expression(&cluster, &[opinion])
        .expect("the case should build")
        .writings
        .into_iter()
        .map(|writing| (writing.path, writing.text))
        .collect()
}

fn default_of(opinion_id: u64, cluster_id: u64) -> Option<DefaultCode> {
    let writings = writings(opinion_id, cluster_id);
    default_code::of_case(
        writings
            .iter()
            .map(|(path, text)| (path.as_str(), text.as_str())),
    )
}

#[test]
fn should_find_the_default_sentence_and_the_writing_that_states_it_when_torres_names_the_code() {
    let default = default_of(11136635, 10670048).expect("Torres names its default code");

    assert!(
        default.statement.starts_with(
            "Unless otherwise indicated, statutory references are to the Internal Revenue Code"
        ),
        "the statement is the sentence the opinion wrote, got {:?}",
        default.statement
    );
    assert!(
        default
            .statement
            .ends_with("in effect at all relevant times"),
        "the statement stops at the end of what it says about the Code, got {:?}",
        default.statement
    );
    assert_eq!(default.title, "26");
    assert_eq!(
        default.year, None,
        "\"at all relevant times\" names no year"
    );
    assert!(
        default.reads_bare_sections,
        "statutory references are to the Code"
    );
    assert_eq!(
        default.stated_in, "judicial/case_10670048/opinion_11136635-1",
        "the Court's opinion states it, in its footnote 1"
    );
}

#[test]
fn should_find_no_default_when_the_opinion_names_no_code() {
    assert_eq!(default_of(2651100, 2651100), None);
}

#[test]
fn should_read_the_year_and_cover_only_named_references_when_khalaf_says_what_irc_references_are_to()
 {
    let default = default_of(10607252, 10140651).expect("Khalaf names its default code");

    assert!(
        default
            .statement
            .starts_with("references to the Internal Revenue Code (IRC, or the"),
        "got {:?}",
        default.statement
    );
    assert!(
        default.statement.ends_with("for the 2013 calendar year"),
        "got {:?}",
        default.statement
    );
    assert_eq!(default.year.as_deref(), Some("2013"));
    assert!(
        !default.reads_bare_sections,
        "the sentence is about references to the IRC; Khalaf's bare references can \
         be to the Oregon statutes"
    );
}

/// Title 26 as it read on 2025-07-30, and the `judicial.cites` links of every
/// writing of one case, the way `add-opinions` makes them: the U.S.C.
/// citations, and the bare references when the case names its default code.
fn links_of_case(opinion_id: u64, cluster_id: u64) -> Vec<Link> {
    let mut dataset: Dataset<InMemoryStorage> = Dataset::new(DatasetMetadata {
        name: "Title 26".to_string(),
        ..Default::default()
    });
    dataset
        .add_uslm_xml(TITLE_26, RELEASE, None)
        .expect("title 26 should parse");
    let title = dataset
        .get_expression(&ExpressionId::new(WorkId::new("uscode/title_26"), RELEASE))
        .expect("storage should answer")
        .expect("title 26 should be held");
    let mut index = SectionPaths::new();
    index.add_work(&title.root);
    let scope = dataset.scope().expect("the scope should derive");

    let writings = writings(opinion_id, cluster_id);
    let default = default_code::of_case(
        writings
            .iter()
            .map(|(path, text)| (path.as_str(), text.as_str())),
    );
    let mut links = Vec::new();
    for (path, text) in &writings {
        let opinion = Opinion::held(path.clone(), opinion_id.to_string(), "the case");
        let mut citations = usc::find_with_report(text).0;
        if let Some(default) = &default {
            let bare = default_code::find_bare(text, default, &citations);
            citations.extend(bare);
        }
        for citation in &citations {
            let cited = resolve(citation, &scope, &index);
            links.extend(cites_links(&opinion, citation, &cited));
        }
    }
    links
}

/// The links from `writing` whose object is the section `section` of title 26.
fn citing<'a>(links: &'a [Link], writing: &str, section: &str) -> Vec<&'a Link> {
    links
        .iter()
        // The subject is the writing, or since #316 a passage in it, and the
        // object the section or a subdivision of it.
        .filter(|link| {
            link.subject.path().is_some_and(|subject| {
                subject == writing || subject.starts_with(&format!("{writing}/"))
            })
        })
        .filter(|link| {
            link.object.path().is_some_and(|path| {
                path.ends_with(&format!("/section_{section}"))
                    || path.contains(&format!("/section_{section}/"))
            })
        })
        .collect()
}

fn evidence(link: &Link) -> &str {
    link.provenance
        .evidence
        .as_ref()
        .and_then(|evidence| evidence.reasoning.as_deref())
        .expect("a citation link carries its evidence")
}

#[test]
fn should_link_bare_references_in_every_writing_when_torres_names_its_default_code() {
    let links = links_of_case(11136635, 10670048);

    let majority = "judicial/case_10670048/opinion_11136635-1";
    let dissent = "judicial/case_10670048/opinion_11136635-2";
    for (writing, section) in [
        (majority, "199A"),
        (majority, "280E"),
        (majority, "6051"),
        (dissent, "199A"),
        (dissent, "280E"),
    ] {
        let found = citing(&links, writing, section);
        assert!(
            !found.is_empty(),
            "{writing} cites section {section}, got {:?}",
            links
                .iter()
                .map(|link| (link.subject.name(), link.object.name()))
                .collect::<Vec<_>>()
        );
        let link = found[0];
        let said = evidence(link);
        assert!(
            said.contains(
                "Unless otherwise indicated, statutory references are to the Internal Revenue Code"
            ) && said.contains(majority),
            "the evidence is the citation, the sentence and the writing that states it, \
             got {said:?}"
        );
        assert_eq!(
            link.provenance.verification,
            words_to_data::link::VerificationState::MachineSuggested
        );
        assert_eq!(
            link.provenance
                .method
                .as_ref()
                .map(|method| method.name.as_str()),
            Some("bare section under the opinion's default code"),
        );
    }
}

#[test]
fn should_read_only_the_references_that_name_the_irc_and_carry_the_year_when_khalaf_states_it() {
    // Khalaf writes "IRC section 274(d)", "IRC § 6001" and also bare "section
    // 162" and "Treas Reg section 1.174-3(b)". Its sentence says what references
    // to the IRC are to, and no more, so only the named ones are read, each with
    // 2013 as its time basis.
    let writings = writings(10607252, 10140651);
    let default = default_code::of_case(
        writings
            .iter()
            .map(|(path, text)| (path.as_str(), text.as_str())),
    )
    .expect("Khalaf names its default code");
    let (_, text) = &writings[0];

    let found = default_code::find_bare(text, &default, &usc::find_with_report(text).0);

    let read: Vec<&str> = found
        .iter()
        .map(|citation| citation.text.as_str())
        .collect();
    assert!(read.contains(&"IRC section 274(d)"), "got {read:?}");
    assert!(read.contains(&"IRC \u{a7} 6001"), "got {read:?}");
    assert!(
        found
            .iter()
            .all(|citation| citation.text.starts_with("IRC")),
        "a reference that does not name the IRC is not read, got {read:?}"
    );

    let links = links_of_case(10607252, 10140651);
    let link = citing(&links, &writings[0].0, "274")
        .into_iter()
        .next()
        .expect("Khalaf cites section 274");
    let payload = &link
        .payload
        .as_ref()
        .expect("a citation has a payload")
        .value;
    assert_eq!(payload["time_basis"], "2013");
}

#[test]
fn should_not_read_a_regulation_or_another_law_when_a_bare_reference_names_one() {
    // Real words from the committed opinions, under Torres's default. Each
    // names a section of some other law than the Code.
    let default = default_of(11136635, 10670048).expect("Torres names its default code");
    for text in [
        "In addition, Treas Reg section 1.174-3(b) provides:",
        "19 Section 1.167(a)-2 of the federal Treasury Regulations",
        "section 2119 of the Tax Reform Act of 1976",
    ] {
        let found = default_code::find_bare(text, &default, &[]);
        assert!(
            found.is_empty(),
            "{text:?} is no Code citation, got {found:?}"
        );
    }
    // And the Code named after the number is still the Code.
    let found = default_code::find_bare(
        "Section 162 of the Internal Revenue Code allows",
        &default,
        &[],
    );
    assert_eq!(found.len(), 1, "got {found:?}");
}

#[test]
fn should_store_the_bare_reads_when_add_opinions_reads_a_case_that_names_its_default_code() {
    use words_to_data::link::LinkKind;
    use words_to_data::storage::LinkReader;

    let path = format!("{}/default_code_torres.sqlite", env!("CARGO_TARGET_TMPDIR"));
    let _ = std::fs::remove_file(&path);
    let mut dataset: Dataset<InMemoryStorage> = Dataset::new(DatasetMetadata {
        name: "Title 26 and Torres".to_string(),
        ..Default::default()
    });
    dataset
        .add_uslm_xml(TITLE_26, RELEASE, None)
        .expect("title 26 should parse");
    dataset
        .save_to_sqlite(&path)
        .expect("the fixture should save");

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args([
            "add-opinions",
            &path,
            "--offline",
            "--cache-dir",
            "tests/test_data",
            "--opinions",
            "11136635",
        ])
        .output()
        .expect("the binary should run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("bare reference(s) read under the default code stated in judicial/case_10670048/opinion_11136635-1"),
        "the run says what it read under the default, got {stdout}"
    );

    let stored = Dataset::open_sqlite(&path).expect("the dataset should open");
    let links = stored
        .links_by_kind(LinkKind::CITES)
        .expect("the links should read");
    for writing in [
        "judicial/case_10670048/opinion_11136635-1",
        "judicial/case_10670048/opinion_11136635-2",
    ] {
        assert!(
            !citing(&links, writing, "199A").is_empty(),
            "{writing} cites section 199A, got {links:?}"
        );
    }
}

#[test]
fn should_read_one_section_when_a_singular_reference_is_followed_by_a_footnote_mark() {
    // Torres's dissent, as the text layer of the PDF gives it: the "3" is the
    // mark of footnote 3, not a second section. "section" names one section;
    // only "sections" names a list.
    let default = default_of(11136635, 10670048).expect("Torres names its default code");
    let text = "It is also supported by former\nsection 199(b)(2), 3 which is duplicated by \
                section 199A(b)(4) nearly\nverbatim.";

    let found = default_code::find_bare(text, &default, &[]);

    let sections: Vec<&Vec<String>> = found.iter().map(|citation| &citation.sections).collect();
    assert_eq!(
        sections,
        [
            &vec!["199(b)(2)".to_string()],
            &vec!["199A(b)(4)".to_string()]
        ]
    );

    // A plural reference is still a list.
    let found = default_code::find_bare("The Interaction of Sections 199A and 280E", &default, &[]);
    assert_eq!(found[0].sections, ["199A", "280E"]);
}
