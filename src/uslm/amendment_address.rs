//! The Code address each amending instruction of a public law acts on, read
//! from the publisher's markup (#248).
//!
//! An address is three things the bill already states:
//!
//! - **the section**, from the amending line — the words before *is amended*,
//!   or before *is repealed* where they name a section (#434);
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
//! the enclosing levels, the phrases that open a scope, the three forms of
//! citation and the words *"Such section"* (#364, #431) live in this module
//! once, so the two readers cannot come to different answers about the same
//! sentence. Before #248 three readers of
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
//! # The OLRC is read by the matcher, not here
//!
//! This resolver reads the markup alone, and works the same with or without the
//! Office of Law Revision Counsel's classification (`olrc.classified_from`
//! links, #247). The matcher reads those links
//! ([`crate::legislature::evidence_matching`]): where this resolver gives an
//! instruction no section, the one section the OLRC classifies the
//! instruction's place in the law to becomes its address (#259). **The markup
//! wins**: an instruction addressed here is never readdressed by the table.
//! The links are read as #247 specifies them:
//!
//! - the subject is the Code **section**'s structural path, never lower;
//! - the object is `olrc.classification:<public law>:<law section>`, such as
//!   `olrc.classification:119-21:71301(a)`, and a range stays one object:
//!   `…:70104(a)-(d)`. That law section is what joins a link to an amendment,
//!   whose own node carries its place in the public law;
//! - the payload, in the `olrc` namespace, is `{"descriptions": [...]}`, a list,
//!   because two rows of a table can give one link.
//!
//! **A note neither corroborates nor gives an address.** A link whose
//! descriptions are all note forms — `nt`, `nts`, `nt [tbl]`, `nt new`,
//! `nt …` — classifies a note under the section, and the dataset holds no
//! notes. It sits on the section's path and says nothing about the section's
//! own text. `prec`, the heading before a section, is the same, and so is
//! `prec new`. For Public Law 119-21, 175 of 635 rows are `nt new` and 10 more
//! are `nt`, so counting them would corroborate addresses nothing supports, and
//! reading them as an address would point a link at a section whose text the
//! instruction never touched: § 70118(a) amends § 11026 of Public Law 115-97,
//! which the table places only as a note under 26 U.S.C. 112.

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
/// assert_eq!(addresses_in(&bill.root).len(), 603);
/// ```
pub fn addresses_in(root: &DocumentNode) -> Vec<AmendmentAddress> {
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
    match scope.address(came_through, code_of_1986) {
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
            steps.extend(leading_in_phrase(&text).into_iter().rev());
            if let Some(line) = amending_line_of(&text).or_else(|| repealing_line_of(&text)) {
                steps.extend(in_phrase_after_amending_line(&text).into_iter().rev());
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
    ///
    /// Where the line names its target as *"Such section"* or *"Such
    /// subsection"*, the target is the unit an earlier amendment of the same
    /// section of the bill named ([`such_unit_address`]), as for
    /// [`Self::address`] (#431).
    ///
    /// Otherwise only the amending line's own citation is read. A renumbering such as
    /// *"redesignating section 224 as section 225 and inserting after section
    /// 223 the following new section: SEC. 224."* renumbers inside the part the
    /// line names, and the new section 224 is not where it acts.
    pub(crate) fn section(
        &self,
        came_through: &[&DocumentNode],
        code_of_1986: &[String],
    ) -> Result<(String, Vec<Step>), Reason> {
        self.section_cited_by(came_through, code_of_1986, citation_in)
    }

    /// The address of a whole amending instruction.
    ///
    /// As [`Self::section`], and where the amending line names no section but
    /// the instruction inserts a whole new one, the new section
    /// ([`new_section_inserted`]).
    fn address(
        &self,
        came_through: &[&DocumentNode],
        code_of_1986: &[String],
    ) -> Result<(String, Vec<Step>), Reason> {
        self.section_cited_by(came_through, code_of_1986, |line| {
            citation_in(line).or_else(|| new_section_inserted(&self.clause, came_through))
        })
    }

    /// The section and the steps below it, for both readers: a line that says
    /// *"Such section"* first, then the line's citation as `cite` reads it.
    fn section_cited_by(
        &self,
        came_through: &[&DocumentNode],
        code_of_1986: &[String],
        cite: impl Fn(&str) -> Option<(String, Vec<String>)>,
    ) -> Result<(String, Vec<Step>), Reason> {
        if self.is_table_of_sections() {
            return Err(Reason::TableOfSections);
        }
        let Some((line, holder)) = &self.amending_line else {
            return Err(Reason::NoAmendingLine);
        };
        if such_unit_named_by(line).is_some() {
            let named = such_unit_address(line, holder, came_through, code_of_1986)?;
            let mut container: Vec<Step> = named.whole.into_iter().map(Step::numbered).collect();
            container.extend(self.steps.iter().cloned());
            return Ok((named.section, container));
        }
        if let Some(found) = section_of_a_list_item(holder, came_through) {
            return found.map(|(section, trail)| {
                let mut container: Vec<Step> = trail.into_iter().map(Step::numbered).collect();
                container.extend(self.steps.iter().cloned());
                (section, container)
            });
        }
        let cited = cite(line).ok_or(Reason::NoSectionNamed)?;
        let (section, trail) =
            stored_section_under_amendment(cited, line, holder, came_through, code_of_1986)?;
        // The citation's own trail sits above anything the `in` phrases named:
        // `Section 898(c)` reaches the subsection, and `in paragraph (2)` below
        // it reaches further down.
        let mut container: Vec<Step> = trail.into_iter().map(Step::numbered).collect();
        container.extend(self.steps.iter().cloned());
        Ok((section, container))
    }
}

// --- "Such section": what an earlier amendment named (#364) ---

/// The unit an amending line names, and where it is.
struct NamedUnit {
    /// The section, as a USLM identifier: `/us/usc/t17/s105`.
    section: String,
    /// The designations the citation itself gives: `Section 9062(j)` gives
    /// `j`. A later *"Such section"* names this unit.
    cited: Vec<String>,
    /// The designations below the section, to the whole unit the line names.
    whole: Vec<String>,
}

/// The unit a line names by what an earlier line named.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SuchUnit {
    /// *"Such section"*.
    Section,
    /// *"Such subsection"*.
    Subsection,
}

impl SuchUnit {
    /// The phrase, as a reason quotes it.
    fn phrase(self) -> &'static str {
        match self {
            Self::Section => "Such section",
            Self::Subsection => "Such subsection",
        }
    }
}

