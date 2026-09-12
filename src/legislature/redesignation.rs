//! A redesignation: a provision that a bill renumbered.
//!
//! A bill says "redesignating paragraph (3) as paragraph (2)", and the provision
//! stays the same law under a new number. Nothing in the US Code records that.
//! Two release points later, the diff sees a paragraph (2) whose text is
//! unrecognisable and a paragraph (3) that is gone, and reports a rewrite plus a
//! removal — a confident false statement about what the law did.
//!
//! # Why this is a link and not an identity
//!
//! `docs/adr/0001-structural-paths-locate-not-identify.md` recommended giving
//! each provision a stable identity. A provision has nothing stable to hash: its
//! text changes, which is the point of tracking it, and its location changes,
//! which is why identity was wanted. A minted id would move whenever a newly
//! added bill revealed an earlier redesignation, which is the defect
//! `docs/adr/0004-links-are-stored-and-identified-by-what-they-say.md` rejected
//! UUIDs for.
//!
//! A redesignation is therefore a **record**: one `Link` of kind
//! `legislature.redesignated_as`, whose subject is the provision as it was and
//! whose object is the provision as it became. A provision's continuity is a
//! **projection** over those links, computed on demand and never stored
//! (`docs/adr/0007-a-record-is-what-was-said-everything-else-is-derived.md`).
//!
//! # Reading and resolving are separate
//!
//! [`read_clause`] reads the words. It needs no US Code, because the words are
//! the same whatever format the bill arrived in.
//!
//! [`resolve`] turns the words into paths. That needs the section under
//! amendment and the document as it read before the bill, so a path can be
//! checked against law that is really there.
//!
//! A redesignation the text states and this module cannot resolve is
//! **reported**, never dropped: [`RedesignationReport::unresolved`] names each
//! one and says why. Silence is the failure mode here, and it is the rule #110
//! set for unknown elements.
//!
//! # What this does not handle
//!
//! [`crate::legislature::AmendingAction::Move`] — a provision relocated rather
//! than renumbered, such as a section transferred to another title. Nothing in
//! the committed corpus carries one, so there is no real fixture to build
//! against and `CLAUDE.md` forbids inventing one. The model already permits it:
//! a link's two ends carry a work each, so an edge may cross from one work to
//! another. **A sample is needed before this is built.**
//!
//! A redesignation stated against a *container* rather than a section is not
//! handled either. `Part VII of subchapter B of chapter 1 is amended by
//! redesignating section 224 as section 225` renumbers a whole section, and the
//! container under amendment is a part. [`resolve`] starts from a section, so
//! these are reported with [`Reason::NoSectionNamed`]. Three statements in the
//! corpus take that form.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::document::DocumentNode;
use crate::link::{
    Evidence, KindPayload, Link, LinkKind, Provenance, Target, VerificationState,
    amendment_reference,
};
use crate::uslm::ElementType;

/// One step of a provision's address below a section, as a bill writes it.
///
/// `paragraph (3)` is a level and a number. Both halves are needed: the number
/// alone cannot say whether `(A)` is a subparagraph or an item, and a structural
/// path spells the level out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Designation {
    /// The level the bill named: `paragraph` in `paragraph (3)`.
    pub level: ElementType,
    /// The number inside the parentheses: `3`, `c`, `A`, `iii`, `AA`.
    pub number: String,
}

impl Designation {
    pub fn new(level: ElementType, number: impl Into<String>) -> Self {
        Self {
            level,
            number: number.into(),
        }
    }

    /// The path segment this designation writes: `paragraph_3`.
    ///
    /// Built from [`ElementType::path_segment_name`], which is the same frozen
    /// spelling the parser uses, so a path built here and a path read from a
    /// document cannot disagree about what a level is called.
    pub fn path_segment(&self) -> String {
        format!("{}_{}", self.level.path_segment_name(), self.number)
    }
}

impl fmt::Display for Designation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.path_segment())
    }
}

/// One provision renumbered: what it was called, and what it became.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Renumbering {
    pub from: Designation,
    pub to: Designation,
}

impl fmt::Display for Renumbering {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} -> {}", self.from, self.to)
    }
}

