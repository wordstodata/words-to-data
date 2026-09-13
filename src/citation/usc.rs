//! Reading U.S. Code citations out of text.
//!
//! # Where the patterns come from
//!
//! The patterns below are the `U.S.C.` entry of `reporters_db/data/laws.json`
//! and the `law.section` and `section_marker` templates of
//! `reporters_db/data/regexes.json`, from Free Law Project's `reporters-db`:
//!
//! > BSD 2-Clause License. Copyright (c) 2020, Free Law Project.
//! > <https://github.com/freelawproject/reporters-db>
//!
//! They are copied in rather than fetched, so a build needs no network and a
//! reader of this file can see exactly what is matched. Every place this file
//! departs from the published source is marked where it happens, with what the
//! published pattern says and why we differ, because a silent divergence from an
//! upstream pattern is a maintenance trap. [`LAW_SECTION`] carries three such
//! notes.
//!
//! # What is read, and what is declined
//!
//! Nothing fails quietly. [`find_with_report`] gives back the citations read
//! **and** the citation-shaped text declined, each with a reason, and [`find`]
//! writes the reasons to stderr. A reader can then tell "this text cites no
//! statute" from "this citation form is not read", which is the difference
//! `CONTEXT.md` draws between a Gap and an Exclusion.
//!
//! One form is declined on purpose: a citation with no section marker,
//! `16 U.S.C. 1533`. The U.S. Code's own notes are written that way and there are
//! 51,721 of them in the 2025-07-30 release, so this is the larger of the two
//! limits by count. It is also the risky one. With no `§` and no `Section`, a
//! number in that position cannot be told from a page or a year, which is
//! presumably why the published pattern asks for a marker at all, and a wrong
//! citation is worse than a missed one. So it is counted and named, not matched.
//!
//! A section number cut short is declined for the same reason, and that is the
//! whole of the rest: `25 U.S.C. § 479a–1` is read as nothing rather than as
//! section 479a, because the release holds both ([`ends_cleanly`]). Fourteen
//! citations in the release land there.

use std::sync::LazyLock;

use regex::Regex;

use crate::dataset::WorkId;

/// How the reporter is spelled: the `U.S.C.` key of `laws.json`, then its seven
/// `variations`, verbatim.
///
/// Longest first, so `U.S.C.A.` is preferred over `U.S.C.` followed by a stray
/// `A.`; the alternation takes the first spelling that lets the whole citation
/// match.
const REPORTERS: [&str; 8] = [
    "United States Code",
    "U.S.C.A.",
    "U.S.C.S.",
    "U.S.C.U.",
    "U.S. Code",
    "U. S. C.",
    "U.S.C.",
    "USC",
];

/// `section_marker` from `regexes.json`, verbatim: `§`, `§§`, `Section`,
/// `Sections`, `sec.`, `S.`, and the rest of that family.
const SECTION_MARKER: &str = r"((§§?)|([Ss]((ec)(tion)?)?s?\.?))";

/// `law.section` from `regexes.json`, with its group name removed so it can be
/// used more than once in one pattern: a section number such as `1-2-3`, `1.2.3`,
/// `981(a)(l)(C)` or `300gg-11`.
///
/// The published pattern is:
///
/// ```text
/// (?:\d+(?:\((?:[a-zA-Z]{1}|\d{1,2})\))+)|(?:\d+(?:[\-.:]\d+){,3})
/// ```
///
/// Three changes from it.
///
/// `{,3}` is written `{0,3}`, because Python accepts an open lower bound and
/// Rust does not. That is mechanical.
///
/// The two alternatives are the other way round, so a section with a subsection
/// is read whole. In the published order the plain-number branch is preferred
/// and `981(a)(l)(C)` stops at `981`; eyecite reads the published example that
/// way, and CourtListener's own markup shows the cost — in opinion 11103682 the
/// `(g)(1)` of `18 U.S.C. § 922(g)(1)` falls outside the citation span
/// (`docs/research/courtlistener-formats.md`, section 8.3). What the opinion
/// named is what we keep.
///
/// Every `\d+` may be followed by up to four letters, written `\d+[a-zA-Z]{0,4}`.
/// The published pattern has both alternatives begin `\d+` and allows a letter
/// only inside brackets, so `26 U.S.C. § 45X`, `21 U.S.C. § 355a` and
/// `15 U.S.C. § 78aaa` are all unreadable to it. That is not a tail case: of the
/// 29,592 section numbers in the 2025-07-30 release of the Code, 10,085 end in a
/// letter, and the lettered ones are the heavily litigated ones (#135). The
/// change is made in every `\d+` of a section number rather than only the first,
/// because the Code writes `42 U.S.C. § 1395w-4a` as well as `42 U.S.C. §
/// 300gg-11`.
///
/// Four letters, because four is the longest run the Code uses — `15 U.S.C. §
/// 77bbbb`, `16 U.S.C. § 460dddd` — so a longer run is not a section number. The
/// bound is what keeps [`ends_cleanly`] able to refuse `§ 78aaaaa` instead of
/// reading a number that no section has.
const LAW_SECTION: &str = r"(?:\d+[a-zA-Z]{0,4}(?:\((?:[a-zA-Z]{1}|\d{1,2})\))+)|(?:\d+[a-zA-Z]{0,4}(?:[\-.:]\d+[a-zA-Z]{0,4}){0,3})";