/// The unit a line names as *"Such section"* or *"Such subsection"*, at the
/// end of the words before *is amended*.
///
/// Where the line ends so, that is its target, even when a heading before it
/// cites a section: *"(1) Section 102a.—Such section is further amended—"*.
///
/// > Such section is further amended, in subsection (c)—
///
/// Also where it gives where the phrase starts, so that the units the line
/// names before it can be read: *"Subsection (f)(2) of such section"*.
fn such_unit_named_by(line: &str) -> Option<(SuchUnit, usize)> {
    static SUCH_UNIT: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)\bsuch\s+(section|subsection)\s*,?\s*$")
            .expect("the such-unit pattern must compile")
    });
    let found = SUCH_UNIT.captures(line)?;
    let start = found.get(0).map_or(0, |phrase| phrase.start());
    match found[1].to_lowercase().as_str() {
        "section" => Some((SuchUnit::Section, start)),
        _ => Some((SuchUnit::Subsection, start)),
    }
}

/// The unit the amending line said by `holder` names, when the line names it
/// as *"Such section"* or *"Such subsection"*.
///
/// *Such section* is the section the nearest earlier amending line in the same
/// section of the bill names, as its citation names it: after *"Section
/// 9062(j) of title 10"*, *Such section* is § 9062(j). *Such subsection* is
/// the subsection that line names: after *"Subsection (a) of section 1142"*,
/// it is § 1142(a). The earlier line can say *"Such section"* too, so it is
/// read the same way.
///
/// A unit the line names before the phrase is a step below it, as before a
/// citation: *"Subsection (f)(2) of such section"* reaches (f)(2) of it.
fn such_unit_address(
    line: &str,
    holder: &DocumentNode,
    came_through: &[&DocumentNode],
    code_of_1986: &[String],
) -> Result<NamedUnit, Reason> {
    let (such, start) = such_unit_named_by(line).ok_or(Reason::NoSectionNamed)?;
    let no_antecedent = || Reason::NoAntecedent(such.phrase().to_string());
    let earlier = earlier_line_naming_a_section(holder, came_through).ok_or_else(no_antecedent)?;
    let named = unit_named_by_line(&earlier, code_of_1986)?;
    let mut whole = match such {
        SuchUnit::Section => named.cited.clone(),
        SuchUnit::Subsection => {
            let subsection = named.whole.first().ok_or_else(no_antecedent)?;
            vec![subsection.clone()]
        }
    };
    whole.extend(units_before_the_citation(&line[..start]));
    Ok(NamedUnit {
        section: named.section,
        cited: named.cited,
        whole,
    })
}