/// Why a redesignation the text states could not be turned into two paths.
///
/// Each variant names something a reader can act on. "Could not parse" alone
/// would tell a maintainer nothing about which bills to look at.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    /// The clause names a level this build does not know as a unit of law, such
    /// as `the item relating to section 224`, which is a table-of-sections entry
    /// rather than a provision.
    NotAProvisionLevel(String),
    /// The clause named a level but no designations followed it.
    NoDesignations,
    /// The clause named what a provision was, and not what it became.
    NoNewDesignation,
    /// A range such as `(R) through (Z)` could not be enumerated, because the
    /// numbers at that level do not run in a series this build knows.
    UnknownSeries(String),
    /// Both sides read, and they name different numbers of provisions. Pairing
    /// them would invent a correspondence the bill did not state.
    CountsDiffer { from: usize, to: usize },
    /// No section under amendment was named anywhere above the clause.
    NoSectionNamed,
    /// The amendment changes a table of sections, which is an index of the law
    /// rather than law.
    TableOfSections,
    /// A section was named and nothing said which title of the Code it is in.
    ///
    /// A bare `Section 898` relies on the bill's own "Amendment of 1986 Code"
    /// convention. This build resolves it only where the publisher's own
    /// reference confirms the title, and reports it otherwise, rather than
    /// asserting a title nobody told us.
    NoTitleForSection(String),
    /// The dataset does not hold the section under amendment, so nothing here
    /// can be checked against law that is really there.
    SectionNotHeld(String),
    /// The container the clause sits in is not in the document at that path.
    ContainerNotHeld(String),
    /// One path names more than one provision here, so which one was renumbered
    /// cannot be told apart (`docs/adr/0001-structural-paths-locate-not-identify.md`).
    ContainerIsAmbiguous(String),
    /// The provision the clause renumbers is not at that path in the document
    /// as it read before the bill. Usually an amendment that acts on an earlier
    /// amendment's result, a state no release point holds.
    ProvisionNotHeld(String),
}

impl Reason {
    /// Whether this reason says only "the section belongs to another work".
    ///
    /// The answer every work but one gives when a corpus is swept, and the least
    /// informative one, so it is the one a fold gives up first
    /// ([`RedesignationReport::across_works`]).
    pub fn is_another_title(&self) -> bool {
        matches!(self, Self::SectionNotHeld(_))
    }
}

impl fmt::Display for Reason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAProvisionLevel(word) => write!(f, "`{word}` is not a level of a provision"),
            Self::NoDesignations => write!(f, "no designations followed the level"),
            Self::NoNewDesignation => {
                write!(f, "the clause does not say what the provision became")
            }
            Self::UnknownSeries(level) => {
                write!(f, "the numbers of a {level} do not run in a known series")
            }
            Self::CountsDiffer { from, to } => {
                write!(f, "{from} old designations against {to} new ones")
            }
            Self::NoSectionNamed => write!(f, "no section under amendment was named"),
            Self::TableOfSections => write!(f, "a table of sections, not a provision"),
            Self::NoTitleForSection(section) => {
                write!(f, "nothing says which title holds section {section}")
            }
            Self::SectionNotHeld(id) => write!(f, "the dataset does not hold {id}"),
            Self::ContainerNotHeld(path) => write!(f, "no provision at {path}"),
            Self::ContainerIsAmbiguous(path) => write!(f, "{path} names more than one provision"),
            Self::ProvisionNotHeld(path) => write!(f, "no provision at {path} before the bill"),
        }
    }
}

/// A redesignation as a bill stated it, before any path is resolved.
///
/// The reader of a bill produces these. It knows where in the bill the words
/// sat, which is what supplies the section under amendment and the container,
/// and it knows nothing about the US Code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatedRedesignation {
    /// The amendment this was read out of, by its content hash.
    pub amendment_id: String,
    /// The clause as the bill wrote it, so a reviewer can read the words.
    pub text: String,
    /// The section under amendment, as a USLM identifier: `/us/usc/t26/s898`.
    ///
    /// `None` when the bill named no section this reader could find, which is
    /// reported rather than guessed at.
    pub section: Option<String>,
    /// The steps from that section down to the container, outermost first.
    ///
    /// A citation such as `898(c)` gives a number and no level, so a step's
    /// level is optional and is read from the document when it is absent.
    pub container: Vec<Step>,
    /// What the clause said, when it could be read.
    pub renumberings: Vec<Renumbering>,
    /// Why the clause could not be read, when it could not.
    pub unreadable: Option<Reason>,
}

