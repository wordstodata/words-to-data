//! Reading the redesignations a bill states, out of USLM markup.
//!
//! The words of a redesignation are domain, and
//! [`crate::legislature::redesignation`] reads them. What lives here is the part
//! that belongs to one publisher's markup: *where in the bill* the words sat, and
//! therefore which provision they are about.
//!
//! # Why the markup and not the flat text
//!
//! A bill nests its instructions:
//!
//! ```text
//! Section 163(h)(3)(F) is amended--
//!   (1) in clause (i)--
//!         (B) by redesignating subclauses (III) and (IV) as subclauses (IV) and (V)
//!   (2) by striking clause (ii) and redesignating clauses (iii) and (iv) as ...
//! ```
//!
//! The first redesignation is inside clause (i); the second is not, because
//! item (2) closed that scope. Flattened to one string the two read alike, and a
//! scan backwards for the nearest "in clause (i)" attaches the second one to the
//! wrong provision. The markup says which is which, so the markup is what is
//! read.
//!
//! # What is resolved here and what is not
//!
//! This module answers "which section, and which container inside it". It does
//! not touch the US Code, so it cannot say whether that provision exists;
//! [`crate::legislature::redesignation::resolve`] does that against a document.
//!
//! A statement whose section this module cannot name still comes back, carrying
//! the reason. Dropping it would make the tool's silence read as the corpus's
//! silence, which is the rule #110 set for unknown elements.

use std::str::FromStr;
use std::sync::LazyLock;

use regex::Regex;
use roxmltree::{Document, Node};

use crate::document::DocumentNode;
use crate::io::load_xml_file;
use crate::legislature::redesignation::{Reason, StatedRedesignation, Step, read_clause};
use crate::uslm::parser::{ParseError, normalize_quotes};
use crate::uslm::{ElementType, UscReference, UslmFacts};

pub type Result<T> = std::result::Result<T, ParseError>;

/// Every redesignation a bill states, in document order.
///
/// One entry per `redesignate` action in the markup, whether or not it could be
/// placed. `bill_id` travels through to the links the statements become.
///
/// # Examples
///
/// ```
/// use words_to_data::uslm::bill_redesignation::redesignations_stated_in_file;
///
/// let stated = redesignations_stated_in_file(
///     "119-hr-1",
///     "tests/test_data/congress_client_cache/bill/119/hr/1/public_law.xml",
/// )
/// .unwrap();
/// assert_eq!(stated.len(), 57);
/// ```
pub fn redesignations_stated_in_file(
    bill_id: &str,
    path: &str,
) -> Result<Vec<StatedRedesignation>> {
    let xml = load_xml_file(path)?;
    redesignations_stated(bill_id, &xml)
}

/// Every redesignation a bill states, read from markup already in memory.
pub fn redesignations_stated(bill_id: &str, xml: &str) -> Result<Vec<StatedRedesignation>> {
    Ok(redesignations_stated_in_document(
        bill_id,
        &Document::parse(xml)?,
    ))
}

/// Every redesignation a bill states, read from XML already parsed.
///
/// The same reading as [`redesignations_stated`], from a document the caller
/// already holds. One `roxmltree::Document` then feeds the amendments, the
/// bill's own document and this, so a build reads the bill's XML once
/// (`docs/adr/0009-a-source-is-parsed-once-a-bill-is-a-document.md`).
///
/// # Examples
///
/// ```
/// use words_to_data::uslm::bill_redesignation::redesignations_stated_in_document;
///
/// let path = "tests/test_data/congress_client_cache/bill/119/hr/1/public_law.xml";
/// let xml = std::fs::read_to_string(path).unwrap();
/// let document = roxmltree::Document::parse(&xml).unwrap();
///
/// assert_eq!(redesignations_stated_in_document("119-hr-1", &document).len(), 57);
/// ```
pub fn redesignations_stated_in_document(
    bill_id: &str,
    document: &Document,
) -> Vec<StatedRedesignation> {
    let code_of_1986 = titles_declaring_the_1986_code(document);
    document
        .root()
        .descendants()
        .filter(is_redesignate_action)
        .map(|action| read_action(bill_id, action, &code_of_1986))
        .collect()
}

