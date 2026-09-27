//! Whether an amendment's own words name the section its link points into.
//!
//! An amendment that names § 263A and whose link points inside § 263 is suspect
//! on its face, and nothing said so before (#239). Both halves of the question
//! are already parsed: the amendment's words travel on the link, and the section
//! is in the link's path. No model call, and no diff.
//!
//! **Three outcomes, not two.** [`Outcome::CouldNotBeRead`] is its own case. A
//! citation form the extractor declined is not a citation that disagrees, and
//! reporting it as one would manufacture a false fault out of an extractor
//! limitation (#140).
//!
//! **A quoted section is not the section being amended.** Amendment language
//! quotes the text struck, the text inserted, and the positional anchor an
//! insertion is placed after, and none of those three is the provision acted on.
//! Reading one made the check report correct links as suspect — 29 of 40
//! disagreements sampled on the maintainer's own dataset — so
//! [`without_quotations`] blanks every quotation before a word is read. A link
//! whose only section sits inside a quotation is then honestly the third
//! outcome. The one exception is an insertion that states a whole new section,
//! where the quoted text **is** the answer ([`new_section_inserted`]).
//!
//! **It does not decide.** A disagreement is a reason for a person to look, and
//! nothing here is stored (`docs/adr/0007-a-record-is-what-was-said-everything-else-is-derived.md`).
//! An amendment may lawfully name one section and act on a provision in another,
//! because the drafter said so, so the output is a queue and not a fault list.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;

use crate::dataset::DatasetError;
use crate::link::{Link, LinkKind, Target, Window};
use crate::query::{Answer, LinkQuery};
use crate::storage::Storage;

/// What the check found about one link.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// The named section and the path's section are the same.
    Agrees,
    /// They differ. A reviewer should look.
    Disagrees,
    /// The amendment's naming could not be established, so the two sides were
    /// never compared. Never a disagreement.
    CouldNotBeRead,
}

/// One link, and what the check found about it.
#[derive(Debug, Clone, Serialize)]
pub struct Row {
    /// The link, by the short id `settle` accepts.
    ///
    /// The id leads a row because this list is a review queue and the id is the
    /// one field a reviewer copies out of it (#227).
    pub id: String,
    /// The structural path the link points into.
    pub path: String,
    /// The window the link was recorded over.
    ///
    /// Two links for one amendment over two windows are otherwise two rows a
    /// reader cannot tell apart (#184).
    pub window: Option<Window>,
    /// The section the amendment's own words name, when they were read.
    pub named_section: Option<String>,
    /// The section the path sits in.
    pub path_section: Option<String>,
    pub outcome: Outcome,
    /// Why the amendment's naming could not be read, in the reader's own words.
    ///
    /// Only ever set on [`Outcome::CouldNotBeRead`]. A third case with no reason
    /// reads as a silent gap, and the whole point of the case is that the limit
    /// is named (#140).
    pub reason: Option<String>,
}

/// How one window's links came out.
///
/// Per window, because the split between windows is the signal that filed this:
/// one dataset showed 4% of its first window's links disagreeing and 29% of its
/// second window's, and a figure folded over the whole dataset hides that (#239).
#[derive(Debug, Clone, Serialize)]
pub struct WindowTally {
    pub window: Window,
    /// Every amendment link recorded over this window.
    pub checked: usize,
    pub agrees: usize,
    pub disagrees: usize,
    pub could_not_be_read: usize,
    /// The share of this window's links a reviewer is asked to look at.
    ///
    /// A field rather than something a reader works out, so the denominator is
    /// stated rather than guessed: it is **every** link in the window, including
    /// the ones the check could not read. Those are links it did not clear, and
    /// a share taken over the read ones alone would rise as the extractor got
    /// worse. The counts are beside it, so the other reading is there for
    /// whoever wants it.
    ///
    /// Zero for a window with no links, rather than a division by zero.
    pub disagreeing_share: f64,
}

