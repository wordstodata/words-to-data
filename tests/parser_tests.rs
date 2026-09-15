use rstest::rstest;
use words_to_data::document::DocumentNode;
use words_to_data::uslm::UslmFacts;
use words_to_data::uslm::parser::{parse, parse_from_str, parse_with_report};

/// The USLM facts of a node, out of its class payload.
///
/// A USLM identifier and a number belong to the publisher's schema, not to the
/// core, so they are read through the payload the parser wrote (#129).
fn facts(node: &DocumentNode) -> UslmFacts {
    UslmFacts::of(&node.data).expect("a parsed USLM node carries USLM facts")
}

const PL_XML_PATH: &str = "tests/test_data/congress_client_cache/bill/119/hr/1/public_law.xml";

#[test]
fn test_parse_usc_title_7() {
    let result = parse("tests/test_data/usc/2025-07-18/usc07.xml", "2025-07-18");
    assert!(
        result.is_ok(),
        "Failed to parse USC Title 7: {:?}",
        result.err()
    );

    let root = result.unwrap();
    // Root is now uscode container
    assert_eq!(root.data.path.as_ref(), "uscode");
    assert!(
        facts(&root).uslm_id.is_none(),
        "USCode container has no uslm_id"
    );

    // Check that children also have USLM format paths
    // The first child is a Title element
    assert!(!root.children.is_empty());
    let title = &root.children[0];
    assert_eq!(
        facts(title).uslm_id.as_deref().unwrap(),
        "/us/usc/t7",
        "First child (Title) should have uslm_id /us/usc/t7"
    );
    assert_eq!(title.data.path.as_ref(), "uscode/title_7");
}

#[test]
fn test_parse_public_law() {
    let result = parse(PL_XML_PATH, "2025-07-04");
    assert!(
        result.is_ok(),
        "Failed to parse Public Law: {:?}",
        result.err()
    );

    let root = result.unwrap();
    // Check that the root path is in USLM format
    // Note: XML uses "119-21" format (with hyphen)
    assert_eq!(facts(&root).uslm_id.as_deref(), Some("/us/pl/119-21"));

    // Nothing here says the public law is part of the US Code: the node type
    // namespaces it on its own (#129).
    assert_eq!(root.data.node_type.as_str(), "bill.public_law");

    // A child carries the publisher's own identifier, which spells the public
    // law number with a slash where the root spells it with a hyphen: the root
    // has no identifier of its own, so the parser builds one from the
    // `<docNumber>`, and the publisher writes `/us/pl/119/21/tI` below it.
    //
    // The count is asserted first. Until #114 the root had no children at all,
    // and a property asserted of each child was true of every member of an empty
    // list.
    assert_eq!(
        root.children.len(),
        11,
        "the bill holds one section and ten titles"
    );
    for child in &root.children {
        if let Some(uslm_id) = facts(child).uslm_id {
            assert!(
                uslm_id.starts_with("/us/pl/119/21/"),
                "a child carries the publisher's identifier, got {uslm_id}"
            );
        }
    }
}

// Full parse → serialize → deserialize → verify roundtrip
#[rstest]
#[case("01")]
#[case("04")]
#[case("09")]
#[case("26")]
fn test_cross_title_serialization(#[case] title: &str) {
    let path = format!("tests/test_data/usc/2025-07-18/usc{}.xml", title);
    let root =
        parse(&path, "2025-07-18").unwrap_or_else(|_| panic!("Failed to parse usc{}.xml", title));

    // Serialize to JSON
    let json = serde_json::to_string(&root).expect("Failed to serialize to JSON");

    // Deserialize back
    let deserialized: DocumentNode =
        serde_json::from_str(&json).expect("Failed to deserialize from JSON");

    // Verify paths match
    assert_eq!(root.data.path, deserialized.data.path);
    assert_eq!(root.data.node_type, deserialized.data.node_type);
    assert_eq!(facts(&root).uslm_id, facts(&deserialized).uslm_id);
}

// Compare appendix vs regular titles
#[rstest]
#[case("05", "05A")] // Title 5 vs Title 5 Appendix
fn test_appendix_vs_regular_titles(#[case] regular: &str, #[case] appendix: &str) {
    use words_to_data::uslm::DocumentType;

    let regular_path = format!("tests/test_data/usc/2025-07-18/usc{}.xml", regular);
    let appendix_path = format!("tests/test_data/usc/2025-07-18/usc{}.xml", appendix);

    let regular_root = parse(&regular_path, "2025-07-18")
        .unwrap_or_else(|_| panic!("Failed to parse usc{}.xml", regular));

    let appendix_root = parse(&appendix_path, "2025-07-18")
        .unwrap_or_else(|_| panic!("Failed to parse usc{}.xml", appendix));

    // Both roots are uscode containers
    assert_eq!(regular_root.data.path.as_ref(), "uscode");
    assert_eq!(appendix_root.data.path.as_ref(), "uscode");

    // Both should be USC documents
    assert!(matches!(
        facts(&regular_root).document_type,
        DocumentType::USCode { .. }
    ));
    assert!(matches!(
        facts(&appendix_root).document_type,
        DocumentType::USCode { .. }
    ));

    // The first child (title) paths should be different
    let regular_title = &regular_root.children[0];
    let appendix_title = &appendix_root.children[0];
    assert_ne!(regular_title.data.path, appendix_title.data.path);
}