/// The unit the amending line of the level at the end of `came_through` names.
fn unit_named_by_line(
    came_through: &[&DocumentNode],
    code_of_1986: &[String],
) -> Result<NamedUnit, Reason> {
    let holder = *came_through.last().ok_or(Reason::NoAmendingLine)?;
    let line = amending_line_of(&stored_own_text(holder)).ok_or(Reason::NoAmendingLine)?;
    if such_unit_named_by(&line).is_some() {
        return such_unit_address(&line, holder, came_through, code_of_1986);
    }
    match citation_in(&line) {
        Some(whole) => {
            let (section, whole) =
                stored_section_under_amendment(whole, &line, holder, came_through, code_of_1986)?;
            let alone = citation_alone_in(&line).ok_or(Reason::NoSectionNamed)?;
            let (_, cited) =
                stored_section_under_amendment(alone, &line, holder, came_through, code_of_1986)?;
            Ok(NamedUnit {
                section,
                cited,
                whole,
            })
        }
        None => Err(Reason::NoSectionNamed),
    }
}

/// The path from the root to the nearest level before `holder`, in the same
/// section of the bill, whose amending line names a section.
fn earlier_line_naming_a_section<'a>(
    holder: &DocumentNode,
    came_through: &[&'a DocumentNode],
) -> Option<Vec<&'a DocumentNode>> {
    let at_holder = came_through
        .iter()
        .rposition(|node| std::ptr::eq(*node, holder))?;
    let bill_section = came_through[..=at_holder].iter().rposition(|node| {
        node.data.node_type.local() == ElementType::Section.path_segment_name()
    })?;
    let mut path = came_through[..bill_section].to_vec();
    let mut nearest = None;
    walk_until(came_through[bill_section], holder, &mut path, &mut nearest);
    nearest
}

/// Walk down from `node` in document order until `holder`, and keep in
/// `nearest` the path to the last level whose amending line names a section.
///
/// Whether the walk came to `holder`.
fn walk_until<'a>(
    node: &'a DocumentNode,
    holder: &DocumentNode,
    path: &mut Vec<&'a DocumentNode>,
    nearest: &mut Option<Vec<&'a DocumentNode>>,
) -> bool {
    if std::ptr::eq(node, holder) {
        return true;
    }
    path.push(node);
    if is_stored_level(node) && line_names_a_section(&stored_own_text(node)) {
        *nearest = Some(path.clone());
    }
    for child in &node.children {
        if walk_until(child, holder, path, nearest) {
            return true;
        }
    }
    path.pop();
    false
}