/// What the check found over a dataset.
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    /// One tally per window that carries an amendment link, oldest first.
    pub windows: Vec<WindowTally>,
    /// The queue: every link that asks something of a reviewer, suspect first.
    ///
    /// A link that agrees asks nothing, so it is counted in its window's tally
    /// and not listed. The disagreements lead, and the links whose naming could
    /// not be read follow them: those are a limit of the extractor rather than
    /// a suspect match, and they want a different kind of attention.
    ///
    /// No finer ordering inside either group. Ordering the disagreements by how
    /// well each is corroborated is what a reviewer would want next, and these
    /// links carry no corroboration figure to order them by — `score-amendments`
    /// is what attaches one.
    pub rows: Vec<Row>,
}

/// Check every amendment link a query names.
///
/// The query says **which** links to check — a bill, a window, a path, or any
/// combination of them ([`LinkQuery`]) — and the kind is fixed to
/// `legislature.amended_by`, because the question is about an amendment's own
/// words and no other kind has any.
///
/// **The query's limit is ignored.** A per-window share must be taken over every
/// link in the window, and a fetch that stopped at twenty rows would report the
/// share of a screenful as the share of a window. A caller showing a screenful
/// bounds what it prints, as `redesignation-report` does.
pub fn section_agreement<S: Storage>(
    dataset: &S,
    query: &LinkQuery,
) -> Result<Report, DatasetError> {
    let mut amendments = query.clone();
    amendments.kind = Some(LinkKind::AMENDED_BY.to_string());
    amendments.limit = None;
    let Answer { rows: links, .. } = dataset.links_matching(&amendments)?;

    let checked: Vec<Row> = links.iter().map(check).collect();
    let windows = tally_by_window(&checked);

    // The queue. Every link that agrees is already counted in its window's
    // tally, and listing it would put a row a reviewer must read past in front
    // of the rows they were looking for.
    let mut rows: Vec<Row> = checked
        .into_iter()
        .filter(|row| row.outcome != Outcome::Agrees)
        .collect();
    rows.sort_by(|one, other| {
        queue_place(one.outcome)
            .cmp(&queue_place(other.outcome))
            // Then by where in the Code it sits, so two runs over one dataset
            // give one order.
            .then_with(|| one.path.cmp(&other.path))
            .then_with(|| one.id.cmp(&other.id))
    });

    Ok(Report { windows, rows })
}

/// Where an outcome sits in the queue: the suspect links, then the unread ones.
fn queue_place(outcome: Outcome) -> u8 {
    match outcome {
        Outcome::Disagrees => 0,
        Outcome::CouldNotBeRead => 1,
        // Never queued. Kept exhaustive so that a fourth outcome cannot be
        // added without deciding where it belongs.
        Outcome::Agrees => 2,
    }
}

/// One tally for each window the rows name, oldest first.
///
/// A row whose subject names no window is left out rather than gathered under a
/// window of our choosing: a link with no window cannot answer which window's
/// share it belongs in.
fn tally_by_window(rows: &[Row]) -> Vec<WindowTally> {
    let mut by_window: BTreeMap<Window, WindowTally> = BTreeMap::new();
    for row in rows {
        let Some(window) = row.window.clone() else {
            continue;
        };
        let tally = by_window.entry(window.clone()).or_insert(WindowTally {
            window,
            checked: 0,
            agrees: 0,
            disagrees: 0,
            could_not_be_read: 0,
            disagreeing_share: 0.0,
        });
        tally.checked += 1;
        match row.outcome {
            Outcome::Agrees => tally.agrees += 1,
            Outcome::Disagrees => tally.disagrees += 1,
            Outcome::CouldNotBeRead => tally.could_not_be_read += 1,
        }
    }

    by_window
        .into_values()
        .map(|mut tally| {
            if tally.checked > 0 {
                tally.disagreeing_share = tally.disagrees as f64 / tally.checked as f64;
            }
            tally
        })
        .collect()
}