/// Every dash the Code writes inside a section number.
///
/// `law.section` reads the ASCII hyphen of `[\-.:]` and nothing else, and the
/// published Code prints `479a–1` with an en dash. The rest of the family is here
/// so that a number written with one is not read cut short ([`ends_cleanly`]).
const DASHES: [char; 6] = [
    '-',        // HYPHEN-MINUS
    '\u{2010}', // HYPHEN
    '\u{2011}', // NON-BREAKING HYPHEN
    '\u{2012}', // FIGURE DASH
    '\u{2013}', // EN DASH
    '\u{2014}', // EM DASH
];

/// A title, a reporter, and a number in the section position — whether or not the
/// rest of it is a citation this module can read.
///
/// Not from `reporters_db`. It is the `U.S.C.` regex with the section marker made
/// optional and the section number loosened to any run that starts with a digit,
/// so that a citation form the reader declines can still be named and counted
/// ([`find_with_report`]).
///
/// The run must start with a digit, because every section of the Code is numbered
/// and a report about `5 U.S.C. Appendix` would be a report about prose rather
/// than about a citation. It must end in a letter or a digit, so that the full
/// stop of `16 U.S.C. 1533.` is not read as part of the number. A bracketed
/// subsection is taken only as a whole group, so the closing bracket of
/// `(16 U.S.C. 1533)` stays out as well. The skip is evidence a person reads, and
/// evidence with punctuation in it invites a second look at nothing.
static CANDIDATE: LazyLock<Regex> = LazyLock::new(|| {
    let reporters = reporters_pattern();
    let dashes = dash_class();
    Regex::new(&format!(
        r"\b(?P<title>\d+),?\s+(?P<reporter>{reporters}),?\s+(?:(?P<marker>{SECTION_MARKER})\s*)?(?P<section>\d(?:[0-9A-Za-z.:{dashes}]*[0-9A-Za-z])?(?:\([0-9A-Za-z]+\))*)"
    ))
    .expect("the candidate pattern must compile")
});

/// The `U.S.C.` regex of `laws.json`:
/// `(?P<title>\d+),?\s+$reporter,?\s+$section_marker\s*$law_section`.
///
/// `\b` in front keeps the title from starting inside a longer word.
static CITATION: LazyLock<Regex> = LazyLock::new(|| {
    let reporters = reporters_pattern();
    Regex::new(&format!(
        r"\b(?P<title>\d+),?\s+(?P<reporter>{reporters}),?\s+{SECTION_MARKER}\s*(?P<section>{LAW_SECTION})"
    ))
    .expect("the vendored U.S.C. pattern must compile")
});

/// A further section of the same citation: `, 1988` or ` and 1477`.
///
/// Anchored, because it is matched against the text that follows a citation and
/// a match anywhere later would belong to something else.
static FURTHER_SECTION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"^(?:\s*,\s*|\s+and\s+)(?P<section>{LAW_SECTION})"
    ))
    .expect("the further-section pattern must compile")
});