/// Whether a level's amending line names a section: it cites one, or it says
/// *"Such section"*. A table of sections is not a section.
fn line_names_a_section(text: &str) -> bool {
    amending_line_of(text).is_some_and(|line| {
        !line.contains("table of sections")
            && (citation_in(&line).is_some() || such_unit_named_by(&line).is_some())
    })
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
/// 2. `Section 6(o) of the Food and Nutrition Act of 2008 (7 U.S.C. 2015(o))`,
///    or `Section 321(a)(2) of such Act (19 U.S.C. 1321(a)(2))` — the citation
///    numbers a section of an Act, and the publisher's own `<ref>` beside it
///    gives the place in the Code. A reference whose text says `note` or `et
///    seq.` is refused ([`places_a_section`]): the Act is *not* codified at
///    that section.
/// 3. A bare `Section 898(c)`, first against the publisher's own marginal
///    reference to the same section number — `26 USC 898` — found by climbing
///    the enclosing levels, and then against the bill's References clause,
///    which declares for a whole bill title that a bare section means the
///    Internal Revenue Code.
///
/// Where none of the three answers, [`Reason::NoTitleForSection`]. A title
/// nobody told us is a confident guess, and this is the one place a wrong guess
/// would silently move a provision between titles of the Code.
///
/// A citation of a section **of another law** — `Section 11026(a) of Public
/// Law 115–97`, `Section 2408(g)(1) of the Agriculture Improvement Act of
/// 2018` — is not bare, so form 3 never reads it. Its number is the other
/// law's, and only form 2 can place it in the Code. Where form 2 does not, the
/// answer is [`Reason::SectionOfAnotherLaw`], never a Code section with the
/// other law's number (#259).
fn stored_section_under_amendment(
    cited: (String, Vec<String>),
    line: &str,
    holder: &DocumentNode,
    came_through: &[&DocumentNode],
    code_of_1986: &[String],
) -> Result<(String, Vec<String>), Reason> {
    let (number, trail) = cited;

    if let Some(title) = title_named_in(line) {
        return Ok((uslm_section_id(&title, &number), trail));
    }

    if let Some(placed) = section_of_an_act(line, &number, &trail, stored_usc_references(holder)) {
        return placed;
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

    if let Some(title) = title_of_a_list_above(holder, came_through) {
        return Ok((uslm_section_id(&title, &number), trail));
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

/// Form 2: where the Code places the section of an Act an amending line cites.
///
/// `None` when the line does not cite a section of an Act, and a bare-section
/// reading comes next. `references` are the publisher's references in the level
/// that says the line; only those whose words the line holds count. Where the
/// line cites a section of another law and no reference places it in the Code,
/// the answer is [`Reason::SectionOfAnotherLaw`]: the number is the other
/// law's, which no bare-section reading can turn into the Code's.
///
/// `trail` is what the line states below the section, outermost first: the
/// units it names before the citation and the citation's own designations.
/// The reference is the authority for where the Act sits in the Code. Where
/// its trail starts the line's, the two agree, and the line's trail is kept:
/// `Subsection (c)(7) of section 102A of the National Security Act of 1947
/// (50 U.S.C. 3024)` places the whole of § 102A at § 3024, so the address is
/// 3024(c)(7) (#387). Where the two disagree, the Act's numbering is not the
/// Code's, and the reference's trail is kept.
///
/// Both readers of a bill read form 2 here, so the stored bill and its markup
/// come to the same answer about the same sentence.
pub(crate) fn section_of_an_act(
    line: &str,
    number: &str,
    trail: &[String],
    references: Vec<UscReference>,
) -> Option<Result<(String, Vec<String>), Reason>> {
    let another_law = cites_a_section_of_another_law(line);
    if !line.contains(" of the ") && !another_law {
        return None;
    }
    let in_line: Vec<UscReference> = references
        .into_iter()
        .filter(|reference| line.contains(reference.display.trim()))
        .collect();
    if let Some(reference) = in_line.iter().find(|reference| places_a_section(reference)) {
        let below = if trail.starts_with(&reference.trail) {
            trail.to_vec()
        } else {
            reference.trail.clone()
        };
        return Some(Ok((
            uslm_section_id(&reference.title, &reference.section),
            below,
        )));
    }
    // The publisher placed the Act only as a note or as a range, or the line
    // cites another law and nothing places it.
    (another_law || !in_line.is_empty())
        .then(|| Err(Reason::SectionOfAnotherLaw(number.to_string())))
}

/// Whether a reference to the Code names one section of it.
///
/// `7 U.S.C. 8351 note` names a note, which the dataset does not hold, and
/// `42 U.S.C. 1395 et seq.` names a run of sections starting at § 1395. An Act
/// placed that way is *not* codified at the section the reference starts
/// from.
fn places_a_section(reference: &UscReference) -> bool {
    let display = &reference.display;
    !display.contains("note") && !display.contains("et seq")
}

/// The number of the whole new section an instruction inserts, when it inserts
/// one.
///
/// Such an amendment names a *part* in its amending line, not a section:
///
/// > Part VI of subchapter B of chapter 1 is amended by inserting after section
/// > 174 the following new section:"SEC. 174A. 26 USC 174A. …
///
/// § 174 is only the anchor. The new section states its own number in the text
/// the bill enacts, as a `SEC. 174A.` heading, so that heading is read. Two
/// things must both be there: the phrase *the following new section* in the
/// instruction's own words, and a `SEC. <number>.` heading at the start of the
/// first block it enacts. The title is not read here. The publisher's marginal
/// note `26 USC 174A`, or the bill's References clause, gives it, exactly as for
/// a bare section.
fn new_section_inserted(
    clause: &str,
    came_through: &[&DocumentNode],
) -> Option<(String, Vec<String>)> {
    static SECTION_HEADING: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"^\W*SEC\.\s*([0-9][0-9A-Za-z]*)\s*\."#)
            .expect("the new-section heading pattern must compile")
    });
    if !clause.contains("the following new section") {
        return None;
    }
    let instruction = came_through
        .iter()
        .rev()
        .find_map(|node| stored_facts(node).and_then(|facts| facts.amendment))?;
    let first_block = instruction.enacted_text.first()?;
    let number = SECTION_HEADING.captures(first_block.trim())?[1].to_string();
    Some((number, Vec::new()))
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
///
/// A line that names several provisions says *are each amended*:
/// *"Subparagraphs (A) and (B) of section 1202(d)(1) are each amended"*. A
/// line that names a provision an earlier instruction of the same law changed
/// says *is further amended*: *"Section 6213(g)(2), as amended by this Act, is
/// further amended"* (#259).
pub(crate) fn amending_line_of(text: &str) -> Option<String> {
    let found = AMENDED.find(text)?;
    Some(text[..found.start()].to_string())
}

/// `is amended`, `are each amended`, `is further amended`.
///
/// The publisher's marginal note can sit inside the words: Pub. L. 119-60
/// § 3111(a) prints *"is 10 USC prec. 101. amended"* (#365).
static AMENDED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\b(?:is|are)\s+(?:[0-9]+[A-Za-z]?\s*USC\s*(?:prec\.\s*)?[0-9A-Za-z.\-]*?\s*)?(?:each\s+|further\s+)?amended",
    )
    .expect("the amended pattern must compile")
});