// --- Reading the bill a dataset holds ---
//
// The same reading as above, from the stored bill rather than from its XML. The
// words of a clause, the phrases that open a scope and the citation that names
// a section are the same either way, so only the walk differs: the markup reader
// climbs `ancestors()`, and this one carries the stack of levels it came down
// through.
//
// What the markup holds and the tree does not travels in the bill node's class
// payload — the amending actions and the US Code references
// (`crate::uslm::UslmFacts`). With those stored, the second read of the bill's
// XML recovers nothing the first one could not keep, which is what ADR 0009
// decided.

/// Every redesignation a bill states, read from the bill a dataset holds.
///
/// The same statements, in the same order, as
/// [`redesignations_stated`] reads out of the same bill's XML.
///
/// # Examples
///
/// ```
/// use words_to_data::uslm::bill_parser::bill_expression;
/// use words_to_data::uslm::bill_redesignation::redesignations_stated_in;
///
/// let path = "tests/test_data/congress_client_cache/bill/119/hr/1/public_law.xml";
/// let xml = std::fs::read_to_string(path).unwrap();
/// let document = roxmltree::Document::parse(&xml).unwrap();
/// let (bill, _) = bill_expression(&document, "119-hr-1").unwrap();
///
/// assert_eq!(redesignations_stated_in("119-hr-1", &bill.root).len(), 57);
/// ```
pub fn redesignations_stated_in(bill_id: &str, root: &DocumentNode) -> Vec<StatedRedesignation> {
    let code_of_1986 = stored_titles_declaring_the_1986_code(root);
    let mut stated = Vec::new();
    let mut came_through = Vec::new();
    read_stored_levels(root, &mut came_through, bill_id, &code_of_1986, &mut stated);
    stated
}

/// Walk the bill, reading each `redesignate` action at the level that states it.
///
/// `came_through` is every node from the root down to this one, which is what
/// the markup reader gets from `ancestors()`.
fn read_stored_levels<'a>(
    node: &'a DocumentNode,
    came_through: &mut Vec<&'a DocumentNode>,
    bill_id: &str,
    code_of_1986: &[String],
    stated: &mut Vec<StatedRedesignation>,
) {
    came_through.push(node);

    // One statement per action, not one per level: a clause that renumbers twice
    // states two redesignations.
    let redesignations = stored_facts(node)
        .map(|facts| {
            facts
                .amending_actions
                .iter()
                .filter(|action| *action == "redesignate")
                .count()
        })
        .unwrap_or(0);
    for _ in 0..redesignations {
        stated.push(read_stored_action(came_through, bill_id, code_of_1986));
    }

    for child in &node.children {
        read_stored_levels(child, came_through, bill_id, code_of_1986, stated);
    }
    came_through.pop();
}

/// The USLM facts of a node, or `None` when it carries none.
fn stored_facts(node: &DocumentNode) -> Option<UslmFacts> {
    UslmFacts::of(&node.data)
}

/// Whether a node is a level of the bill's own hierarchy.
///
/// The stored counterpart of [`is_level`]. A `Level` is the parser's
/// structural filler and names nothing, so it opens no scope, exactly as in the
/// markup.
fn is_stored_level(node: &DocumentNode) -> bool {
    node.data.node_type.local() != ElementType::Level.path_segment_name()
}