/// The start of another citation: a number we have just read turns out to be the
/// title of the next one.
static ANOTHER_CITATION: LazyLock<Regex> = LazyLock::new(|| {
    let reporters = reporters_pattern();
    Regex::new(&format!(r"^,?\s+(?:{reporters})"))
        .expect("the further-citation pattern must compile")
});

/// [`DASHES`] as the inside of a regex character class, so the list and the
/// pattern cannot drift apart.
fn dash_class() -> String {
    DASHES
        .iter()
        .map(|dash| regex::escape(&dash.to_string()))
        .collect()
}

/// Every spelling of the reporter, as one alternation.
///
/// Each spelling is escaped, and any run of whitespace may stand where it has a
/// space: opinions wrap lines and scanned text widens spacing, so a single
/// literal space would miss `United\nStates Code`.
fn reporters_pattern() -> String {
    REPORTERS
        .iter()
        .map(|spelling| {
            spelling
                .split(' ')
                .map(regex::escape)
                .collect::<Vec<_>>()
                .join(r"\s+")
        })
        .collect::<Vec<_>>()
        .join("|")
}

/// A citation to the U.S. Code, as one piece of text wrote it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UscCitation {
    /// The title: `26` in `26 U.S.C. § 174`.
    pub title: String,
    /// Every section the citation names, in the order written.
    ///
    /// `42 U.S.C. §§ 1983, 1988` names two. One citation naming two provisions
    /// is two links, so keeping only the first would lose a link.
    ///
    /// Each is written as the text wrote it, brackets and all: `981(a)(l)(C)`.
    pub sections: Vec<String>,
    /// The text matched, verbatim.
    ///
    /// This is the evidence a reviewer checks a link against, so it is never
    /// tidied up.
    pub text: String,
    /// Where `text` starts in the text it was found in, in bytes.
    pub start: usize,
}

impl UscCitation {
    /// Where `text` ends, in bytes.
    pub fn end(&self) -> usize {
        self.start + self.text.len()
    }

    /// The work this citation is into: `uscode/title_26`.
    ///
    /// The same shape storage keys a title under, so a caller can ask the
    /// dataset whether it holds the title at all.
    pub fn work(&self) -> WorkId {
        WorkId::new(format!("uscode/title_{}", self.title))
    }

    /// The USLM identifier of one cited section: `/us/usc/t26/s174`.
    ///
    /// A subsection the citation names is deliberately dropped, so
    /// `981(a)(l)(C)` resolves to section 981. The published example shows why:
    /// its `(l)` is a lower-case L where the provision has a paragraph `(1)`.
    /// Text that cannot be trusted at that depth must not be resolved at that
    /// depth.
    pub fn uslm_id(&self, section: &str) -> String {
        format!("/us/usc/t{}/s{}", self.title, section_number(section))
    }
}

/// The number of the section itself, without any subsection: `981` for
/// `981(a)(l)(C)`.
pub fn section_number(section: &str) -> &str {
    section
        .split_once('(')
        .map_or(section, |(number, _)| number)
}

/// Why a piece of text that looks like a citation was not read as one.
///
/// A reason is part of the statement. A hole with no reason cannot be told apart
/// from an oversight (`CONTEXT.md`, *Exclusion*), and "did not match" is no
/// reason at all: it names the reader's behaviour rather than the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipReason {
    /// No `§` and no `Section` stands between the reporter and the number, as in
    /// `16 U.S.C. 1533`.
    ///
    /// The form the Code's own notes use, and the one form left unread on
    /// purpose. Without a marker a number in that position cannot be told from a
    /// page or a year, which is why the published pattern asks for a marker, and
    /// a wrong citation is worse than a missed one.
    NoSectionMarker,
    /// A marker is there, and the number beside it cannot be read whole, as in
    /// `25 U.S.C. § 479a–1`.
    ///
    /// Reading the part of it that the pattern does read would name a different
    /// provision — section 479a is not section 479a–1, and the release holds both
    /// — so nothing is read. See [`ends_cleanly`].
    SectionNumberNotRead,
}

impl std::fmt::Display for SkipReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let said = match self {
            Self::NoSectionMarker => "no section marker",
            Self::SectionNumberNotRead => "a section number that cannot be read whole",
        };
        f.write_str(said)
    }
}