/// One step down from a section, as a bill named it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Step {
    /// The number: `c`, `3`, `A`, `iii`.
    pub number: String,
    /// The level, when the bill named one. `Section 898(c)` names none;
    /// `in subsection (c)` names one.
    pub level: Option<ElementType>,
}

impl Step {
    pub fn numbered(number: impl Into<String>) -> Self {
        Self {
            number: number.into(),
            level: None,
        }
    }

    pub fn named(level: ElementType, number: impl Into<String>) -> Self {
        Self {
            number: number.into(),
            level: Some(level),
        }
    }
}

/// A redesignation resolved to the two paths it moved a provision between.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Redesignation {
    /// Where the provision was before the bill.
    pub from_path: String,
    /// Where the bill put it.
    pub to_path: String,
    /// The amendment that said so.
    pub amendment_id: String,
    /// The clause, as the bill wrote it.
    pub text: String,
}

/// A redesignation the text states and this build could not resolve.
///
/// Reported rather than dropped. A reader must be able to see that the corpus
/// said something the tool could not place, because otherwise the tool's silence
/// reads as the law's silence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Unresolved {
    pub amendment_id: String,
    /// The clause, as the bill wrote it.
    pub text: String,
    pub reason: Reason,
}

impl fmt::Display for Unresolved {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "warning: redesignation not resolved ({}): {}",
            self.reason, self.text
        )
    }
}

/// What one sweep for redesignations found, and what it could not place.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RedesignationReport {
    pub resolved: Vec<Redesignation>,
    pub unresolved: Vec<Unresolved>,
}

impl RedesignationReport {
    /// How many statements were looked at: resolved plus unresolved.
    pub fn stated(&self) -> usize {
        self.resolved.len() + self.unresolved.len()
    }

    /// Write every unresolved statement to stderr, so it reaches the person who
    /// ran the command.
    ///
    /// The crate carries no logger and the CLI writes its own warnings with
    /// `eprintln!`, so this does the same (`crate::uslm::parser::ParseReport`).
    pub fn warn(&self) {
        for unresolved in &self.unresolved {
            eprintln!("{unresolved}");
        }
    }

    /// Fold another sweep's findings into this one.
    pub fn absorb(&mut self, other: RedesignationReport) {
        self.resolved.extend(other.resolved);
        self.unresolved.extend(other.unresolved);
    }

    /// One view of a corpus, from one report per work.
    ///
    /// A statement resolves in the one work that holds its section and fails in
    /// every other, so concatenating the reports would say the same statement was
    /// unresolved fifty times over. A statement that resolved in any work is
    /// resolved; only one that resolved nowhere is reported, once.
    ///
    /// The reason kept is the most telling one. Every work but one answers
    /// [`Reason::SectionNotHeld`], which says only "this is not my title", and
    /// keeping that would hide the one work that had the section and still could
    /// not place the statement.
    pub fn across_works(reports: impl IntoIterator<Item = RedesignationReport>) -> Self {
        let mut folded = Self::default();
        for report in reports {
            folded.absorb(report);
        }
        // A statement is named by the amendment it came from and the words it
        // was read out of. Two statements in one amendment are different
        // statements, and their words differ, so the pair tells them apart.
        let name_of = |amendment_id: &str, text: &str| (amendment_id.to_string(), text.to_string());
        let placed: std::collections::HashSet<(String, String)> = folded
            .resolved
            .iter()
            .map(|resolved| name_of(&resolved.amendment_id, &resolved.text))
            .collect();

        let mut best: std::collections::BTreeMap<(String, String), Unresolved> =
            std::collections::BTreeMap::new();
        for unresolved in &folded.unresolved {
            let name = name_of(&unresolved.amendment_id, &unresolved.text);
            if placed.contains(&name) {
                continue;
            }
            let keep = match best.get(&name) {
                None => true,
                Some(held) => {
                    held.reason.is_another_title() && !unresolved.reason.is_another_title()
                }
            };
            if keep {
                best.insert(name, unresolved.clone());
            }
        }
        Self {
            resolved: folded.resolved,
            unresolved: best.into_values().collect(),
        }
    }
}