/// One statement, read from the level that states it and the levels above it.
fn read_stored_action(
    came_through: &[&DocumentNode],
    bill_id: &str,
    code_of_1986: &[String],
) -> StatedRedesignation {
    let levels: Vec<&DocumentNode> = came_through
        .iter()
        .rev()
        .copied()
        .filter(|node| is_stored_level(node))
        .collect();
    let clause = levels
        .first()
        .map(|node| stored_own_text(node))
        .unwrap_or_default();

    let mut container: Vec<Step> = Vec::new();
    let mut amending_line = None;
    for (depth, level) in levels.iter().enumerate() {
        let text = match depth {
            0 => clause.clone(),
            _ => stored_own_text(level),
        };
        if let Some(step) = leading_in_phrase(&text) {
            container.push(step);
        }
        if let Some(line) = amending_line_of(&text) {
            amending_line = Some((line, *level));
            break;
        }
    }
    container.reverse();

    let mut statement = StatedRedesignation {
        amendment_id: stored_amendment_id(&levels, bill_id),
        text: clause.clone(),
        // The level that states the clause, which is where a reviewer opens the
        // bill to read the words that defeated the reader.
        path: levels.first().map(|node| node.data.path.to_string()),
        section: None,
        container,
        renumberings: Vec::new(),
        unreadable: None,
    };

    if amending_line
        .as_ref()
        .is_some_and(|(line, _)| line.contains("table of sections"))
    {
        statement.unreadable = Some(Reason::TableOfSections);
        return statement;
    }

    match read_clause(&clause) {
        Ok(renumberings) => statement.renumberings = renumberings,
        Err(reason) => {
            statement.unreadable = Some(reason);
            return statement;
        }
    }

    let Some((line, holder)) = amending_line else {
        statement.unreadable = Some(Reason::NoSectionNamed);
        return statement;
    };

    match stored_section_under_amendment(&line, holder, came_through, code_of_1986) {
        Ok((section, trail)) => {
            statement.section = Some(section);
            let mut steps: Vec<Step> = trail.into_iter().map(Step::numbered).collect();
            steps.append(&mut statement.container);
            statement.container = steps;
        }
        Err(reason) => statement.unreadable = Some(reason),
    }
    statement
}

/// The id of the amendment this action belongs to, as the bill stores it.
///
/// The innermost level marked an instruction, whose node already carries the
/// content hash [`crate::uslm::bill_parser`] minted for it. Where no level is an
/// instruction the markup reader hashes the empty string, and so does this.
fn stored_amendment_id(levels: &[&DocumentNode], bill_id: &str) -> String {
    levels
        .iter()
        .find_map(|node| stored_facts(node).and_then(|facts| facts.amendment))
        .map(|amendment| amendment.id)
        .unwrap_or_else(|| crate::uslm::bill_parser::compute_amendment_id(bill_id, ""))
}

/// The words of one stored level, without the levels nested inside it.
///
/// The markup reader takes the level's own text run and leaves the nested levels
/// out. The parser has already split that run into the number and the five text
/// fields, and put the nested levels in `children`, so putting the fields back
/// together in document order is the same words.
fn stored_own_text(node: &DocumentNode) -> String {
    let mut text = stored_facts(node)
        .map(|facts| facts.number_display)
        .unwrap_or_default();
    for field in [
        node.data.heading.as_deref(),
        node.data.chapeau.as_deref(),
        node.data.content.as_deref(),
        node.data.proviso.as_deref(),
        node.data.continuation.as_deref(),
    ] {
        text.push_str(field.unwrap_or_default());
    }
    normalize_quotes(&collapse_spaces(&text))
}

/// The bill titles that declare a bare section to mean the Internal Revenue
/// Code, read from the stored bill.
///
/// The stored counterpart of [`titles_declaring_the_1986_code`], and the same
/// clause in the same words.
fn stored_titles_declaring_the_1986_code(root: &DocumentNode) -> Vec<String> {
    let mut declaring = Vec::new();
    collect_declaring_titles(root, &mut declaring);
    declaring
}

fn collect_declaring_titles(node: &DocumentNode, declaring: &mut Vec<String>) {
    let is_title = node.data.node_type.local() == ElementType::Title.path_segment_name();
    if is_title
        && let Some(identifier) = stored_facts(node).and_then(|facts| facts.uslm_id)
        && stored_declares_the_1986_code(node)
    {
        declaring.push(identifier);
    }
    for child in &node.children {
        collect_declaring_titles(child, declaring);
    }
}

/// Whether a stored title carries the References clause, in its own words.
fn stored_declares_the_1986_code(node: &DocumentNode) -> bool {
    let says_so = node.data.content.as_deref().is_some_and(|content| {
        let text = collapse_spaces(content);
        text.contains("whenever in this title") && text.contains("Internal Revenue Code of 1986")
    });
    says_so || node.children.iter().any(stored_declares_the_1986_code)
}

