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
//! reader of this file can see exactly what is matched. One change was needed to
//! compile them with the Rust `regex` crate: Python writes a bounded repeat with
//! an open lower bound, `{,3}`, and Rust requires `{0,3}`. Nothing else differs.
//!
//! # Two limits this shares with eyecite
//!
//! Both follow from the vendored patterns, and both fail by finding nothing
//! rather than by naming the wrong provision.
//!
//! A citation with no section marker, `16 U.S.C. 1533`, is not matched: the
//! pattern requires a `§` or a `Section`. The U.S. Code's own notes are written
//! that way, so this is worth fixing, with its own fixtures.
//!
//! A section number that ends in a letter, `21 U.S.C. § 355a`, is not matched
//! either, because `law.section` allows no letters outside brackets. Reporting
//! section 355 instead would be worse than reporting nothing: it names a
//! different provision.

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
/// used more than once in one pattern: a section number such as `1-2-3`, `1.2.3`
/// or `981(a)(l)(C)`.
///
/// Two changes from the published pattern.
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
const LAW_SECTION: &str = r"(?:\d+(?:\((?:[a-zA-Z]{1}|\d{1,2})\))+)|(?:\d+(?:[\-.:]\d+){0,3})";

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

/// Every U.S. Code citation in a piece of text, in the order they appear.
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
    let mut found = Vec::new();

    for capture in CITATION.captures_iter(text) {
        let whole = capture.get(0).expect("a match always has a whole");
        let first = capture
            .name("section")
            .expect("the pattern names a section")
            .as_str();
        if !ends_cleanly(text, whole.end()) {
            // The section number runs on into letters, as in `§ 355a`. What we
            // matched is a different provision from the one cited.
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
fn ends_cleanly(text: &str, end: usize) -> bool {
    !text[end..]
        .chars()
        .next()
        .is_some_and(|next| next.is_alphanumeric())
}