// --- Reading the words ---

/// Every renumbering one redesignating clause states.
///
/// The clause is the amending text around a `redesignate` action. Reading stops
/// at the first thing that is not part of a designation list, so the rest of the
/// sentence — "and by inserting after paragraph (6) the following" — is ignored
/// rather than mistaken for part of the statement.
///
/// # Examples
///
/// ```
/// use words_to_data::legislature::redesignation::read_clause;
///
/// let one = read_clause("by redesignating paragraph (3) as paragraph (2).").unwrap();
/// assert_eq!(one.len(), 1);
/// assert_eq!(one[0].from.path_segment(), "paragraph_3");
/// assert_eq!(one[0].to.path_segment(), "paragraph_2");
///
/// // A list pairs off in the order the clause named them.
/// let two = read_clause(
///     "by redesignating subsections (c) and (d) as subsections (d) and (e), respectively",
/// )
/// .unwrap();
/// assert_eq!(two.len(), 2);
/// assert_eq!(two[1].from.path_segment(), "subsection_d");
/// assert_eq!(two[1].to.path_segment(), "subsection_e");
///
/// // A range is enumerated through the series that level's numbers run in.
/// let range = read_clause(
///     "by redesignating subparagraphs (R) through (Z) as subparagraphs (S) through (AA), respectively",
/// )
/// .unwrap();
/// assert_eq!(range.len(), 9);
/// assert_eq!(range[8].from.path_segment(), "subparagraph_Z");
/// assert_eq!(range[8].to.path_segment(), "subparagraph_AA");
/// ```
pub fn read_clause(clause: &str) -> Result<Vec<Renumbering>, Reason> {
    let rest = after_redesignating(clause).ok_or(Reason::NoDesignations)?;
    let (from_level, rest) = read_level(rest)?;
    let (from_numbers, rest) = read_designations(rest, from_level);
    if from_numbers.is_empty() {
        return Err(Reason::NoDesignations);
    }

    let rest = skip_to_new_designation(rest).ok_or(Reason::NoNewDesignation)?;
    let (to_level, rest) = read_level(rest)?;
    let (to_numbers, _) = read_designations(rest, to_level);
    if to_numbers.is_empty() {
        return Err(Reason::NoNewDesignation);
    }

    let from = expand(&from_numbers, from_level)?;
    let to = expand(&to_numbers, to_level)?;
    if from.len() != to.len() {
        return Err(Reason::CountsDiffer {
            from: from.len(),
            to: to.len(),
        });
    }

    Ok(from
        .into_iter()
        .zip(to)
        .map(|(from, to)| Renumbering {
            from: Designation::new(from_level, from),
            to: Designation::new(to_level, to),
        })
        .collect())
}

/// The text after the word the bill uses to redesignate, with a leading `the`
/// dropped.
fn after_redesignating(clause: &str) -> Option<&str> {
    let lower = clause.to_lowercase();
    let at = lower.find("redesignat")?;
    let rest = clause[at..].split_once(char::is_whitespace)?.1;
    Some(
        rest.trim_start()
            .strip_prefix("the ")
            .unwrap_or(rest.trim_start()),
    )
}

/// The level word at the front of `text`, and what follows it.
///
/// A plural is what a list is written with — `subsections (c) and (d)` — so the
/// trailing `s` comes off before the word is looked up.
fn read_level(text: &str) -> Result<(ElementType, &str), Reason> {
    let text = text.trim_start();
    let end = text
        .find(|c: char| !c.is_ascii_alphabetic())
        .unwrap_or(text.len());
    let word = &text[..end];
    let singular = word.strip_suffix('s').unwrap_or(word);
    match ElementType::from_str(singular) {
        Ok(ElementType::Unknown) | Err(_) => Err(Reason::NotAProvisionLevel(word.to_string())),
        Ok(level) if !is_provision_level(level) => {
            Err(Reason::NotAProvisionLevel(word.to_string()))
        }
        Ok(level) => Ok((level, &text[end..])),
    }
}