/// The part of a level's text before `is repealed`, when it says that and
/// names a section of the Code there (#434).
///
/// > Section 147 of title 10, United States Code, is repealed.
/// >
/// > Subsection (e) of section 455 of the Higher Education Act of 1965 (20
/// > U.S.C. 1087e(e)) is repealed.
///
/// A repeal names its section as an amendment does. Only a line that cites a
/// section and names the Code itself counts: it names the title, or it cites
/// the Code as `20 U.S.C. 1087e(e)`. Other lines are left to the walk:
///
/// - *"(B) Paragraph (4) is repealed."* under an amending line names a unit
///   of the section that line names.
/// - *"Subsection (a) of section 50262 of Public Law 117–169 30 USC 226. …
///   is repealed, and any provision of law amended … by that subsection is
///   restored"* repeals a provision of another law. The publisher's marginal
///   note `30 USC 226` says where the restored words are, and (a) is the other
///   law's subsection, not one of § 226.
/// - *"Subsection (c) of such section 321 … is repealed"* names a section of
///   an Act the line before named, the Tariff Act of 1930, and not § 321 of
///   the Internal Revenue Code.
fn repealing_line_of(text: &str) -> Option<String> {
    static REPEALED: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"\b(?:is|are)\s+(?:[0-9]+[A-Za-z]?\s*USC\s*(?:prec\.\s*)?[0-9A-Za-z.\-]*?\s*)?(?:hereby\s+)?repealed",
        )
        .expect("the repealed pattern must compile")
    });
    let found = REPEALED.find(text)?;
    let line = &text[..found.start()];
    let names_the_code = title_named_in(line).is_some() || line.contains("U.S.C.");
    (citation_in(line).is_some() && names_the_code).then(|| line.to_string())
}

/// The steps a leading `in subsection (a)` phrase names, outermost first.
///
/// Only at the front of the text, and only with the level spelt out, so a
/// mention further along the instruction — "by inserting after paragraph (6)" —
/// cannot be read as a scope. `in subsection (d)(2)` names two steps: the
/// subsection, and the paragraph in it (#339). Empty when the text opens with
/// no such phrase.
///
/// Empty too where the phrase goes on *of section*: *"in subsection (b) of
/// section 502"* is one citation, and [`citation_in`] reads its units. Read as
/// a scope as well, the unit came twice: 14 U.S.C. 502(b)(b) (#434).
pub(crate) fn leading_in_phrase(text: &str) -> Vec<Step> {
    static IN_PHRASE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?i)^\s*(?:\([0-9A-Za-z]{1,6}\)\s*)?in\s+([a-z]+)\s+((?:\([0-9A-Za-z]{1,6}\))+)",
        )
        .unwrap()
    });
    static OF_SECTION: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)^\s*of\s+section\b").expect("the of-section pattern must compile")
    });
    let Some(found) = IN_PHRASE.captures(text) else {
        return Vec::new();
    };
    let after = found.get(0).map_or("", |phrase| &text[phrase.end()..]);
    if OF_SECTION.is_match(after) {
        return Vec::new();
    }
    let level = match element_type_of(&found[1]) {
        ElementType::Unknown | ElementType::Level => return Vec::new(),
        level => level,
    };
    let mut designations = designations_in(&found[2]).into_iter();
    let Some(first) = designations.next() else {
        return Vec::new();
    };
    let mut steps = vec![Step::named(level, first)];
    steps.extend(designations.map(Step::numbered));
    steps
}

