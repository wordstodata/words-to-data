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
use words_to_data::judicial::{OpinionFacts, PassageFacts};

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

/// Justice Thomas's dissent. It starts on page 457 of 537 U.S.
const DISSENT: &str = "judicial/case_122262/opinion_9434365";

/// CAP's own id of a paragraph, `b693-7`, is kept beside the path, so a later
/// import can match the paragraphs again whatever their paths are.
#[test]
fn should_keep_the_source_paragraph_id_as_a_node_fact_when_harvard_gives_one() {
    let case = boeing();

    let paragraph = node(&case, &format!("{DISSENT}/page_457/paragraph_3"));
    let facts = PassageFacts::of(&paragraph.data).expect("a paragraph carries judicial facts");
    assert_eq!(facts.source_ids, ["b693-7"]);

    let footnote = node(&case, &format!("{MAJORITY}/footnote_12"));
    let facts = PassageFacts::of(&footnote.data).expect("a footnote carries judicial facts");
    assert_eq!(
        facts.source_ids,
        ["b688-6", "b688-7", "b688-8"],
        "a footnote of three paragraphs keeps the id of each"
    );
}

/// Every node under `node`, itself first.
fn all_nodes<'a>(node: &'a DocumentNode, into: &mut Vec<&'a DocumentNode>) {
    into.push(node);
    for child in &node.children {
        all_nodes(child, into);
    }
}

/// The majority says the dissent argues the point "post, at 458-459". So "537
/// U.S. at 459" is a path: page 459 of the case is in the dissent, and it holds
/// the paragraphs that start there.
#[test]
fn should_name_an_existing_path_when_a_pin_cite_names_a_page_of_the_case() {
    let case = boeing();
    let mut every = Vec::new();
    all_nodes(&case.root, &mut every);

    let pages: Vec<&str> = every
        .iter()
        .filter(|node| node.data.path.ends_with("/page_459"))
        .map(|node| &*node.data.path)
        .collect();
    assert_eq!(pages, [format!("{DISSENT}/page_459")]);

    let page = node(&case, &format!("{DISSENT}/page_459"));
    assert!(
        !page.children.is_empty(),
        "the page holds the paragraphs that start on it"
    );
    assert!(
        words(&case, &format!("{DISSENT}/page_459/paragraph_1")).len() > 100,
        "and each paragraph holds its words"
    );
}

/// A footnote is its own node beside the pages, and its words are in no
/// paragraph.
#[test]
fn should_store_each_footnote_as_its_own_node_when_harvard_marks_footnotes() {
    let case = boeing();

    let footnote = words(&case, &format!("{MAJORITY}/footnote_13"));
    assert!(
        footnote.starts_with("In support of its argument that"),
        "got {footnote:.80}"
    );
    assert_eq!(
        node(&case, &format!("{MAJORITY}/footnote_13"))
            .data
            .node_type
            .as_str(),
        "judicial.footnote"
    );

    let mut every = Vec::new();
    all_nodes(node(&case, MAJORITY), &mut every);
    let holding: Vec<&str> = every
        .iter()
        .filter(|node| {
            node.data
                .content
                .as_deref()
                .is_some_and(|text| text.contains("whatever final regulations were ultimately"))
        })
        .map(|node| &*node.data.path)
        .collect();
    assert_eq!(holding, [format!("{MAJORITY}/footnote_13")]);
}

/// Each paragraph says where its words came from: the record and the field,
/// and that the field is OCR.
#[test]
fn should_name_the_record_and_the_field_when_a_paragraph_states_its_provenance() {
    let case = boeing();

    let provenance = node(&case, &format!("{DISSENT}/page_457/paragraph_3"))
        .data
        .provenance
        .as_ref()
        .expect("a paragraph records where its words came from");
    assert_eq!(provenance.source, "courtlistener:opinion/9434365:xml_harvard");
    assert_eq!(
        provenance.verification,
        words_to_data::link::VerificationState::MachineSuggested
    );
}

/// One case built from its cluster and every record the cluster lists.
fn case_of(cluster_id: u64) -> Expression {
    let cluster = cluster(cluster_id);
    let opinions: Vec<OpinionRecord> = cluster.opinion_ids().into_iter().map(opinion).collect();
    case_expression(&cluster, &opinions)
        .expect("the case should build")
        .expression
}

/// The paths of the children of the node at `path`.
fn child_paths(expression: &Expression, path: &str) -> Vec<String> {
    node(expression, path)
        .children
        .iter()
        .map(|child| child.data.path.to_string())
        .collect()
}