/// Whether a level names a unit of law a redesignation can move.
///
/// A `level` is the parser's container for a hierarchy it could not name, and a
/// bill never redesignates one. Listing what is allowed, rather than what is
/// not, keeps a new `ElementType` from silently joining the set.
fn is_provision_level(level: ElementType) -> bool {
    matches!(
        level,
        ElementType::Section
            | ElementType::Subsection
            | ElementType::Paragraph
            | ElementType::Subparagraph
            | ElementType::Clause
            | ElementType::Subclause
            | ElementType::Item
            | ElementType::Subitem
            | ElementType::Subsubitem
    )
}

/// One side of a clause, as the bill wrote it.
#[derive(Debug, PartialEq, Eq)]
enum Written {
    /// `(c) and (d)`, or one designation on its own.
    Listed(Vec<String>),
    /// `(R) through (Z)`.
    Range { first: String, last: String },
}

impl Written {
    /// True when no designation was read at all.
    fn is_empty(&self) -> bool {
        matches!(self, Self::Listed(numbers) if numbers.is_empty())
    }
}

/// The designations at the front of `text`, and what follows them.
///
/// Reading stops at the first token that is not a designation or a word that
/// joins two of them, so the remainder of the sentence is left alone.
fn read_designations(text: &str, level: ElementType) -> (Written, &str) {
    let mut numbers: Vec<String> = Vec::new();
    let mut range = false;
    let mut rest = text;

    loop {
        let trimmed = rest.trim_start_matches([' ', ',']);
        if let Some(after) = strip_word(trimmed, "and") {
            rest = after;
            continue;
        }
        if let Some(after) = strip_word(trimmed, "through") {
            range = true;
            rest = after;
            continue;
        }
        match read_number(trimmed, level) {
            Some((number, after)) => {
                numbers.push(number);
                rest = after;
            }
            None => break,
        }
    }

    match (range, numbers.first(), numbers.last()) {
        (true, Some(first), Some(last)) if numbers.len() == 2 => (
            Written::Range {
                first: first.clone(),
                last: last.clone(),
            },
            rest,
        ),
        _ => (Written::Listed(numbers), rest),
    }
}

/// `text` with `word` removed from the front, when it starts with that whole
/// word.
fn strip_word<'a>(text: &'a str, word: &str) -> Option<&'a str> {
    let rest = text.strip_prefix(word)?;
    match rest.chars().next() {
        None => Some(rest),
        Some(next) if !next.is_ascii_alphanumeric() => Some(rest),
        Some(_) => None,
    }
}

/// One designation at the front of `text`, and what follows it.
///
/// A section is written bare — `redesignating section 224 as section 225` —
/// and every level below it is written in parentheses.
fn read_number(text: &str, level: ElementType) -> Option<(String, &str)> {
    if let Some(inside) = text.strip_prefix('(') {
        let (number, after) = inside.split_once(')')?;
        let number = number.trim();
        let plausible = !number.is_empty()
            && number.len() <= 6
            && number.chars().all(|c| c.is_ascii_alphanumeric());
        return plausible.then(|| (number.to_string(), after));
    }
    if level != ElementType::Section {
        return None;
    }
    let end = text
        .find(|c: char| !c.is_ascii_alphanumeric() && c != '-')
        .unwrap_or(text.len());
    let number = &text[..end];
    let starts_with_digit = number.starts_with(|c: char| c.is_ascii_digit());
    starts_with_digit.then(|| (number.to_string(), &text[end..]))
}

/// The text after the `as` that introduces the new designation.
///
/// A clause can carry an aside between the two halves — "redesignating
/// subsection (g), as amended by this section, as subsection (h)" — so the
/// first `as` is not always the right one. The one that counts is followed by a
/// level word.
fn skip_to_new_designation(text: &str) -> Option<&str> {
    let mut rest = text;
    loop {
        let at = rest.to_lowercase().find(" as ")?;
        let after = &rest[at + " as ".len()..];
        let after = strip_article(after);
        if read_level(after).is_ok() {
            return Some(after);
        }
        rest = &rest[at + " as ".len()..];
    }
}

/// `text` with a leading `a`, `an` or `the` removed.
fn strip_article(text: &str) -> &str {
    for article in ["an ", "a ", "the "] {
        if let Some(rest) = text.strip_prefix(article) {
            return rest;
        }
    }
    text
}

