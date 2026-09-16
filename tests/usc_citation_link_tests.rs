//! An opinion citing the U.S. Code becomes a link (#52).
//!
//! The dataset under test is the real U.S. Code: titles 1, 25, 26 and 42 of the
//! 2025-07-30 release point, from `tests/test_data/usc`. Every path asserted
//! below is a path that release point actually publishes, en dashes and all.
//!
//! What a citation resolves to is a structural path, because the model has no
//! provision identity yet (#93,
//! `docs/adr/0001-structural-paths-locate-not-identify.md`). Where the dataset
//! does not carry the title, the answer is "out of scope" and never "not found".

use words_to_data::citation::resolve::{Resolution, SectionPaths, resolve};
use words_to_data::citation::{Opinion, cites_links, usc};
use words_to_data::dataset::{Dataset, DatasetMetadata, ExpressionId, WorkId};
use words_to_data::link::{LinkKind, Target, VerificationState};
use words_to_data::storage::{InMemoryStorage, LinkReader};

const RELEASE: &str = "2025-07-30";
const TITLE_1: &str = "tests/test_data/usc/2025-07-30/usc01.xml";
const TITLE_25: &str = "tests/test_data/usc/2025-07-30/usc25.xml";
const TITLE_26: &str = "tests/test_data/usc/2025-07-30/usc26.xml";
const TITLE_42: &str = "tests/test_data/usc/2025-07-30/usc42.xml";

/// Section 174 as the 2025-07-30 release point publishes it.
const SECTION_174: &str = "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174";

/// Section 300gg–11 where the 2025-07-30 release point puts it.
///
/// The publisher writes the dash as an en dash, and the stored path keeps it:
/// #141 folds the dash in a lookup key and changes nothing that is stored.
const SECTION_300GG_11: &str =
    "uscode/title_42/chapter_6A/subchapter_XXV/part_A/subpart_II/section_300gg\u{2013}11";

/// Section 479a–1 where the 2025-07-30 release point puts it. The title holds
/// section 479a as well, which is why #135 read a citation to `479a–1` as
/// nothing rather than as section 479a.
const SECTION_479A_1: &str = "uscode/title_25/chapter_14/subchapter_V/section_479a\u{2013}1";

/// A dataset holding the titles named, and an index of where their sections are.
fn dataset_holding(titles: &[(&str, &str)]) -> (Dataset<InMemoryStorage>, SectionPaths) {
    let mut dataset = Dataset::new(DatasetMetadata {
        name: "U.S.C. citation fixture".to_string(),
        ..Default::default()
    });
    let mut paths = SectionPaths::new();

    for (file, work) in titles {
        dataset
            .add_uslm_xml(file, RELEASE, None)
            .unwrap_or_else(|error| panic!("{file} should parse: {error}"));
        let id = ExpressionId::new(WorkId::new(*work), RELEASE);
        let expression = dataset
            .get_expression(&id)
            .expect("storage should answer")
            .unwrap_or_else(|| panic!("{work} should be held"));
        paths.add_work(&expression.root);
    }

    (dataset, paths)
}

fn obergefell() -> Opinion {
    Opinion::new("2812209", "Obergefell v. Hodges, 576 U.S. 644 (2015)")
}