/// Every US Code reference in a stored subtree, in document order.
fn stored_usc_references(node: &DocumentNode) -> Vec<UscReference> {
    let mut found = stored_facts(node)
        .map(|facts| facts.references)
        .unwrap_or_default();
    for child in &node.children {
        found.extend(stored_usc_references(child));
    }
    found
}

/// The section a stored level's amending line names, and the designations below
/// it.
///
/// The stored counterpart of [`section_under_amendment`], reading the same three
/// forms in the same order of trust.
fn stored_section_under_amendment(
    line: &str,
    holder: &DocumentNode,
    came_through: &[&DocumentNode],
    code_of_1986: &[String],
) -> std::result::Result<(String, Vec<String>), Reason> {
    let cited = citation_in(line).ok_or(Reason::NoSectionNamed)?;
    let (number, trail) = cited;

    if let Some(title) = title_named_in(line) {
        return Ok((uslm_section_id(&title, &number), trail));
    }

    let names_an_act = line.contains(" of the ");
    if names_an_act {
        let codified = stored_usc_references(holder).into_iter().find(|reference| {
            line.contains(reference.display.trim()) && !reference.display.contains("note")
        });
        if let Some(reference) = codified {
            return Ok((
                uslm_section_id(&reference.title, &reference.section),
                reference.trail,
            ));
        }
    }

    // A bare section: the title must come from the publisher, not from us. The
    // walk climbs from the holder, and stops where the markup reader stops —
    // at a section, or at a parent that is not a level.
    let mut at = came_through
        .iter()
        .rposition(|node| std::ptr::eq(*node, holder));
    while let Some(index) = at {
        let node = came_through[index];
        let confirming = stored_usc_references(node)
            .into_iter()
            .find(|reference| reference.section == number);
        if let Some(reference) = confirming {
            return Ok((uslm_section_id(&reference.title, &number), trail));
        }
        if node.data.node_type.local() == ElementType::Section.path_segment_name() {
            break;
        }
        at = index
            .checked_sub(1)
            .filter(|above| is_stored_level(came_through[*above]));
    }

    let in_a_declaring_title = came_through.iter().any(|node| {
        node.data.node_type.local() == ElementType::Title.path_segment_name()
            && stored_facts(node)
                .and_then(|facts| facts.uslm_id)
                .is_some_and(|id| code_of_1986.contains(&id))
    });
    if in_a_declaring_title {
        return Ok((uslm_section_id(INTERNAL_REVENUE_CODE, &number), trail));
    }

    Err(Reason::NoTitleForSection(number))
}

/// The bill titles that declare a bare section reference to mean the Internal
/// Revenue Code, by their USLM identifier.
///
/// A tax bill opens with a References clause: "whenever in this title, an
/// amendment … is expressed in terms of an amendment to … a section …, the
/// reference shall be considered to be made to a section … of the Internal
/// Revenue Code of 1986". That is the bill telling the reader which title of the
/// Code a bare `Section 898` means, so it is read from the bill rather than
/// assumed. `119-hr-1` declares it once, for its title VII.
///
/// The clause scopes itself — "in this title" — so the declaration is recorded
/// against the title that holds it and reaches no further. The same bill
/// declares the Immigration and Nationality Act for another subtitle, which is
/// not the US Code at all, and a rule that ignored scope would read one as the
/// other.
fn titles_declaring_the_1986_code(document: &Document) -> Vec<String> {
    document
        .root()
        .descendants()
        .filter(|node| {
            node.tag_name().name().eq_ignore_ascii_case("title")
                && node.attribute("identifier").is_some()
        })
        .filter(|title| declares_the_1986_code(title))
        .filter_map(|title| title.attribute("identifier").map(str::to_string))
        .collect()
}