/// What the check finds about one link.
fn check(link: &Link) -> Row {
    let path = link.subject.path().unwrap_or_default().to_string();
    let path_section = section_in_path(&path);
    let (named_section, reason) = match section_named_in(amendment_words(&link.object)) {
        Naming::Section(section) => (Some(section), None),
        Naming::Unread(why) => (None, Some(why)),
    };

    let outcome = match (&named_section, &path_section) {
        (Some(named), Some(sitting)) if same_section(named, sitting) => Outcome::Agrees,
        (Some(_), Some(_)) => Outcome::Disagrees,
        _ => Outcome::CouldNotBeRead,
    };

    Row {
        id: crate::review::short_id(&link.id()).to_string(),
        path,
        window: link.subject.window(),
        named_section,
        path_section,
        outcome,
        reason,
    }
}

/// The amendment's own words, as the link carries them.
///
/// An amendment link's object is external and its display text is the words the
/// bill wrote that caused this change. Reading them off the link rather than out
/// of the stored bill keeps the check answerable from the links alone, which is
/// what makes it cheap.
fn amendment_words(object: &Target) -> &str {
    match object {
        Target::External { display, .. } => display,
        _ => "",
    }
}

/// Whether two spellings of a section number name one section.
///
/// Case-folded, because 10,085 of the Code's section numbers end in a letter and
/// a bill may write `45X` where a path segment carries `45x`.
///
/// Dash-folded on **both** sides. The Code prints `300gg–11` with an en dash and
/// prose writes `300gg-11` with a hyphen, and comparing the two character by
/// character answers that the Code has no such section (#141). Folding one side
/// only moves the mismatch, which is what
/// [`crate::citation::usc::fold_dashes`] says in its own note.
fn same_section(one: &str, other: &str) -> bool {
    let fold = |section: &str| crate::citation::usc::fold_dashes(section).to_ascii_lowercase();
    fold(one) == fold(other)
}

/// The section a structural path sits in: `263` in `…/section_263/subsection_a`.
///
/// `None` for a path above a section, such as a whole part of a title. Nothing
/// is compared there, because a path that sits in no section cannot disagree
/// with one.
fn section_in_path(path: &str) -> Option<String> {
    path.split('/')
        .find_map(|segment| segment.strip_prefix("section_"))
        .map(str::to_string)
}

/// What an amendment's own words name, or why they could not be read.
///
/// Public because the reading is the reusable half of this module and the half
/// worth testing on its own. A review tool holding an amendment's words and no
/// dataset still wants the answer, and a test that feeds it one real sentence
/// out of the committed bill says more about the reading than a test that
/// builds a dataset around it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Naming {
    /// A section of the Code.
    Section(String),
    /// Nothing this check may rely on, and why not.
    Unread(String),
}

impl Naming {
    /// The section named, or `None` when the words could not be read.
    ///
    /// Here so a caller can ask the question without matching on the reason,
    /// which is a sentence for a person and not something to branch on.
    pub fn section(&self) -> Option<&str> {
        match self {
            Self::Section(section) => Some(section),
            Self::Unread(_) => None,
        }
    }
}