/// *Obergefell* is 576 U.S. 644, and Harvard's text of it labels the pages of
/// 135 S. Ct. 2584, a commercial reporter: `*2594`. A path page names a page of
/// the official reporter, so the same pin cite means the same path in every
/// case; this writing has no pages of 576 U.S., so it has no page level at all.
#[test]
fn should_give_paragraph_n_only_when_the_labelled_pages_are_not_the_official_reporter() {
    let case = case_of(2812209);
    let writing = "judicial/case_2812209/opinion_2812209";

    let children = child_paths(&case, writing);
    assert!(
        !children.iter().any(|path| path.contains("/page_")),
        "no page of 135 S. Ct. is a path, got {:?}",
        &children[..children.len().min(5)]
    );
    assert!(
        words(&case, &format!("{writing}/paragraph_2"))
            .starts_with("The Constitution promises"),
        "the paragraphs are counted through the writing"
    );
}

/// *Shami* is 741 F.3d 560. CourtListener's conversion of Harvard's text marks
/// its pages with `<span class="star-pagination" label="564">`, and those are
/// pages of F.3d, the reporter the case is cited in.
#[test]
fn should_put_paragraphs_under_the_reporter_pages_when_the_conversion_marks_star_pagination() {
    let case = case_of(2651100);
    let writing = "judicial/case_2651100/opinion_2651100";

    let opening = words(&case, &format!("{writing}/page_565/paragraph_1"));
    assert!(
        opening.starts_with("*565") && opening.contains("Following the trial"),
        "the paragraph that opens with the mark for 565 keeps the mark, got {opening:.60}"
    );
    let footnote = words(&case, &format!("{writing}/footnote_53"));
    assert!(
        footnote.starts_with('.') && footnote.contains("See Henry v. Commissioner"),
        "the footnote's words, without the mark `53` it opens with, got {footnote:.60}"
    );
}

/// A page segment is a page of one volume of one reporter, and the writing
/// says which, so `page_459` reads as "537 U.S. at 459".
#[test]
fn should_name_the_reporter_volume_whose_pages_the_writing_counts() {
    let pages_of = |case: &Expression, writing: &str| {
        OpinionFacts::of(&node(case, writing).data)
            .expect("a writing carries judicial facts")
            .pages
    };

    assert_eq!(pages_of(&boeing(), DISSENT).as_deref(), Some("537 U.S."));
    assert_eq!(
        pages_of(&case_of(2651100), "judicial/case_2651100/opinion_2651100").as_deref(),
        Some("741 F.3d")
    );
    assert_eq!(
        pages_of(&case_of(2812209), "judicial/case_2812209/opinion_2812209"),
        None,
        "Obergefell's labels count pages of no official reporter"
    );
}

// --- Columbia's archive --------------------------------------------------

/// *Wallace v. City of Fresno*, 42 Cal. 2d 180 (1954), cluster 1197312.
/// CourtListener holds the Court's opinion and an addendum as split records
/// from the Columbia archive, in `html_columbia`, and the whole case as one
/// combined record. Columbia marks a page with
/// `<span class="star-pagination">*Page 182</span>`, gives no paragraph an id,
/// and opens each footnote inside a `<p>` that closes inside the footnote.
const WALLACE: &str = "judicial/case_1197312/opinion_9943788";

#[test]
fn should_put_paragraphs_under_the_reporter_pages_when_columbia_marks_star_pages() {
    let case = case_of(1197312);

    assert!(
        words(&case, &format!("{WALLACE}/page_181/paragraph_1"))
            .starts_with("R.T. Wallace was a member of the police department"),
        "the opinion starts on the page before the first mark, *Page 182"
    );
    assert!(
        words(&case, &format!("{WALLACE}/page_182/paragraph_1"))
            .starts_with("When Wallace entered the police department"),
        "the first paragraph that starts on 182"
    );
    let facts = OpinionFacts::of(&node(&case, WALLACE).data).expect("writing facts");
    assert_eq!(facts.pages.as_deref(), Some("42 Cal. 2d"));

    let provenance = node(&case, &format!("{WALLACE}/page_182/paragraph_1"))
        .data
        .provenance
        .as_ref()
        .expect("a paragraph records where its words came from");
    assert_eq!(provenance.source, "courtlistener:opinion/9943788:html_columbia");
    assert_eq!(
        provenance.verification,
        words_to_data::link::VerificationState::Asserted,
        "a person prepared Columbia's markup; no OCR"
    );
}

/// Columbia's footnotes carry no label but the mark they open with, and here
/// the marks are `*` and `**`.
#[test]
fn should_store_each_footnote_by_its_printed_mark_when_columbia_gives_no_label() {
    let case = case_of(1197312);

    assert!(
        words(&case, &format!("{WALLACE}/footnote_*"))
            .starts_with("Section 2 of the 1923 ordinance provided"),
        "the footnote's words, without its mark"
    );
    assert!(
        words(&case, &format!("{WALLACE}/footnote_**"))
            .starts_with("For the rule with respect to the right of a city"),
    );
    let last = child_paths(&case, WALLACE);
    assert!(
        last.ends_with(&[
            format!("{WALLACE}/footnote_*"),
            format!("{WALLACE}/footnote_**")
        ]),
        "the footnotes follow the pages, got {last:?}"
    );
}