/// The steps an `in subsection (c)` phrase right after *is amended* names,
/// outermost first.
///
/// > Such section is further amended, in subsection (c)—
///
/// The phrase opens a scope for every clause below the line, as a leading
/// phrase does (#339). Only the words right after *is amended* count, so a
/// mention further along the instruction cannot be read as a scope.
///
/// Where the line names its target as *"Such paragraph"* or a smaller unit,
/// the phrase names a unit inside a unit this reader does not name, so it
/// gives no step. *"Such subsection"* is read: it is the subsection an earlier
/// amendment named (#364).
pub(crate) fn in_phrase_after_amending_line(text: &str) -> Vec<Step> {
    static SUCH_UNIT: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)\bsuch\s+(?:paragraph|subparagraph|clause|subclause|item|subitem)\b")
            .expect("the such-unit pattern must compile")
    });
    let Some(found) = AMENDED.find(text) else {
        return Vec::new();
    };
    if SUCH_UNIT.is_match(&text[..found.start()]) {
        return Vec::new();
    }
    let after = text[found.end()..].trim_start().trim_start_matches(',');
    leading_in_phrase(after)
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
///
/// A unit the line names before the section is a step below the citation:
/// *"Paragraph (2) of section 6213(b)"* reaches 6213(b)(2), and *"Paragraph
/// (2) of subsection (f) of section 149"* reaches 149(f)(2) (#339).
pub(crate) fn citation_in(line: &str) -> Option<(String, Vec<String>)> {
    let (number, mut trail) = citation_alone_in(line)?;
    let start = CITATION.find(line).map_or(0, |cited| cited.start());
    trail.extend(units_before_the_citation(&line[..start]));
    Some((number, trail))
}

/// The section number an amending line cites, and the designations the
/// citation itself gives: `Paragraph (2) of section 6213(b)` gives 6213 and
/// `b`.
fn citation_alone_in(line: &str) -> Option<(String, Vec<String>)> {
    let cited = CITATION.captures(line)?;
    Some((
        crate::citation::usc::fold_dashes(&cited[1]),
        designations_in(&cited[2]),
    ))
}

/// The designations of the units the words right before a citation name, the
/// outermost first: `Paragraph (2) of subsection (f) of ` gives `f`, `2`.
///
/// A unit named in a plural, *"Subparagraphs (A) and (B) of"*, is several
/// units, so it gives none. A unit that only places other words, *"the matter
/// preceding paragraph (1) of"*, is not where the instruction acts, so it gives
/// none either.
fn units_before_the_citation(before: &str) -> Vec<String> {
    static UNITS_OF: LazyLock<Regex> = LazyLock::new(|| {
        let unit = r"(?:subsection|paragraph|subparagraph|clause|subclause|item|subitem)\s+(?:\([0-9A-Za-z]{1,6}\))+\s+of\s+";
        Regex::new(&format!(r"(?i)(\S+\s+)?\b((?:{unit})+)$"))
            .expect("the units-before-a-citation pattern must compile")
    });
    static ONE_UNIT: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?:\([0-9A-Za-z]{1,6}\))+").expect("the designation pattern must compile")
    });
    let Some(found) = UNITS_OF.captures(before) else {
        return Vec::new();
    };
    let places_other_words = found.get(1).is_some_and(|word| {
        let word = word.as_str().trim().to_lowercase();
        ["preceding", "following", "before", "after"].contains(&word.as_str())
    });
    if places_other_words {
        return Vec::new();
    }
    // The bill names the innermost unit first.
    ONE_UNIT
        .find_iter(&found[2])
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .flat_map(|unit| designations_in(unit.as_str()))
        .collect()
}

/// `section 898(c)`: a section number, and the designations that follow it.
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

/// Whether the section an amending line cites is a section of another law:
/// the citation is followed at once by `of Public Law`, `of such Act`, or `of
/// the` and the name of an Act.
///
/// Only the words right after the citation count. *"Section 274(o), as added
/// by section 13304 of Public Law 115-97, is amended"* cites § 274(o) of the
/// Code, and the public law is only where that subsection came from. *"of the
/// Internal Revenue Code of 1986"* names the Code itself.
fn cites_a_section_of_another_law(line: &str) -> bool {
    let Some(citation) = CITATION.find(line) else {
        return false;
    };
    let Some(named) = line[citation.end()..].trim_start().strip_prefix("of ") else {
        return false;
    };
    named.starts_with("Public Law")
        || named.starts_with("such Act")
        || (named.starts_with("the ") && !named.starts_with("the Internal Revenue Code"))
}

/// The title of the US Code an amending line names outright: `of title 10`.
///
/// `Section 6213(b)(1) of the Internal Revenue Code of 1986` names title 26
/// outright too. Read as the section of an Act, the publisher's `26 USC 6213`
/// beside it gave the address and lost the citation's (b)(1) (#339).
pub(crate) fn title_named_in(line: &str) -> Option<String> {
    static OF_TITLE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)of\s+title\s+([0-9]+[A-Za-z]?)\b").unwrap());
    if let Some(title) = OF_TITLE.captures(line) {
        return Some(title[1].to_string());
    }
    cites_a_section_of_the_internal_revenue_code(line).then(|| INTERNAL_REVENUE_CODE.to_string())
}