#[test]
fn should_link_the_opinion_to_the_provision_when_the_dataset_holds_the_title() {
    let (dataset, paths) = dataset_holding(&[(TITLE_26, "uscode/title_26")]);
    let scope = dataset.scope().expect("scope should derive");

    let found = usc::find("Research costs are amortized under 26 U.S.C. § 174 (2018).");
    let citation = found.first().expect("one citation");
    let cited = resolve(citation, &scope, &paths);

    assert_eq!(
        cited[0].uslm_id, "/us/usc/t26/s174",
        "a citation names a title and a section, which is a USLM identifier"
    );
    assert_eq!(
        cited[0].resolution,
        Resolution::Provision {
            paths: vec![SECTION_174.to_string()]
        },
        "the dataset holds title 26, so the citation resolves to where \
         section 174 sits in it"
    );

    let links = cites_links(&obergefell(), citation, &cited);

    assert_eq!(links.len(), 1);
    let link = &links[0];
    assert_eq!(link.kind, LinkKind::new(LinkKind::CITES));
    assert_eq!(link.object, Target::Provision(SECTION_174.to_string()));
    assert_eq!(
        link.subject,
        Target::External {
            reference: "judicial.opinion:2812209".to_string(),
            display: "Obergefell v. Hodges, 576 U.S. 644 (2015)".to_string(),
        },
        "an opinion is not a work in the core model yet, so it is named \
         in the namespace of the link that mentions it"
    );
    assert_eq!(
        link.provenance.verification,
        VerificationState::MachineSuggested,
        "a rule matched some text and no person has looked at it"
    );
    assert_eq!(
        link.provenance
            .evidence
            .as_ref()
            .and_then(|evidence| evidence.reasoning.as_deref()),
        Some("26 U.S.C. § 174"),
        "the matched text is the evidence, so a reviewer can read what the \
         rule read"
    );
    assert_eq!(
        link.payload
            .as_ref()
            .map(|payload| payload.namespace.as_str()),
        Some("judicial"),
        "the payload belongs to the extension that defines the kind"
    );
}

#[test]
fn should_keep_the_subsection_in_the_payload_when_the_citation_names_one() {
    let (dataset, paths) = dataset_holding(&[(TITLE_26, "uscode/title_26")]);
    let scope = dataset.scope().expect("scope should derive");

    let found = usc::find("the election under 26 U.S.C. § 174(a)");
    let citation = found.first().expect("one citation");
    assert_eq!(
        citation.sections,
        ["174(a)"],
        "read whole, brackets and all"
    );

    let cited = resolve(citation, &scope, &paths);
    let links = cites_links(&obergefell(), citation, &cited);

    // The path stops at the section. A citation's own subsection cannot be
    // trusted at that depth -- the published example `981(a)(l)(C)` has a
    // lower-case L where the provision has a paragraph (1) -- so the subsection
    // is recorded as written instead of resolved.
    assert_eq!(links[0].object, Target::Provision(SECTION_174.to_string()));
    assert_eq!(
        links[0].payload.as_ref().expect("a payload").value["section"],
        "174(a)"
    );
}

#[test]
fn should_link_every_section_when_one_citation_names_a_list_of_them() {
    // Title 1 has both a section 1 and a section 2, so both halves of the list
    // resolve. This is the eyecite defect that #52 names: it keeps the first
    // section of a list and loses the rest, which loses a link.
    let (dataset, paths) = dataset_holding(&[(TITLE_1, "uscode/title_1")]);
    let scope = dataset.scope().expect("scope should derive");

    let found = usc::find("See 1 U.S.C. §§ 1, 2.");
    let citation = found.first().expect("one citation");
    let cited = resolve(citation, &scope, &paths);

    assert_eq!(cited.len(), 2, "one citation, two provisions");
    let links = cites_links(&obergefell(), citation, &cited);
    let objects: Vec<&Target> = links.iter().map(|link| &link.object).collect();
    assert_eq!(
        objects,
        vec![
            &Target::Provision("uscode/title_1/chapter_1/section_1".to_string()),
            &Target::Provision("uscode/title_1/chapter_1/section_2".to_string()),
        ],
        "each provision is its own statement, so each is its own link"
    );
    for link in &links {
        assert_eq!(
            link.provenance
                .evidence
                .as_ref()
                .and_then(|evidence| evidence.reasoning.as_deref()),
            Some("1 U.S.C. §§ 1, 2"),
            "the evidence is the whole citation, because the whole citation is \
             what was read"
        );
    }
}