/// Text that looks like a U.S. Code citation and was not read as one.
///
/// Enough to act on: what the text says, where it is, and why it was declined.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedCitation {
    /// The text that was declined, verbatim: `16 U.S.C. 1533`.
    pub text: String,
    /// Where `text` starts in the text it was found in, in bytes. The same
    /// measure [`UscCitation::start`] uses, so the two can be read side by side.
    pub start: usize,
    pub reason: SkipReason,
}

impl SkippedCitation {
    /// Where `text` ends, in bytes.
    pub fn end(&self) -> usize {
        self.start + self.text.len()
    }
}

/// What one pass over a piece of text declined to read.
///
/// A skip happens before resolution: the citation was never read, so there is
/// nothing to resolve and no [`Resolution`](super::resolve::Resolution) to give.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FindReport {
    /// Every candidate declined, in the order they appear in the text.
    pub skipped: Vec<SkippedCitation>,
}

impl FindReport {
    /// True when every citation-shaped piece of text was read.
    pub fn is_empty(&self) -> bool {
        self.skipped.is_empty()
    }

    /// Write the report to stderr, so it reaches the person who ran the command.
    ///
    /// One line for each reason, not one for each skip. A title of the Code
    /// writes the marker-less form thousands of times — title 42 alone holds over
    /// ten thousand — and ten thousand lines is not a report anybody reads. The
    /// count says how much was declined, the example says what it looked like,
    /// and [`FindReport::skipped`] holds every one for a caller that wants them.
    ///
    /// The crate carries no logger, and the parser reports what it dropped the
    /// same way (`uslm::parser::ParseReport::print_to_stderr`).
    pub fn print_to_stderr(&self) {
        for line in self.summary() {
            eprintln!("{line}");
        }
    }

    /// One line for each reason met, in the order the reasons were first met.
    ///
    /// The line names the reason, how many skips it accounts for, and the first
    /// of them, so a reader can go and look at one.
    pub fn summary(&self) -> Vec<String> {
        let mut lines = Vec::new();

        for (at, skipped) in self.skipped.iter().enumerate() {
            let met_before = self.skipped[..at]
                .iter()
                .any(|earlier| earlier.reason == skipped.reason);
            if met_before {
                continue;
            }

            let count = self
                .skipped
                .iter()
                .filter(|other| other.reason == skipped.reason)
                .count();
            lines.push(format!(
                "warning: {count} U.S.C. citation(s) were not read: {}. The first is {:?} at {}",
                skipped.reason, skipped.text, skipped.start
            ));
        }

        lines
    }
}

/// Every U.S. Code citation in a piece of text, in the order they appear.
///
/// What the pass declined to read goes to stderr. Use [`find_with_report`] when
/// the caller decides how a skip is shown.
///
/// ```
/// use words_to_data::citation::usc;
///
/// let found = usc::find("See 26 U.S.C. § 174 (2018), and 42 U.S.C. §§ 1983, 1988.");
///
/// assert_eq!(found.len(), 2);
/// assert_eq!(found[0].sections, ["174"]);
/// // The second section of a list is kept, which is where eyecite loses one.
/// assert_eq!(found[1].sections, ["1983", "1988"]);
/// ```
pub fn find(text: &str) -> Vec<UscCitation> {
    let (found, report) = find_with_report(text);
    report.print_to_stderr();
    found
}

/// Every citation read, and every citation-shaped piece of text declined.
///
/// The same pass as [`find`], except that the caller receives the [`FindReport`]
/// instead of having it printed to stderr.
///
/// ```
/// use words_to_data::citation::usc::{self, SkipReason};
///
/// let (found, report) = usc::find_with_report("under 16 U.S.C. 1533 and 26 U.S.C. § 45X");
///
/// // The lettered section is read; the one with no marker is named and declined.
/// assert_eq!(found.len(), 1);
/// assert_eq!(found[0].sections, ["45X"]);
/// assert_eq!(report.skipped.len(), 1);
/// assert_eq!(report.skipped[0].text, "16 U.S.C. 1533");
/// assert_eq!(report.skipped[0].reason, SkipReason::NoSectionMarker);
/// ```
pub fn find_with_report(text: &str) -> (Vec<UscCitation>, FindReport) {
    let found = read_citations(text);
    let skipped = declined_candidates(text, &found);
    (found, FindReport { skipped })
}