/// Whether the words right after the citation are `of the Internal Revenue
/// Code`.
fn cites_a_section_of_the_internal_revenue_code(line: &str) -> bool {
    CITATION.find(line).is_some_and(|citation| {
        line[citation.end()..]
            .trim_start()
            .starts_with("of the Internal Revenue Code")
    })
}

// --- A list of instructions under a line that names a whole title (#365) ---

/// The title of the Code a line names when it amends a whole title, or a part
/// of one, and cites no section in it.
///
/// > Title 10, United States Code, is amended as follows:
/// > Chapter 173 of title 10, United States Code, is amended—
///
/// The items under such a line each name the section they act on, so the line
/// opens a list of instructions rather than giving one. A line that names an
/// index of the law — *"The analysis for chapter 5 of title 14"* — opens no
/// such list.
pub(crate) fn title_a_list_amends(line: &str) -> Option<String> {
    static WHOLE_TITLE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)\btitle\s+([0-9]+[A-Za-z]?)\s*,\s*United States Code")
            .expect("the whole-title pattern must compile")
    });
    if citation_in(line).is_some() || names_an_index(line) {
        return None;
    }
    WHOLE_TITLE.captures(line).map(|title| title[1].to_string())
}

/// The section one item of such a list names, and the designations below it.
///
/// > (5) In section 183a(h)(3), by striking the semicolon …
/// > (10) In chapter 40, by redesignating section 711 (relating to …) as
/// > section 710a …
/// > (1) Section 2101(2) is amended by striking …
///
/// The item must open with *In* or *Section*, which is how an item names where
/// it acts, and only its words before the first quotation count: a quoted
/// string is words of the Code. An item that edits an index — *"In the
/// analysis for chapter 700 by striking the item relating to section 70006"*
/// — names no section it acts on.
pub(crate) fn section_a_list_item_names(words: &str) -> Option<(String, Vec<String>)> {
    static OPENS_WITH_A_PLACE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)^\W*(?:\([0-9A-Za-z]{1,6}\)\s*)?(?:in|section)\b")
            .expect("the list-item pattern must compile")
    });
    let unquoted = words.split('"').next().unwrap_or_default();
    if !OPENS_WITH_A_PLACE.is_match(unquoted) {
        return None;
    }
    let cited = CITATION.find(unquoted)?;
    if names_an_index(&unquoted[..cited.start()]) {
        return None;
    }
    // `In section 179—` ends the number at the dash after it.
    let (number, trail) = citation_in(unquoted)?;
    Some((number.trim_end_matches('-').to_string(), trail))
}

/// The section an item of a list under a whole title names, when `holder`
/// opens such a list and the clause sits in one of its items (#365).
///
/// > (a) Title 10, United States Code, is amended as follows:
/// > (10) In chapter 40, by redesignating section 711 …
///
/// The list's line gives the title and the item gives the section. `None` when
/// the holder opens no list, or the item names no section, and the line is
/// read as any other.
fn section_of_a_list_item(
    holder: &DocumentNode,
    came_through: &[&DocumentNode],
) -> Option<Result<(String, Vec<String>), Reason>> {
    let title = stored_list_title(holder)?;
    let item = item_below(holder, came_through)?;
    let (number, trail) = section_a_list_item_names(&stored_own_text(item))?;
    Some(Ok((uslm_section_id(&title, &number), trail)))
}

/// The title a stored level's line amends whole, when the level opens a list.
///
/// Only the `chapeau` is read, as the bill's parser reads it when it splits the
/// list ([`crate::uslm::bill_parser`]). A heading such as *"Amending
/// provisions that refer to section 3455(f)"* names words the list changes,
/// not where it acts.
fn stored_list_title(level: &DocumentNode) -> Option<String> {
    let chapeau = normalize_quotes(&collapse_spaces(level.data.chapeau.as_deref()?));
    title_a_list_amends(&amending_line_of(&chapeau)?)
}

/// The level right below `holder` on the way down to the clause.
fn item_below<'a>(
    holder: &DocumentNode,
    came_through: &[&'a DocumentNode],
) -> Option<&'a DocumentNode> {
    let at = came_through
        .iter()
        .position(|node| std::ptr::eq(*node, holder))?;
    came_through[at + 1..]
        .iter()
        .copied()
        .find(|node| is_stored_level(node))
}