/// Whether a bill title carries the References clause, in its own words.
fn declares_the_1986_code(title: &Node) -> bool {
    title
        .descendants()
        .filter(|node| node.tag_name().name().eq_ignore_ascii_case("content"))
        .any(|content| {
            let text = collapse_spaces(
                &content
                    .descendants()
                    .filter(Node::is_text)
                    .filter_map(|node| node.text())
                    .collect::<String>(),
            );
            text.contains("whenever in this title")
                && text.contains("Internal Revenue Code of 1986")
        })
}

/// Whether a node is an `<amendingAction type="redesignate">`.
fn is_redesignate_action(node: &Node) -> bool {
    node.tag_name()
        .name()
        .eq_ignore_ascii_case("amendingAction")
        && node.attribute("type") == Some("redesignate")
}

/// One statement, read from the action and the structure around it.
fn read_action(bill_id: &str, action: Node, code_of_1986: &[String]) -> StatedRedesignation {
    let levels: Vec<Node> = action.ancestors().filter(is_level).collect();
    let clause = levels.first().map(own_text).unwrap_or_default();

    // The container, from `in subsection (a)` phrases. Read innermost-first,
    // because the ancestors are, and turned round at the end so the steps read
    // outermost-first like a path.
    let mut container: Vec<Step> = Vec::new();
    let mut amending_line = None;
    for (depth, level) in levels.iter().enumerate() {
        let text = match depth {
            0 => clause.clone(),
            _ => own_text(level),
        };
        // The clause's own `in clause (i),` prefix counts as well: a bill writes
        // a one-line instruction either way.
        if let Some(step) = leading_in_phrase(&text) {
            container.push(step);
        }
        if let Some(line) = amending_line_of(&text) {
            amending_line = Some((line, *level));
            break;
        }
    }
    container.reverse();

    let mut statement = StatedRedesignation {
        amendment_id: amendment_id_around(bill_id, action),
        text: clause.clone(),
        // Markup carries no structural path. One is generated when the bill
        // becomes a document, and the reader below gives it.
        path: None,
        section: None,
        container,
        renumberings: Vec::new(),
        unreadable: None,
    };

    // A table of sections is an index of the law rather than law, so the whole
    // statement is out of scope. Said before the clause is read, because the
    // clause reads as nonsense — "the item relating to section 224" — and the
    // reason a reader needs is the one about the table.
    if amending_line
        .as_ref()
        .is_some_and(|(line, _)| line.contains("table of sections"))
    {
        statement.unreadable = Some(Reason::TableOfSections);
        return statement;
    }

    match read_clause(&clause) {
        Ok(renumberings) => statement.renumberings = renumberings,
        Err(reason) => {
            statement.unreadable = Some(reason);
            return statement;
        }
    }

    let Some((line, holder)) = amending_line else {
        statement.unreadable = Some(Reason::NoSectionNamed);
        return statement;
    };

    match section_under_amendment(&line, holder, code_of_1986) {
        Ok((section, trail)) => {
            statement.section = Some(section);
            // The citation's own trail sits above anything the `in` phrases
            // named: `Section 898(c)` reaches the subsection, and `in
            // paragraph (2)` below it reaches further down.
            let mut steps: Vec<Step> = trail.into_iter().map(Step::numbered).collect();
            steps.append(&mut statement.container);
            statement.container = steps;
        }
        Err(reason) => statement.unreadable = Some(reason),
    }
    statement
}

// --- Reading the markup ---

/// The id of the amendment this action belongs to.
///
/// The same content hash [`crate::uslm::bill_parser`] mints, over the same text:
/// the innermost enclosing `role="instruction"` element. That is what makes the
/// link's payload point at an amendment the dataset actually holds, rather than
/// at a hash of a sentence nothing else knows.
///
/// A bill nests instructions, so the innermost one is the tightest amendment the
/// parser extracted around this action.
fn amendment_id_around(bill_id: &str, action: Node) -> String {
    let instruction = action
        .ancestors()
        .find(|node| node.attribute("role") == Some("instruction"));
    let text = instruction
        .map(|node| crate::uslm::bill_parser::node_text(&node))
        .unwrap_or_default();
    crate::uslm::bill_parser::compute_amendment_id(bill_id, &text)
}