/// Every designation one side of a clause names, in order.
fn expand(written: &Written, level: ElementType) -> Result<Vec<String>, Reason> {
    match written {
        Written::Listed(numbers) => Ok(numbers.clone()),
        Written::Range { first, last } => {
            let series = Series::of(level)
                .ok_or_else(|| Reason::UnknownSeries(level.path_segment_name().to_string()))?;
            series
                .between(first, last)
                .ok_or_else(|| Reason::UnknownSeries(level.path_segment_name().to_string()))
        }
    }
}

/// How the numbers at one level run on.
///
/// The drafting convention, which is what a range such as `(R) through (Z) as
/// (S) through (AA)` relies on: the Code doubles a letter after `(z)`, and
/// numbers a clause in lower-case roman.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Series {
    /// `1`, `2`, `3`.
    Arabic,
    /// `a` … `z`, then `aa`, `bb`.
    LowerLetter,
    /// `A` … `Z`, then `AA`, `BB`.
    UpperLetter,
    /// `i`, `ii`, `iii`.
    LowerRoman,
    /// `I`, `II`, `III`.
    UpperRoman,
}

impl Series {
    /// The series a level's numbers run in, or `None` where this build does not
    /// know.
    ///
    /// Only a range needs this. A list names every designation, so it is read
    /// without any convention at all.
    fn of(level: ElementType) -> Option<Self> {
        match level {
            ElementType::Section | ElementType::Paragraph => Some(Self::Arabic),
            ElementType::Subsection | ElementType::Item => Some(Self::LowerLetter),
            ElementType::Subparagraph => Some(Self::UpperLetter),
            ElementType::Clause => Some(Self::LowerRoman),
            ElementType::Subclause => Some(Self::UpperRoman),
            _ => None,
        }
    }

    /// Every designation from `first` to `last`, both ends included.
    ///
    /// `None` when `last` is not reached: a range that runs away is a range this
    /// build has misread, and a long list of invented designations would be
    /// worse than saying so.
    fn between(self, first: &str, last: &str) -> Option<Vec<String>> {
        /// Long enough for any range the Code writes, short enough that a
        /// misread range stops rather than filling memory.
        const LIMIT: usize = 64;

        let mut run = vec![first.to_string()];
        while run.last().is_some_and(|current| current != last) {
            if run.len() >= LIMIT {
                return None;
            }
            let next = self.next(run.last()?)?;
            run.push(next);
        }
        Some(run)
    }

    /// The designation after this one in the series.
    fn next(self, current: &str) -> Option<String> {
        match self {
            Self::Arabic => Some((current.parse::<u32>().ok()? + 1).to_string()),
            Self::LowerLetter => next_letter_run(current, 'a', 'z'),
            Self::UpperLetter => next_letter_run(current, 'A', 'Z'),
            Self::LowerRoman => Some(roman(from_roman(current)? + 1).to_lowercase()),
            Self::UpperRoman => Some(roman(from_roman(current)? + 1)),
        }
    }
}

/// The letter run after this one: `c` to `d`, `z` to `aa`, `aa` to `bb`.
///
/// The Code repeats one letter rather than counting in base 26, so `(z)` is
/// followed by `(aa)` and never by `(ab)`.
fn next_letter_run(current: &str, first: char, last: char) -> Option<String> {
    let mut letters = current.chars();
    let letter = letters.next()?;
    if !(first..=last).contains(&letter) || letters.any(|other| other != letter) {
        return None;
    }
    match letter == last {
        true => Some(first.to_string().repeat(current.chars().count() + 1)),
        false => Some(
            char::from_u32(letter as u32 + 1)?
                .to_string()
                .repeat(current.chars().count()),
        ),
    }
}

/// A roman numeral as a number, or `None` when it is not one.
fn from_roman(text: &str) -> Option<u32> {
    let digit = |c: char| match c.to_ascii_uppercase() {
        'I' => Some(1),
        'V' => Some(5),
        'X' => Some(10),
        'L' => Some(50),
        'C' => Some(100),
        'D' => Some(500),
        'M' => Some(1000),
        _ => None,
    };
    if text.is_empty() {
        return None;
    }
    let values: Option<Vec<u32>> = text.chars().map(digit).collect();
    let values = values?;
    let mut total = 0;
    for (index, value) in values.iter().enumerate() {
        match values[index + 1..].iter().any(|later| later > value) {
            true => total -= *value as i64,
            false => total += *value as i64,
        }
    }
    u32::try_from(total).ok()
}