/// The section an amendment's own words name.
///
/// A bill names its target in prose before it quotes anything, so the prose is
/// read first and a citation answers only where the prose does not. Three forms,
/// in the order they are trusted, following the order
/// [`crate::uslm::bill_redesignation`] already reads the same sentence in:
///
/// 1. A bare `Section 263A(c)(2)` — the number is a section of whichever Code
///    the bill's own References clause declares, which is the Internal Revenue
///    Code for the bills this corpus holds. The same is true of
///    `Section 32912 of title 49, United States Code`, where the bill says the
///    title outright.
/// 2. `Section 401(b)(7)(A)(iii) of the Higher Education Act of 1965
///    (20 U.S.C. 1070a(b)(7)(A)(iii))` — the number is a section of an **Act**,
///    and an Act's numbering is not the Code's. The section is whatever the
///    citation beside it says, and where the extractor declined that citation
///    the answer is [`Naming::Unread`] rather than the Act's own number.
///    Comparing `401` against `1070a` would manufacture a fault out of an
///    extractor limitation (#140).
/// 3. Words that name no section in prose — a U.S.C. citation answers for them,
///    and a citation the extractor declined leaves them unread.
///
/// The designations below the section are dropped in every form: the check
/// compares sections, and a path deeper than the citation is not a
/// disagreement.
///
/// **The title is not compared.** A bare section states no title, and inferring
/// one is the single place a wrong guess would silently move a provision between
/// titles of the Code. No link in the committed corpus names a title its own
/// path disagrees with, so the check stays with the sections.
pub fn section_named_in(words: &str) -> Naming {
    // The dashed part is taken only after a **letter**, which is the rule
    // `crate::citation::usc::law_section` already carries and for the same
    // reason: the Code numbers `1400Z-1`, `479a-1` and `300gg-11` that way, so
    // stopping at the dash names a real but different provision (#135, #141).
    // After a digit the dash is a range — `sections 1961-63` — and there the
    // first number is one the text really named, so it is left alone.
    static IN_PROSE: LazyLock<Regex> = LazyLock::new(|| {
        let dashes: String = crate::citation::usc::DASHES
            .iter()
            .map(|dash| regex::escape(&dash.to_string()))
            .collect();
        // The dashed alternative comes first, so a number that has one is read
        // whole. In the other order the plain branch matches `1400Z` and stops,
        // which is the same trap `law_section` documents for subsections.
        //
        // Its digits and its letters are separate runs, `[0-9]+[A-Za-z]+`,
        // rather than one `[0-9A-Za-z]*` followed by a letter. The second
        // spelling lets the same characters be read two ways, and this engine
        // then answers `1400Z` for `1400Z-1`: it reports the branch that reaches
        // a match first rather than backtracking the run to save the branch.
        let number = format!(r"[0-9]+[A-Za-z]+[{dashes}][0-9][0-9A-Za-z]*|[0-9][0-9A-Za-z]*");
        Regex::new(&format!(
            r"(?i)\bsection\s+(?P<section>{number})\s*(?:\([0-9A-Za-z]{{1,6}}\))*"
        ))
        .expect("the prose-section pattern must compile")
    });
    static OF_AN_ACT: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)^\s*of\s+the\s+").expect("the of-an-Act pattern must compile")
    });

    // An insertion names the new section, and the bill states its number inside
    // the text it inserts. Read before the quotations go, because that is the
    // one place the quoted text holds the answer rather than a distraction.
    if let Some(inserted) = new_section_inserted(words) {
        return Naming::Section(inserted);
    }

    // Every quotation is blanked before a word of this is read. A quoted run is
    // struck text, inserted text, or a positional anchor, and none of the three
    // is the provision the amendment acts on.
    let spoken = without_quotations(words);

    if let Some(prose) = IN_PROSE.captures(&spoken) {
        let whole = prose.get(0).expect("a match always has a whole");
        let section = prose["section"].to_string();
        if !OF_AN_ACT.is_match(&spoken[whole.end()..]) {
            return Naming::Section(section);
        }
        // A section of an Act. Its place in the Code is the citation's to give.
        return cited_in(&spoken).unwrap_or_else(|| {
            Naming::Unread(format!(
                "the amendment names section {section} of an Act, and no U.S.C. \
                 citation beside it gives its place in the Code"
            ))
        });
    }

    cited_in(&spoken).unwrap_or_else(|| match IN_PROSE.captures(words) {
        // The words do name a section, and every mention of it is quoted. That
        // is a different fact from naming none, and folding the two together
        // would hide how much of this outcome has one cause (#211).
        Some(quoted) => Naming::Unread(format!(
            "every section the amendment names sits inside a quotation — \
             section {} — so none of them is the section it acts on",
            &quoted["section"]
        )),
        None => Naming::Unread("the amendment's words name no section".to_string()),
    })
}