/// The title a list above `holder` amends, when one does (#365).
///
/// > (f) Title 46, United States Code, is amended as follows:
/// > (1) Section 2101(2) is amended by striking …
///
/// The item's own line cites a bare section, and the list says which title.
fn title_of_a_list_above(holder: &DocumentNode, came_through: &[&DocumentNode]) -> Option<String> {
    let at = came_through
        .iter()
        .position(|node| std::ptr::eq(*node, holder))?;
    came_through[..at]
        .iter()
        .rev()
        .filter(|node| is_stored_level(node))
        .find(|node| amending_line_of(&stored_own_text(node)).is_some())
        .and_then(|node| stored_list_title(node))
}

/// The chapter an item of such a list opens with: `40` in *"(10) In chapter
/// 40, by redesignating section 711 …"*.
///
/// Title 10 holds two sections numbered 711, one in chapter 40 and one in
/// chapter 41, so the chapter is what tells them apart.
pub(crate) fn chapter_a_list_item_names(words: &str) -> Option<String> {
    static IN_CHAPTER: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)^\W*(?:\([0-9A-Za-z]{1,6}\)\s*)?in\s+chapter\s+([0-9]+[A-Za-z]?)\b")
            .expect("the chapter pattern must compile")
    });
    IN_CHAPTER
        .captures(words)
        .map(|chapter| chapter[1].to_string())
}

/// Whether words name an index of the law: a table of sections, a chapter
/// analysis, or an item of one.
fn names_an_index(words: &str) -> bool {
    let words = words.to_lowercase();
    ["analysis", "table of", "item relating to"]
        .iter()
        .any(|index| words.contains(index))
}

/// The title of the US Code the Internal Revenue Code of 1986 is.
pub(crate) const INTERNAL_REVENUE_CODE: &str = "26";

/// `/us/usc/t26/s898`, the identifier a parsed section carries.
pub(crate) fn uslm_section_id(title: &str, section: &str) -> String {
    format!("/us/usc/t{title}/s{section}")
}

/// The units an instruction's words name after its amending line, each as the
/// steps down to it: the level the words name, then the designations below it.
///
/// *"is amended by striking subsection (g)"* names subsection (g).
/// *"by striking subparagraphs (B)(i)(I) and (C)(i)"* names two units, and each
/// starts at a subparagraph. The scope phrases count as well: *"in clause (i),
/// by striking subclause (II)"* names clause (i) and subclause (II), and the
/// change is in one of them.
///
/// Read these only from words that quote nothing. A quoted string or an
/// enacted block can hold *"paragraph (2)"* as words of the Code, and the
/// bill's text does not say where a quotation ends when one is nested in
/// another.
///
/// Empty when the words name a run, *"subparagraphs (C) through (F)"*: the
/// units between the two ends are named too, and the words do not say which
/// they are. An empty answer names nothing, so nothing is narrowed.
///
/// ```
/// use words_to_data::uslm::amendment_address::units_named_in;
///
/// let units = units_named_in(
///     "Section 115a of title 10, United States Code, is amended by striking subsection (g).",
/// );
/// assert_eq!(units.len(), 1);
/// assert_eq!(units[0][0].number, "g");
/// ```
pub fn units_named_in(words: &str) -> Vec<Vec<Step>> {
    static UNITS: LazyLock<Regex> = LazyLock::new(|| {
        let designation = r"(?:\([0-9A-Za-z]{1,8}\))+";
        Regex::new(&format!(
            r"(?i)\b(subsection|paragraph|subparagraph|clause|subclause|item|subitem)s?\s+({designation}(?:\s*(?:,|,?\s*and|,?\s*or)\s*{designation})*)"
        ))
        .expect("the named-unit pattern must compile")
    });
    static RUN: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)\)\s*through\s+(?:[a-z]+\s+)?\(").expect("the run pattern must compile")
    });
    static ONE_UNIT: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?:\([0-9A-Za-z]{1,8}\))+").expect("the designation pattern must compile")
    });
    let instruction = match amending_line_of(words) {
        Some(line) => &words[line.len()..],
        None => words,
    };
    if RUN.is_match(instruction) {
        return Vec::new();
    }
    let mut units = Vec::new();
    for named in UNITS.captures_iter(instruction) {
        let level = element_type_of(&named[1].to_lowercase());
        for unit in ONE_UNIT.find_iter(&named[2]) {
            let mut designations = designations_in(unit.as_str()).into_iter();
            let Some(first) = designations.next() else {
                continue;
            };
            let mut steps = vec![Step::named(level, first)];
            steps.extend(designations.map(Step::numbered));
            units.push(steps);
        }
    }
    units
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