/// A number as an upper-case roman numeral.
fn roman(mut value: u32) -> String {
    const DIGITS: [(u32, &str); 13] = [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut written = String::new();
    for (amount, digit) in DIGITS {
        while value >= amount {
            written.push_str(digit);
            value -= amount;
        }
    }
    written
}

// --- Resolving the words to paths ---

/// Every section of a document, by USLM identifier, with the node itself.
///
/// [`crate::citation::resolve::SectionPaths`] answers the same question with a
/// path. A redesignation needs the node as well, because a citation such as
/// `898(c)` names a number and not a level, and only the document can say that
/// `(c)` is a subsection.
#[derive(Debug, Default)]
pub struct SectionIndex<'a> {
    sections: std::collections::BTreeMap<String, Vec<&'a DocumentNode>>,
}

impl<'a> SectionIndex<'a> {
    /// Index every section of one parsed work.
    pub fn of(work: &'a DocumentNode) -> Self {
        let mut index = Self::default();
        index.add(work);
        index
    }

    fn add(&mut self, node: &'a DocumentNode) {
        let is_section = node.data.node_type.local() == ElementType::Section.type_name();
        if is_section
            && let Some(id) = crate::uslm::UslmFacts::of(&node.data).and_then(|facts| facts.uslm_id)
        {
            self.sections
                .entry(plain_dashes(&id))
                .or_default()
                .push(node);
        }
        for child in &node.children {
            self.add(child);
        }
    }

    /// The sections one USLM identifier names, in document order.
    pub fn get(&self, uslm_id: &str) -> &[&'a DocumentNode] {
        self.sections
            .get(&plain_dashes(uslm_id))
            .map_or(&[], Vec::as_slice)
    }

    /// How many identifiers are indexed.
    pub fn len(&self) -> usize {
        self.sections.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sections.is_empty()
    }
}

/// Every dash written as a plain hyphen.
///
/// The publisher sets a section number with an en dash — `/us/usc/t26/s1400Z–1` —
/// and a bill's text types a hyphen. The two name one section, and comparing them
/// byte for byte reported § 1400Z-1 as a section the Code does not hold.
fn plain_dashes(text: &str) -> String {
    text.replace(
        ['\u{2010}', '\u{2011}', '\u{2012}', '\u{2013}', '\u{2014}'],
        "-",
    )
}

/// Turn stated redesignations into paths, against the document as it read
/// before the bill.
///
/// `document` must be the earlier expression. A redesignation moves a provision
/// *from* a path, and that path exists only before the move; checking it there
/// is what keeps a misread clause from becoming a link that points at nothing.
pub fn resolve(stated: &[StatedRedesignation], document: &DocumentNode) -> RedesignationReport {
    let index = SectionIndex::of(document);
    let mut report = RedesignationReport::default();
    for statement in stated {
        resolve_one(statement, &index, &mut report);
    }
    report
}

fn resolve_one(
    statement: &StatedRedesignation,
    index: &SectionIndex,
    report: &mut RedesignationReport,
) {
    let mut reject = |reason: Reason| {
        report.unresolved.push(Unresolved {
            amendment_id: statement.amendment_id.clone(),
            text: statement.text.clone(),
            reason,
        });
    };

    if let Some(reason) = &statement.unreadable {
        reject(reason.clone());
        return;
    }
    let Some(section_id) = &statement.section else {
        reject(Reason::NoSectionNamed);
        return;
    };
    let section = match index.get(section_id) {
        [] => {
            reject(Reason::SectionNotHeld(section_id.clone()));
            return;
        }
        [only] => *only,
        _ => {
            reject(Reason::ContainerIsAmbiguous(section_id.clone()));
            return;
        }
    };

    let container = match walk_down(section, &statement.container) {
        Ok(container) => container,
        Err(reason) => {
            reject(reason);
            return;
        }
    };

    for renumbering in &statement.renumberings {
        let from_path = format!(
            "{}/{}",
            container.data.path,
            renumbering.from.path_segment()
        );
        // The provision must be where the bill says it was. A link built from a
        // path no document holds cannot be checked by the party reading it.
        let held = container
            .children
            .iter()
            .filter(|child| *child.data.path == *from_path)
            .count();
        match held {
            0 => reject(Reason::ProvisionNotHeld(from_path)),
            1 => report.resolved.push(Redesignation {
                to_path: format!("{}/{}", container.data.path, renumbering.to.path_segment()),
                from_path,
                amendment_id: statement.amendment_id.clone(),
                text: statement.text.clone(),
            }),
            _ => reject(Reason::ContainerIsAmbiguous(from_path)),
        }
    }
}