/// The element type a USLM tag name names, or `Unknown`.
///
/// [`ElementType::from_str`] never fails — an unknown name is `Unknown` — and
/// naming that here keeps the `unwrap` out of the readers below.
fn element_type_of(tag_name: &str) -> ElementType {
    ElementType::from_str(tag_name).unwrap_or(ElementType::Unknown)
}

/// Whether a node is a level of the bill's own hierarchy.
///
/// These are the elements that carry a number and hold others, so they are the
/// ones whose text can open a scope. Anything else — a heading, a sidenote, a
/// `<ref>` — is part of its level's own words.
fn is_level(node: &Node) -> bool {
    if !node.is_element() {
        return false;
    }
    !matches!(
        element_type_of(node.tag_name().name()),
        ElementType::Unknown | ElementType::Level
    )
}

/// The words of one level, without the levels nested inside it.
///
/// A level's own text is the instruction it gives. Its children give their own,
/// and reading them here would fold a whole subtree of instructions into one
/// sentence. Quoted content is skipped for the same reason it is skipped
/// everywhere else: it is the text the bill enacts, not law in force, and it is
/// full of sentences that look like citations.
fn own_text(level: &Node) -> String {
    let mut text = String::new();
    for child in level.children() {
        if child.is_text() {
            text.push_str(child.text().unwrap_or_default());
            continue;
        }
        if is_level(&child)
            || child
                .tag_name()
                .name()
                .eq_ignore_ascii_case("quotedContent")
        {
            continue;
        }
        for descendant in child.descendants().filter(Node::is_text) {
            text.push_str(descendant.text().unwrap_or_default());
        }
    }
    normalize_quotes(&collapse_spaces(&text))
}

/// One run of whitespace as one space, so a pattern does not have to allow for
/// the line breaks in the markup.
fn collapse_spaces(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The part of a level's text before `is amended`, when it says that.
///
/// This is where a bill names what it is about to change. Everything after it is
/// the instruction, which can mention any number of other provisions.
fn amending_line_of(text: &str) -> Option<String> {
    static AMENDED: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\b(?:is|are)\s+amended").unwrap());
    let found = AMENDED.find(text)?;
    Some(text[..found.start()].to_string())
}

/// The step a leading `in subsection (a)` phrase names.
///
/// Only at the front of the text, and only with the level spelt out, so a
/// mention further along the instruction — "by inserting after paragraph (6)" —
/// cannot be read as a scope.
fn leading_in_phrase(text: &str) -> Option<Step> {
    static IN_PHRASE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)^\s*(?:\([0-9A-Za-z]{1,6}\)\s*)?in\s+([a-z]+)\s+\(([0-9A-Za-z]{1,6})\)")
            .unwrap()
    });
    let found = IN_PHRASE.captures(text)?;
    let level = match element_type_of(&found[1]) {
        ElementType::Unknown | ElementType::Level => return None,
        level => level,
    };
    Some(Step::named(level, &found[2]))
}