/// The number of the whole new section an amendment inserts, when it inserts
/// one.
///
/// The bill writes
///
/// > by inserting after section 223 the following new section:"SEC. 224.
/// > 26 USC 224. QUALIFIED TIPS."
///
/// where § 223 is the **anchor** the new section is placed after. An anchor is
/// no more the section the amendment names than a quoted string is, so reading
/// it would report a true finding with a false reason — and the maintainer met
/// exactly that at link `3609722628c6`, where the row was right and its words
/// were wrong.
///
/// Two things must both be there, so this cannot fire on a sentence that merely
/// mentions an insertion: the phrase *the following new section*, and a
/// `SEC. <number>.` heading after it. A bill that says *"inserting after
/// subsection (f) the following new section"* and then quotes `"(g) …"` states
/// no section heading, so this answers `None` and the ordinary reading stands.
///
/// This is the one place a quotation is read rather than blanked, because it is
/// the one place the quoted text **is** the answer.
fn new_section_inserted(words: &str) -> Option<String> {
    static INSERTING_A_SECTION: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r#"(?i)the\s+following\s+new\s+section[^"“]*["“]\s*SEC\.\s*(?P<section>[0-9][0-9A-Za-z]*)\s*\."#,
        )
        .expect("the new-section pattern must compile")
    });
    Some(INSERTING_A_SECTION.captures(words)?["section"].to_string())
}

/// `words` with every quoted run replaced by spaces.
///
/// Amendment language quotes three kinds of thing and **none** of them is the
/// provision being amended: the text struck, the text inserted, and the
/// positional anchor an insertion is placed after. The bill writes
///
/// > by inserting ", as in effect for such academic year," after
/// > "section 479A(b)(1)(B)(v)"
///
/// and § 479A is there only as the string to search for. Reading it as the
/// section the amendment names reported a **correct** link as suspect, which is
/// the cry-wolf failure `#183` and `#211` both warn about.
///
/// Spaces rather than deletion, so a byte offset into the result is the same
/// offset in `words` and a reason built from it still points at the real
/// sentence.
///
/// Both the straight and the curly marks, because the publisher's markup writes
/// curly quotes and a stored excerpt of it writes straight ones.
///
/// **A run nothing closes is blanked to the end.** An excerpt that stops inside
/// a quotation is still inside it — 116 of the 753 committed annotations carry
/// an odd number of quote marks — and refusing to read is the safe direction: a
/// wrong flag costs a reviewer more than a missing one.
fn without_quotations(words: &str) -> String {
    const OPENS: [char; 2] = ['"', '\u{201C}'];
    const CLOSES: [char; 2] = ['"', '\u{201D}'];

    let mut spoken = String::with_capacity(words.len());
    let mut inside = false;
    for mark in words.chars() {
        let opening = !inside && OPENS.contains(&mark);
        let closing = inside && CLOSES.contains(&mark);

        if inside || opening {
            for _ in 0..mark.len_utf8() {
                spoken.push(' ');
            }
        } else {
            spoken.push(mark);
        }

        if opening {
            inside = true;
        } else if closing {
            inside = false;
        }
    }
    spoken
}

/// What the U.S. Code citations in `text` name, read by the shared extractor.
///
/// `None` when the text holds no citation at all, read **or** declined. That is
/// not the same as a citation the extractor could not read, and only the caller
/// knows what it means where it asked, so the two are kept apart here (#140).
fn cited_in(text: &str) -> Option<Naming> {
    let (found, report) = crate::citation::usc::find_with_report(text);
    match (found.first(), report.skipped.first()) {
        (Some(citation), _) => Some(Naming::Section(section_of(citation))),
        (None, Some(skipped)) => Some(Naming::Unread(declined(skipped))),
        (None, None) => None,
    }
}

/// The section a citation names, with the designations below it dropped.
fn section_of(citation: &crate::citation::usc::UscCitation) -> String {
    let named = citation.sections.first().map_or("", String::as_str);
    named
        .split_once('(')
        .map_or(named, |(section, _)| section)
        .to_string()
}

/// Why a declined citation left the naming unread, in the extractor's own words.
fn declined(skipped: &crate::citation::usc::SkippedCitation) -> String {
    format!(
        "the extractor declined \"{}\" — {}",
        skipped.text.trim(),
        skipped.reason
    )
}