#[test]
fn test_parse_from_str_should_produce_same_result_as_parse() {
    // Load XML manually
    let xml_str = std::fs::read_to_string("tests/test_data/usc/2025-07-18/usc07.xml")
        .expect("Failed to read test file");

    // Parse from string
    let from_str_result = parse_from_str(&xml_str, "2025-07-18");
    assert!(
        from_str_result.is_ok(),
        "parse_from_str failed: {:?}",
        from_str_result.err()
    );

    // Parse from file
    let from_file_result = parse("tests/test_data/usc/2025-07-18/usc07.xml", "2025-07-18");
    assert!(from_file_result.is_ok());

    // Both should produce identical results
    let from_str = from_str_result.unwrap();
    let from_file = from_file_result.unwrap();

    assert_eq!(facts(&from_str).uslm_id, facts(&from_file).uslm_id);
    assert_eq!(from_str.data.path, from_file.data.path);
    assert_eq!(from_str.children.len(), from_file.children.len());
}

#[test]
fn test_parse_text_fields() {
    let result = parse("tests/test_data/usc/2025-07-30/usc26.xml", "2025-07-30")
        .expect("failed to load XML");
    let s174b = result
        .find("uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174/subsection_b")
        .expect("Failed to find S174(b)");
    assert!(s174b.data.heading.is_some());
    assert!(s174b.data.content.is_some());
}

/// Every element in the tree, depth first.
fn walk<'a>(element: &'a DocumentNode, out: &mut Vec<&'a DocumentNode>) {
    out.push(element);
    for child in &element.children {
        walk(child, out);
    }
}

#[test]
fn should_exclude_quoted_amendment_text_when_it_carries_no_quoted_content_wrapper() {
    let doc = parse("tests/test_data/usc/2025-07-30/usc26.xml", "2025-07-30")
        .expect("Error running parser");

    let mut all = Vec::new();
    walk(&doc, &mut all);

    // Quoted statutory text is amendment language, not law in force. The
    // publisher usually wraps it in `quotedContent`, which the parser skips,
    // but nine elements in this title carry no wrapper and are marked only by
    // a quotation mark opening the number (#86).
    let quoted: Vec<String> = all
        .iter()
        .filter(|e| {
            let display = facts(e).number_display;
            let n = display.trim_start();
            n.starts_with('\u{201C}') || n.starts_with('"')
        })
        .map(|e| format!("{} num={}", e.data.path, facts(e).number_display))
        .collect();

    assert!(
        quoted.is_empty(),
        "quoted text should not become a provision, found {}: {:#?}",
        quoted.len(),
        quoted
    );
}

#[test]
fn should_keep_the_real_provision_when_quoted_text_shares_its_number() {
    let doc = parse("tests/test_data/usc/2025-07-30/usc26.xml", "2025-07-30")
        .expect("Error running parser");

    let mut all = Vec::new();
    walk(&doc, &mut all);

    // Section 1563(f)(2): the U.S. Code website renders one paragraph (2),
    // "Operating rules". The quoted "Brother-sister controlled group" beside it
    // is amendment text and must not survive, but the real one must.
    const F2: &str = "uscode/title_26/subtitle_A/chapter_6/subchapter_B/part_II/section_1563/subsection_f/paragraph_2";
    let paragraphs: Vec<&&DocumentNode> = all.iter().filter(|e| &*e.data.path == F2).collect();

    assert_eq!(
        paragraphs.len(),
        1,
        "the site renders one paragraph (2) here, got {:?}",
        paragraphs
            .iter()
            .map(|e| e.data.heading.as_deref())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        paragraphs[0].data.heading.as_deref(),
        Some(" Operating rules"),
        "the surviving paragraph should be the real provision"
    );
}

