//! The Code address each amending instruction of a public law acts on, read
//! from the publisher's markup (#248).
//!
//! An address is three things the bill already states:
//!
//! - **the section**, from the amending line — the words before *is amended*;
//! - **the designations below it** that the citation gives: `Section 898(c)`
//!   reaches subsection (c);
//! - **the containers the scope phrases open**: an instruction nested under
//!   *"(1) in paragraph (2)--"* acts inside paragraph (2).
//!
//! This is stage 2 of
//! `docs/adr/0013-matching-is-evidence-first-and-the-batch-calls-no-model.md`.
//!
//! # One resolver
//!
//! The redesignation reader ([`crate::uslm::bill_redesignation`]) asks the same
//! question about one kind of clause, and it asks it here. The walk up through
//! the enclosing levels, the phrases that open a scope and the three forms of
//! citation live in this module once, so the two readers cannot come to
//! different answers about the same sentence. Before #248 three readers of
//! "which section does this amendment name" existed, and they disagreed.
//!
//! # What is resolved here and what is not
//!
//! This module does not touch the US Code, so it cannot say whether the
//! address exists. It says what the bill says. An instruction it cannot
//! address still comes back, with the [`Reason`]. Dropping it would make the
//! tool's silence read as the bill's silence, which is the rule #110 set for
//! unknown elements.
//!
//! # The OLRC is not read yet
//!
//! ADR 0013 says the Office of Law Revision Counsel's classification
//! (`olrc.classified_from` links, #247) corroborates or contradicts an address.
//! No dataset holds those links yet, so this resolver reads the markup alone and
//! works the same with or without them. The comparison is a follow-up.

use std::str::FromStr;
use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::document::DocumentNode;
use crate::legislature::redesignation::{Reason, Step};
use crate::uslm::parser::normalize_quotes;
use crate::uslm::{ElementType, UscReference, UslmFacts};

/// Where one amending instruction acts, or why that could not be read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AmendmentAddress {
    /// The amendment, by the content hash the bill's node carries.
    pub amendment_id: String,
    /// Where in the bill the instruction sits, as a structural path.
    pub path: String,
    /// The instruction's own words, without the levels nested in it and
    /// without the text it enacts.
    pub text: String,
    /// The section acted on, as a USLM identifier: `/us/usc/t26/s898`.
    ///
    /// `None` exactly when [`Self::unresolved`] says why.
    pub section: Option<String>,
    /// The steps from the section down to where the instruction acts,
    /// outermost first: the citation's designations, then the scope phrases.
    pub container: Vec<Step>,
    /// Why no section could be read, when none could.
    pub unresolved: Option<Reason>,
}

/// The address of every amending instruction a stored bill states.
///
/// One entry for each node that carries an amendment, in document order.
///
/// # Examples
///
/// ```
/// use words_to_data::uslm::amendment_address::addresses_in;
/// use words_to_data::uslm::bill_parser::bill_expression;
///
/// let path = "tests/test_data/congress_client_cache/bill/119/hr/1/public_law.xml";
/// let xml = std::fs::read_to_string(path).unwrap();
/// let document = roxmltree::Document::parse(&xml).unwrap();
/// let (bill, _) = bill_expression(&document, "119-hr-1").unwrap();
///
/// assert_eq!(addresses_in("119-hr-1", &bill.root).len(), 603);
/// ```
pub fn addresses_in(_bill_id: &str, root: &DocumentNode) -> Vec<AmendmentAddress> {
    let code_of_1986 = stored_titles_declaring_the_1986_code(root);
    let mut addresses = Vec::new();
    let mut came_through = Vec::new();
    collect_addresses(root, &mut came_through, &code_of_1986, &mut addresses);
    addresses
}

/// Walk the bill, addressing each node that carries an amendment.
///
/// `came_through` is every node from the root down to this one.
fn collect_addresses<'a>(
    node: &'a DocumentNode,
    came_through: &mut Vec<&'a DocumentNode>,
    code_of_1986: &[String],
    addresses: &mut Vec<AmendmentAddress>,
) {
    came_through.push(node);
    if let Some(amendment) = stored_facts(node).and_then(|facts| facts.amendment) {
        addresses.push(address_of(amendment.id, came_through, code_of_1986));
    }
    for child in &node.children {
        collect_addresses(child, came_through, code_of_1986, addresses);
    }
    came_through.pop();
}