/// The section a level's amending line names, as a USLM identifier, and the
/// designations the citation gives below it.
///
/// Three forms, in the order they are trusted.
///
/// 1. `Section 2881a of title 10, United States Code` — the bill says which
///    title, so nothing has to be inferred.
/// 2. `Section 6(o) of the Food and Nutrition Act of 2008 (7 U.S.C. 2015(o))` —
///    the citation numbers a section of an Act, and the publisher's own `<ref>`
///    beside it gives the place in the Code. The reference is the authority, so
///    the trail comes from it and not from the Act's numbering. A reference whose
///    text says `note` is refused: the Act is *not* codified at that section,
///    and treating the note's home as the provision would point a link at
///    somebody else's law.
/// 3. A bare `Section 898(c)`, first against the publisher's own marginal
///    reference to the same section number — `26 USC 898` — and then against the
///    bill's References clause, which declares for a whole bill title that a
///    bare section means the Internal Revenue Code
///    ([`titles_declaring_the_1986_code`]).
///
/// Where none of the three answers, the statement is reported with
/// [`Reason::NoTitleForSection`]. A title nobody told us is a confident guess,
/// and this is the one place a wrong guess would silently move a provision
/// between titles of the Code.
fn section_under_amendment(
    line: &str,
    holder: Node,
    code_of_1986: &[String],
) -> std::result::Result<(String, Vec<String>), Reason> {
    let (number, trail) = citation_in(line).ok_or(Reason::NoSectionNamed)?;

    if let Some(title) = title_named_in(line) {
        return Ok((uslm_section_id(&title, &number), trail));
    }

    let names_an_act = line.contains(" of the ");
    if names_an_act && let Some(reference) = codified_reference(holder, line) {
        return Ok((
            uslm_section_id(&reference.title, &reference.section),
            reference.trail,
        ));
    }

    // A bare section: the title must come from the publisher, not from us.
    let mut scope = Some(holder);
    while let Some(node) = scope {
        let confirming = usc_references(node)
            .into_iter()
            .find(|reference| reference.section == number);
        if let Some(reference) = confirming {
            return Ok((uslm_section_id(&reference.title, &number), trail));
        }
        if node.tag_name().name().eq_ignore_ascii_case("section") {
            break;
        }
        scope = node.parent().filter(|parent| is_level(parent));
    }

    // The bill's own References clause, which says for a whole title of the bill
    // that a bare section means the Internal Revenue Code of 1986 — title 26.
    let in_a_declaring_title = holder.ancestors().any(|ancestor| {
        ancestor.tag_name().name().eq_ignore_ascii_case("title")
            && ancestor
                .attribute("identifier")
                .is_some_and(|id| code_of_1986.iter().any(|declared| declared == id))
    });
    if in_a_declaring_title {
        return Ok((uslm_section_id(INTERNAL_REVENUE_CODE, &number), trail));
    }

    Err(Reason::NoTitleForSection(number))
}

/// The section number an amending line cites, and the designations below it.
///
/// One reading for both readers of a bill, so the stored bill and its markup
/// cannot come to different answers about the same sentence.
fn citation_in(line: &str) -> Option<(String, Vec<String>)> {
    static CITATION: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)section\s+([0-9][0-9A-Za-z\-]*)\s*((?:\([0-9A-Za-z]{1,6}\))*)").unwrap()
    });
    let cited = CITATION.captures(line)?;
    Some((cited[1].to_string(), designations_in(&cited[2])))
}

/// The title of the US Code an amending line names outright: `of title 10`.
fn title_named_in(line: &str) -> Option<String> {
    static OF_TITLE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)of\s+title\s+([0-9]+[A-Za-z]?)\b").unwrap());
    Some(OF_TITLE.captures(line)?[1].to_string())
}

/// The title of the US Code the Internal Revenue Code of 1986 is.
const INTERNAL_REVENUE_CODE: &str = "26";

/// `/us/usc/t26/s898`, the identifier a parsed section carries.
fn uslm_section_id(title: &str, section: &str) -> String {
    format!("/us/usc/t{title}/s{section}")
}

/// The numbers in a run of parentheses: `(c)(1)(A)` becomes `c`, `1`, `A`.
fn designations_in(parenthesised: &str) -> Vec<String> {
    parenthesised
        .split(['(', ')'])
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect()
}

/// The reference in an amending line that gives the codified home of the Act it
/// names.
///
/// Taken from the line's own words rather than from the whole element: an
/// instruction quotes the text it enacts, and that text cites other statutes.
fn codified_reference(holder: Node, line: &str) -> Option<UscReference> {
    usc_references(holder).into_iter().find(|reference| {
        line.contains(reference.display.trim()) && !reference.display.contains("note")
    })
}

/// Every US Code reference in a subtree.
///
/// One reading for both readers of a bill: the stored bill carries the same
/// references, split into the same parts by the same function
/// (`crate::uslm::bill_parser::usc_reference`).
fn usc_references(node: Node) -> Vec<UscReference> {
    node.descendants()
        .filter(|child| child.tag_name().name().eq_ignore_ascii_case("ref"))
        .filter_map(|child| crate::uslm::bill_parser::usc_reference(&child))
        .collect()
}