#[test]
fn should_drop_a_real_provision_the_publisher_nested_inside_a_quoted_block() {
    // A deliberate, known loss, recorded so it is not mistaken for an accident.
    //
    // 26 U.S.C. 1563(f)(5)(B) "Applicable provision" is law, and the U.S. Code
    // website renders it. The publisher nested it inside the quoted block that
    // (f)(5)(A) introduces, and stamped it `/us/usc/t26/s1563/f/2/B`, an
    // identifier for a paragraph it has nothing to do with. Its position and
    // its identifier are both wrong, so there is no sound place to put it.
    //
    // Skipping quoted text takes this with it, because it sits under a quoted
    // parent. That was decided knowingly: salvaging it would mean inventing a
    // location the source does not give. Tracked in #87.
    //
    // If this test starts failing, the parser has begun keeping it. That may be
    // right, but it is a change of decision and wants saying out loud.
    let doc = parse("tests/test_data/usc/2025-07-30/usc26.xml", "2025-07-30")
        .expect("Error running parser");

    let mut all = Vec::new();
    walk(&doc, &mut all);

    let applicable_provision_in_1563 = all.iter().any(|e| {
        e.data.path.contains("section_1563")
            && e.data
                .heading
                .as_deref()
                .is_some_and(|h| h.contains("Applicable provision"))
    });
    assert!(
        !applicable_provision_in_1563,
        "1563(f)(5)(B) is dropped with the quoted block it was nested inside; see #87"
    );

    // The same heading elsewhere is untouched: only the misplaced one goes.
    let elsewhere = all
        .iter()
        .filter(|e| {
            e.data
                .heading
                .as_deref()
                .is_some_and(|h| h.contains("Applicable provision"))
        })
        .count();
    assert_eq!(
        elsewhere, 1,
        "the well-formed 'Applicable provision' at 414(s)(4) should survive"
    );
}

/// A `pLaw` root holds its law in `<main>`, beside material that describes the
/// document. Until #114 the parser stepped into `<main>` for a US Code root
/// only, so a public law parsed to its root alone and the whole body of the bill
/// went with the element the parser could not name.
#[test]
fn should_hold_the_bills_section_and_titles_when_a_public_law_is_parsed() {
    let (root, report) = parse_with_report(PL_XML_PATH, "2025-07-04")
        .expect("the committed public law should parse");

    let children: Vec<_> = root
        .children
        .iter()
        .map(|child| child.data.path.as_ref())
        .collect();
    assert_eq!(
        children,
        [
            "publiclawdocument_119-21/section_1",
            "publiclawdocument_119-21/title_I",
            "publiclawdocument_119-21/title_II",
            "publiclawdocument_119-21/title_III",
            "publiclawdocument_119-21/title_IV",
            "publiclawdocument_119-21/title_V",
            "publiclawdocument_119-21/title_VI",
            "publiclawdocument_119-21/title_VII",
            "publiclawdocument_119-21/title_VIII",
            "publiclawdocument_119-21/title_IX",
            "publiclawdocument_119-21/title_X",
        ],
        "the bill's section and its ten titles are the root's children"
    );

    // `<main>` is a name the parser steps into now, so it raises no warning.
    assert!(
        report.dropped_containers.is_empty(),
        "a public law should drop no container it cannot name, got {:?}",
        report.dropped_containers
    );
}

/// The parser steps into `<main>`, so the root's other children never enter the
/// tree. Each one is named on the report with the reason it was declined: an
/// absence with no reason cannot be told apart from an oversight (`CONTEXT.md`,
/// *Exclusion*).
#[test]
fn should_give_a_reason_when_a_public_law_root_child_is_not_law() {
    let (_root, report) = parse_with_report(PL_XML_PATH, "2025-07-04")
        .expect("the committed public law should parse");

    let declined: Vec<_> = report
        .declined_elements
        .iter()
        .map(|element| element.element_name.as_str())
        .collect();
    assert_eq!(
        declined,
        ["meta", "preface", "legislativeHistory", "endMarker"],
        "every child of the root beside <main> is answered for"
    );

    for element in &report.declined_elements {
        assert_eq!(
            element.parent_path, "publiclawdocument_119-21",
            "a declined element says where it sat"
        );
        assert!(
            !element.reason.is_empty(),
            "{} was declined with no reason",
            element.element_name
        );
    }
}

/// A `uscDoc` root holds `<meta>` beside `<main>`, and the parser steps over it
/// exactly as it steps over the four of a bill. One rule, stated once, for both
/// roots: the step into `<main>` for one root only is what #114 was.
#[test]
fn should_give_a_reason_when_a_us_code_root_child_is_not_law() {
    let (root, report) =
        parse_with_report("tests/test_data/usc/2025-07-18/usc09.xml", "2025-07-18")
            .expect("title 9 should parse");

    let declined: Vec<_> = report
        .declined_elements
        .iter()
        .map(|element| (element.element_name.as_str(), element.reason.as_str()))
        .collect();
    assert_eq!(declined.len(), 1, "title 9 holds <meta> and <main>");
    assert_eq!(declined[0].0, "meta");
    assert!(
        !declined[0].1.is_empty(),
        "<meta> was declined with no reason"
    );

    // The law of the title is where it was. A stated decision is not a warning,
    // so the report is still empty of them.
    assert!(!root.children.is_empty(), "title 9 still parses");
    assert!(
        report.is_empty(),
        "a declined element is not a dropped container, got {:?}",
        report.dropped_containers
    );
}