/// The address of the instruction at the end of `came_through`.
fn address_of(
    amendment_id: String,
    came_through: &[&DocumentNode],
    code_of_1986: &[String],
) -> AmendmentAddress {
    let scope = Scope::above(came_through);
    let mut address = AmendmentAddress {
        amendment_id,
        path: came_through
            .last()
            .map(|node| node.data.path.to_string())
            .unwrap_or_default(),
        text: scope.clause.clone(),
        section: None,
        container: Vec::new(),
        unresolved: None,
    };
    match scope.section(came_through, code_of_1986) {
        Ok((section, container)) => {
            address.section = Some(section);
            address.container = container;
        }
        Err(reason) => {
            address.container = scope.steps;
            address.unresolved = Some(reason);
        }
    }
    address
}

// --- The walk, shared with the redesignation reader ---

/// What the levels above one clause say about where it acts.
pub(crate) struct Scope<'a> {
    /// The clause's own words: the innermost level's text.
    pub clause: String,
    /// The containers the `in paragraph (2)` phrases open, outermost first.
    pub steps: Vec<Step>,
    /// The words before `is amended`, and the level that says them.
    pub amending_line: Option<(String, &'a DocumentNode)>,
}

impl<'a> Scope<'a> {
    /// Read the levels from the clause at the end of `came_through` upwards,
    /// until one names what it amends.
    pub(crate) fn above(came_through: &[&'a DocumentNode]) -> Self {
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

        let mut steps: Vec<Step> = Vec::new();
        let mut amending_line = None;
        for (depth, level) in levels.iter().enumerate() {
            let text = match depth {
                0 => clause.clone(),
                _ => stored_own_text(level),
            };
            // The clause's own `in clause (i),` prefix counts as well: a bill
            // writes a one-line instruction either way.
            if let Some(step) = leading_in_phrase(&text) {
                steps.push(step);
            }
            if let Some(line) = amending_line_of(&text) {
                amending_line = Some((line, *level));
                break;
            }
        }
        // Read innermost-first, and turned round so the steps read
        // outermost-first like a path.
        steps.reverse();

        Scope {
            clause,
            steps,
            amending_line,
        }
    }

    /// Whether the amending line names a table of sections.
    ///
    /// A table of sections is an index of the law rather than law.
    pub(crate) fn is_table_of_sections(&self) -> bool {
        self.amending_line
            .as_ref()
            .is_some_and(|(line, _)| line.contains("table of sections"))
    }

    /// The section this scope acts in, and every step below it: the
    /// citation's designations first, then the scope phrases.
    pub(crate) fn section(
        &self,
        came_through: &[&DocumentNode],
        code_of_1986: &[String],
    ) -> Result<(String, Vec<Step>), Reason> {
        if self.is_table_of_sections() {
            return Err(Reason::TableOfSections);
        }
        let Some((line, holder)) = &self.amending_line else {
            return Err(Reason::NoSectionNamed);
        };
        let (section, trail) =
            stored_section_under_amendment(line, holder, came_through, code_of_1986)?;
        // The citation's own trail sits above anything the `in` phrases named:
        // `Section 898(c)` reaches the subsection, and `in paragraph (2)` below
        // it reaches further down.
        let mut container: Vec<Step> = trail.into_iter().map(Step::numbered).collect();
        container.extend(self.steps.iter().cloned());
        Ok((section, container))
    }
}

/// The USLM facts of a node, or `None` when it carries none.
fn stored_facts(node: &DocumentNode) -> Option<UslmFacts> {
    UslmFacts::of(&node.data)
}

/// Whether a node is a level of the bill's own hierarchy.
///
/// A `Level` is the parser's structural filler and names nothing, so it opens no
/// scope, exactly as in the markup.
pub(crate) fn is_stored_level(node: &DocumentNode) -> bool {
    node.data.node_type.local() != ElementType::Level.path_segment_name()
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
/// The stored counterpart of the markup reader's own, and the same clause in
/// the same words. See [`crate::uslm::bill_redesignation`] for why the clause
/// scopes itself to one title.
pub(crate) fn stored_titles_declaring_the_1986_code(root: &DocumentNode) -> Vec<String> {
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
/// Three forms, in the order they are trusted.
///
/// 1. `Section 2881a of title 10, United States Code` — the bill says which
///    title, so nothing has to be inferred.
/// 2. `Section 6(o) of the Food and Nutrition Act of 2008 (7 U.S.C. 2015(o))` —
///    the citation numbers a section of an Act, and the publisher's own `<ref>`
///    beside it gives the place in the Code. A reference whose text says `note`
///    is refused: the Act is *not* codified at that section.
/// 3. A bare `Section 898(c)`, first against the publisher's own marginal
///    reference to the same section number — `26 USC 898` — found by climbing
///    the enclosing levels, and then against the bill's References clause,
///    which declares for a whole bill title that a bare section means the
///    Internal Revenue Code.
///
/// Where none of the three answers, [`Reason::NoTitleForSection`]. A title
/// nobody told us is a confident guess, and this is the one place a wrong guess
/// would silently move a provision between titles of the Code.
fn stored_section_under_amendment(
    line: &str,
    holder: &DocumentNode,
    came_through: &[&DocumentNode],
    code_of_1986: &[String],
) -> Result<(String, Vec<String>), Reason> {
    let (number, trail) = citation_in(line).ok_or(Reason::NoSectionNamed)?;

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

// --- Reading the words, shared with the markup reader ---

/// The element type a USLM tag name names, or `Unknown`.
///
/// [`ElementType::from_str`] never fails — an unknown name is `Unknown` — and
/// naming that here keeps the `unwrap` out of the readers.
pub(crate) fn element_type_of(tag_name: &str) -> ElementType {
    ElementType::from_str(tag_name).unwrap_or(ElementType::Unknown)
}

/// One run of whitespace as one space, so a pattern does not have to allow for
/// the line breaks in the markup.
pub(crate) fn collapse_spaces(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The part of a level's text before `is amended`, when it says that.
///
/// This is where a bill names what it is about to change. Everything after it is
/// the instruction, which can mention any number of other provisions.
pub(crate) fn amending_line_of(text: &str) -> Option<String> {
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
pub(crate) fn leading_in_phrase(text: &str) -> Option<Step> {
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

/// The section number an amending line cites, and the designations below it.
///
/// One reading for both readers of a bill, so the stored bill and its markup
/// cannot come to different answers about the same sentence.
///
/// A dash inside the number is any of the dashes the Code prints, and comes
/// back as a hyphen. The bill writes `Section 1400Z–2` with an en dash, the
/// publisher's identifiers write `s1400Z-2`, and a number cut at the dash names
/// § 1400Z, which is a different section (#135, #141).
pub(crate) fn citation_in(line: &str) -> Option<(String, Vec<String>)> {
    static CITATION: LazyLock<Regex> = LazyLock::new(|| {
        let dashes: String = crate::citation::usc::DASHES
            .iter()
            .map(|dash| regex::escape(&dash.to_string()))
            .collect();
        Regex::new(&format!(
            r"(?i)section\s+([0-9][0-9A-Za-z{dashes}]*)\s*((?:\([0-9A-Za-z]{{1,6}}\))*)"
        ))
        .unwrap()
    });
    let cited = CITATION.captures(line)?;
    Some((
        crate::citation::usc::fold_dashes(&cited[1]),
        designations_in(&cited[2]),
    ))
}

/// The title of the US Code an amending line names outright: `of title 10`.
pub(crate) fn title_named_in(line: &str) -> Option<String> {
    static OF_TITLE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)of\s+title\s+([0-9]+[A-Za-z]?)\b").unwrap());
    Some(OF_TITLE.captures(line)?[1].to_string())
}

/// The title of the US Code the Internal Revenue Code of 1986 is.
pub(crate) const INTERNAL_REVENUE_CODE: &str = "26";

/// `/us/usc/t26/s898`, the identifier a parsed section carries.
pub(crate) fn uslm_section_id(title: &str, section: &str) -> String {
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