/// The node the steps lead to, below `section`.
fn walk_down<'a>(section: &'a DocumentNode, steps: &[Step]) -> Result<&'a DocumentNode, Reason> {
    let mut current = section;
    for step in steps {
        let matches: Vec<&DocumentNode> = current
            .children
            .iter()
            .filter(|child| step_names(step, &child.data.path))
            .collect();
        current = match matches.as_slice() {
            [only] => only,
            [] => {
                return Err(Reason::ContainerNotHeld(format!(
                    "{}/…_{}",
                    current.data.path, step.number
                )));
            }
            _ => {
                return Err(Reason::ContainerIsAmbiguous(format!(
                    "{}/…_{}",
                    current.data.path, step.number
                )));
            }
        };
    }
    Ok(current)
}

/// Whether a step names the provision at this path.
///
/// A step from a citation carries a number and no level, so the number is what
/// is compared; where the bill did name a level, it must agree.
fn step_names(step: &Step, path: &str) -> bool {
    let Some(segment) = path.rsplit('/').next() else {
        return false;
    };
    let Some((level, number)) = segment.split_once('_') else {
        return false;
    };
    if number != step.number {
        return false;
    }
    match step.level {
        Some(named) => level == named.path_segment_name(),
        None => true,
    }
}

// --- The link a redesignation becomes ---

impl Redesignation {
    /// The link that records this redesignation.
    ///
    /// Subject and object are both [`Target::Change`]: the provision as it was,
    /// and the provision as it became, each with the two dates it was observed
    /// between. A bare provision could not say *when* it moved, and a path is
    /// reused — a paragraph (3) struck today can be added again later — so a
    /// dateless edge would claim a renumbering held for all time
    /// (`docs/adr/0004-links-are-stored-and-identified-by-what-they-say.md`).
    ///
    /// Both ends carry the same work and the same dates. An edge whose two ends
    /// sit in different works is permitted by the model, because a section can
    /// be transferred between titles, and the corpus has none; nothing is built
    /// for it here.
    pub fn link(
        &self,
        work: &crate::dataset::WorkId,
        from_date: &str,
        to_date: &str,
        bill_id: &str,
    ) -> Link {
        let kind = LinkKind::new(LinkKind::REDESIGNATED_AS);
        Link {
            subject: Target::Change {
                work: work.clone(),
                path: self.from_path.clone(),
                from_date: from_date.to_string(),
                to_date: to_date.to_string(),
            },
            object: Target::Change {
                work: work.clone(),
                path: self.to_path.clone(),
                from_date: from_date.to_string(),
                to_date: to_date.to_string(),
            },
            provenance: Provenance {
                source: "rule:bill_redesignation".to_string(),
                method: Some("amendingAction type=redesignate".to_string()),
                // A rule read a sentence a source wrote. The bill asserts the
                // renumbering; the reading of it is a machine's, and no person
                // has confirmed it.
                verification: VerificationState::MachineSuggested,
                evidence: Some(Evidence {
                    // The words the statement was read out of, so a reviewer can
                    // check the reading against them.
                    reasoning: Some(self.text.clone()),
                    ..Evidence::default()
                }),
                raw_score: None,
                // No clock reading. The same bill gives the same answer on any
                // day, so a timestamp would only record when the build ran.
                timestamp: None,
                corroboration: None,
            },
            payload: Some(KindPayload {
                namespace: kind.namespace().to_string(),
                value: serde_json::json!({
                    "bill_id": bill_id,
                    "amendment_id": self.amendment_id,
                    "amendment": amendment_reference(bill_id, &self.amendment_id),
                }),
            }),
            kind,
        }
    }
}