/// Every citation the vendored patterns read.
fn read_citations(text: &str) -> Vec<UscCitation> {
    let mut found = Vec::new();

    for capture in CITATION.captures_iter(text) {
        let whole = capture.get(0).expect("a match always has a whole");
        let first = capture
            .name("section")
            .expect("the pattern names a section")
            .as_str();
        if !ends_cleanly(text, whole.end()) {
            // The section number runs on, as in `§ 479a–1`. What we matched is a
            // different provision from the one cited.
            continue;
        }

        let mut sections = vec![first.to_string()];
        let mut end = whole.end();
        while let Some((start, stop)) = further_section(text, end) {
            sections.push(text[start..stop].to_string());
            end = stop;
        }

        found.push(UscCitation {
            title: capture
                .name("title")
                .expect("the pattern names a title")
                .as_str()
                .to_string(),
            sections,
            text: text[whole.start()..end].to_string(),
            start: whole.start(),
        });
    }

    found
}

/// Every citation-shaped piece of text that no citation in `found` covers.
///
/// A candidate the reader took is not a skip, so the citations already read say
/// which candidates to pass over. The two patterns agree on where a citation
/// begins — same title, same reporter — so one starting offset is enough to pair
/// them.
fn declined_candidates(text: &str, found: &[UscCitation]) -> Vec<SkippedCitation> {
    let mut skipped = Vec::new();

    for candidate in CANDIDATE.captures_iter(text) {
        let whole = candidate.get(0).expect("a match always has a whole");
        if found.iter().any(|citation| citation.start == whole.start()) {
            continue;
        }

        let reason = if candidate.name("marker").is_none() {
            SkipReason::NoSectionMarker
        } else {
            SkipReason::SectionNumberNotRead
        };
        skipped.push(SkippedCitation {
            text: text[whole.start()..whole.end()].to_string(),
            start: whole.start(),
            reason,
        });
    }

    skipped
}

/// Where the next section of the citation that ends at `end` sits, if the text
/// carries on with one.
///
/// A list continues while each item is a section number and nothing more. A
/// number followed by a reporter is the title of the next citation, not a
/// further section of this one: in `26 U.S.C. § 174, 42 U.S.C. § 1983` the `42`
/// belongs to the second citation.
fn further_section(text: &str, end: usize) -> Option<(usize, usize)> {
    let rest = &text[end..];
    let section = FURTHER_SECTION.captures(rest)?.name("section")?;
    let (start, stop) = (end + section.start(), end + section.end());

    if ANOTHER_CITATION.is_match(&text[stop..]) || !ends_cleanly(text, stop) {
        return None;
    }
    Some((start, stop))
}

/// Whether a citation that ends here ends at the end of its section number.
///
/// A letter or a digit straight after it means the number was cut short, and a
/// section number cut short names the wrong provision.
///
/// A dash and then a letter or a digit mean the same, when the number read ends
/// in a letter. The Code numbers its lettered sections `479a–1`, `77z–1` and
/// `2000e–5`, and prints the dash as an en dash, which the published separator
/// class `[\-.:]` does not read. The 2025-07-30 release holds `25 U.S.C. § 479a`
/// and `25 U.S.C. § 479a–1` both, so reading the first where the text names the
/// second would name a real but different provision. A number that ends in a
/// digit is left alone: there the dash is the range of `§§ 1961–63`, and the
/// first section named is still one the text cited.
fn ends_cleanly(text: &str, end: usize) -> bool {
    let mut after = text[end..].chars();
    let Some(next) = after.next() else {
        return true;
    };
    if next.is_alphanumeric() {
        return false;
    }

    let ends_in_a_letter = text[..end]
        .chars()
        .next_back()
        .is_some_and(|last| last.is_ascii_alphabetic());
    let carries_on = DASHES.contains(&next) && after.next().is_some_and(char::is_alphanumeric);

    !(ends_in_a_letter && carries_on)
}