#[test]
fn should_say_out_of_scope_when_the_dataset_does_not_carry_the_title() {
    let (dataset, paths) = dataset_holding(&[(TITLE_1, "uscode/title_1")]);
    let scope = dataset.scope().expect("scope should derive");

    let found = usc::find("brought under 42 U.S.C. §§ 1983, 1988");
    let citation = found.first().expect("one citation");
    let cited = resolve(citation, &scope, &paths);

    // A dataset holding title 1 knows nothing about title 42. Reporting "not
    // found" would tell a reader that no such authority exists.
    assert_eq!(cited.len(), 2);
    for section in &cited {
        assert_eq!(section.resolution, Resolution::OutOfScope, "{section:?}");
    }
    assert!(
        cites_links(&obergefell(), citation, &cited).is_empty(),
        "a link into material the dataset does not hold could not be checked \
         by the party reading it"
    );
}

#[test]
fn should_link_the_opinion_to_the_provision_when_the_section_number_ends_in_a_letter() {
    let (dataset, paths) = dataset_holding(&[(TITLE_26, "uscode/title_26")]);
    let scope = dataset.scope().expect("scope should derive");

    // `26 U.S.C. § 45X`, the advanced manufacturing production credit. Until the
    // section number was allowed a letter this citation read as nothing, so a
    // third of the Code could not be linked to at all (#135). It is the section
    // `docs/adr/0001-structural-paths-locate-not-identify.md` uses for its
    // duplicate-path example, and H.R. 1 amended it.
    let found = usc::find("the credit under 26 U.S.C. \u{a7} 45X(b)(1)(A)");
    let citation = found.first().expect("one citation");
    let cited = resolve(citation, &scope, &paths);

    // The subsection travels as written, and the path stops at the section,
    // because the citation cannot be trusted at that depth.
    assert_eq!(cited[0].section, "45X(b)(1)(A)");
    assert_eq!(cited[0].uslm_id, "/us/usc/t26/s45X");
    assert_eq!(
        cited[0].resolution,
        Resolution::Provision {
            paths: vec![
                "uscode/title_26/subtitle_A/chapter_1/subchapter_A/part_IV/subpart_D/section_45X"
                    .to_string()
            ]
        },
        "the release publishes section 45X"
    );

    let links = cites_links(&obergefell(), citation, &cited);
    assert_eq!(links.len(), 1, "one link, got {links:?}");
}

#[test]
fn should_say_absent_when_the_dataset_holds_the_title_and_it_has_no_such_section() {
    let (dataset, paths) = dataset_holding(&[(TITLE_1, "uscode/title_1")]);
    let scope = dataset.scope().expect("scope should derive");

    // `1 U.S.C.U. §§ 1-2` is one of the published examples. The hyphenated form
    // stays as it is written, because whether it means one section numbered
    // `1-2` or sections 1 through 2 cannot be decided from the text: the code
    // does have sections such as `300gg-11`. Title 1 has no section `1-2`, and
    // saying so is an answer about the law, not about our coverage.
    let found = usc::find("1 U.S.C.U. §§ 1-2");
    let citation = found.first().expect("one citation");
    let cited = resolve(citation, &scope, &paths);

    assert_eq!(cited[0].section, "1-2");
    assert_eq!(cited[0].uslm_id, "/us/usc/t1/s1-2");
    assert_eq!(cited[0].resolution, Resolution::Absent);
    assert!(cites_links(&obergefell(), citation, &cited).is_empty());
}

#[test]
fn should_carry_a_citation_link_through_storage_when_the_dataset_is_saved() {
    let (mut dataset, paths) = dataset_holding(&[(TITLE_1, "uscode/title_1")]);
    let scope = dataset.scope().expect("scope should derive");

    let found = usc::find("under 1 U.S.C. § 1");
    let citation = found.first().expect("one citation");
    let cited = resolve(citation, &scope, &paths);
    for link in cites_links(&obergefell(), citation, &cited) {
        dataset.add_link(link).expect("a link should store");
    }

    let stored = dataset
        .links_by_kind(LinkKind::CITES)
        .expect("links should read back");

    // A link nobody can read back is not a link. The core carries a kind it
    // does not act on (docs/adr/0002-links-live-in-the-core.md).
    assert_eq!(stored.len(), 1);
    assert_eq!(
        stored[0].object,
        Target::Provision("uscode/title_1/chapter_1/section_1".to_string())
    );
    assert_eq!(
        stored[0].provenance.verification,
        VerificationState::MachineSuggested
    );
}

#[test]
fn should_resolve_to_the_published_section_when_prose_writes_the_dash_as_a_hyphen() {
    let (dataset, paths) = dataset_holding(&[(TITLE_42, "uscode/title_42")]);
    let scope = dataset.scope().expect("scope should derive");

    // The publisher indexes this section as `/us/usc/t42/s300gg–11`, with an en
    // dash, and prose writes an ASCII hyphen. The two name one section, so the
    // citation must find it (#141). About 5,000 sections are reachable only if
    // the dash matches, most of them in this title.
    let found = usc::find("a group health plan under 42 U.S.C. \u{a7} 300gg-11 (2018)");
    let citation = found.first().expect("one citation");
    let cited = resolve(citation, &scope, &paths);

    // What the text wrote travels as the text wrote it. Only the key the lookup
    // is made with is folded.
    assert_eq!(cited[0].section, "300gg-11");
    assert_eq!(cited[0].uslm_id, "/us/usc/t42/s300gg-11");
    assert_eq!(
        cited[0].resolution,
        Resolution::Provision {
            paths: vec![SECTION_300GG_11.to_string()]
        },
        "the release publishes this section, so `Absent` is a false statement \
         about the law"
    );

    let links = cites_links(&obergefell(), citation, &cited);
    assert_eq!(links.len(), 1, "one link, got {links:?}");
    assert_eq!(
        links[0].object,
        Target::Provision(SECTION_300GG_11.to_string()),
        "the link names the stored path, which keeps the publisher's en dash"
    );
}

#[test]
fn should_say_absent_when_a_dashed_section_number_is_not_in_the_title() {
    let (dataset, paths) = dataset_holding(&[(TITLE_42, "uscode/title_42")]);
    let scope = dataset.scope().expect("scope should derive");

    // The guard on the fold. Title 42 publishes `300gg–9` and `300gg–91` to
    // `300gg–95`, and no `300gg–99`. A fold that matched loosely would answer
    // with one of the neighbours, which is the false statement this project
    // treats as worse than no answer at all.
    let found = usc::find("a plan under 42 U.S.C. \u{a7} 300gg-99");
    let citation = found.first().expect("one citation");
    let cited = resolve(citation, &scope, &paths);

    assert_eq!(cited[0].uslm_id, "/us/usc/t42/s300gg-99");
    assert_eq!(
        cited[0].resolution,
        Resolution::Absent,
        "the title is held and has no such section, which is an answer about \
         the law"
    );
    assert!(cites_links(&obergefell(), citation, &cited).is_empty());
}

#[test]
fn should_read_and_resolve_the_citation_when_prose_writes_the_dash_as_an_en_dash() {
    let (dataset, paths) = dataset_holding(&[(TITLE_25, "uscode/title_25")]);
    let scope = dataset.scope().expect("scope should derive");

    // Real prose from the notes to the Federal Rules of Criminal Procedure, in
    // `tests/test_data/usc/2025-07-30/usc18a.xml`. #135 read nothing here on
    // purpose, because reading `479a` would have named a different section of
    // the release. Now the whole number is read and the fold resolves it (#141).
    let text = "on a list published in the Federal Register under \
                25 U.S.C. \u{a7}\u{202f}479a\u{2013}1.";

    let (found, report) = usc::find_with_report(text);

    assert_eq!(found.len(), 1, "one citation, got {found:?}");
    let citation = &found[0];
    // The number the text wrote, en dash and all: it is the evidence a reviewer
    // checks the link against.
    assert_eq!(citation.sections, ["479a\u{2013}1"]);
    assert!(report.is_empty(), "nothing was declined, got {report:?}");

    let cited = resolve(citation, &scope, &paths);

    assert_eq!(cited[0].uslm_id, "/us/usc/t25/s479a\u{2013}1");
    assert_eq!(
        cited[0].resolution,
        Resolution::Provision {
            paths: vec![SECTION_479A_1.to_string()]
        },
        "the release publishes section 479a–1"
    );
}
