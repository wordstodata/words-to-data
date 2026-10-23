//! The code an opinion names as the default of its bare references.
//!
//! Tax opinions write "section 199A(c)(1)", "sec. 174" or "I.R.C. § 174", with
//! no title, and state once which code such a reference is to: "Unless
//! otherwise indicated, all section references are to the Internal Revenue Code
//! of 1986, as amended and in effect for the years in issue". When a case has
//! such a sentence, the references are read as citations to title 26, and their
//! evidence is the citation and the sentence. A case without it gets no bare
//! reads (`docs/adr/0014-a-case-is-a-work-and-a-citation-runs-paragraph-to-subdivision.md`,
//! decision 8; #318).
//!
//! The sentence holds for the whole case. A Tax Court report states it once, in
//! the first footnote of the Court's opinion, and the concurrences and dissents
//! printed with it use the same references. The evidence names the writing
//! that states it ([`DefaultCode::stated_in`]).

use std::sync::LazyLock;

use regex::Regex;

use super::usc::{UscCitation, compact, ends_cleanly, further_section, law_section};

/// The default an opinion states for its bare references.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefaultCode {
    /// The words of the default, verbatim, from its first word to the end of
    /// what it says about the Code: "Unless otherwise indicated, statutory
    /// references are to the Internal Revenue Code, Title 26 U.S.C. (I.R.C. or
    /// Code), in effect at all relevant times".
    pub statement: String,
    /// The title of the U.S. Code the default names: `26`.
    pub title: &'static str,
    /// The year the opinion applies the Code as in effect for, when the
    /// statement names one: `2013` for "as in effect and operative for the 2013
    /// calendar year". `None` for "in effect at all relevant times". The year in
    /// "the Internal Revenue Code of 1986" is the name of the Code and not a
    /// time basis, so it is never read as one.
    pub year: Option<String>,
    /// Whether the default covers a reference with no code at all, such as
    /// "section 199A". True when the statement says "section references",
    /// "statutory references" or "all references" are to the Code. False when it
    /// says only what "references to the Internal Revenue Code" are to: then
    /// only a reference that names the Code ("IRC section 274(d)") is read.
    pub reads_bare_sections: bool,
    /// The path of the writing whose text states the default.
    pub stated_in: String,
}

/// A statement that the references of an opinion are to the Internal Revenue
/// Code.
///
/// Two shapes. `bare` covers every reference: "(all) section references are
/// to", "statutory references are to". `named` covers the references that name
/// the Code: "references to the Internal Revenue Code (IRC) are to the IRC".
/// Each may open with "Unless otherwise indicated," and each may end with what
/// it says about time ("in effect for 1986", "as amended", "in effect at all
/// relevant times"), which stops at a comma, a semicolon or a full stop.
static STATEMENT: LazyLock<Regex> = LazyLock::new(|| {
    let opening = r"(?:Unless\s+otherwise\s+(?:indicated|noted|stated|specified),\s+)?";
    let bare = r"(?P<bare>(?:all\s+)?(?:section|statutory)\s+references\s+(?:herein\s+)?are\s+to\s+the\s+Internal\s+Revenue\s+Code)";
    let named = r"(?P<named>(?:all\s+)?references\s+to\s+the\s+Internal\s+Revenue\s+Code[^;]{0,60}?\s+are\s+to\s+the\s+(?:IRC|I\.R\.C\.|Internal\s+Revenue\s+Code|Code))";
    let time = r"(?:[^;]{0,120}?(?P<time>(?:as\s+)?(?:in\s+effect|amended)[^,;.]*))?";
    Regex::new(&format!(r"(?i){opening}(?:{bare}|{named}){time}"))
        .expect("the default-code pattern must compile")
});

/// A year in the part of a statement that says what time the Code is read at.
static YEAR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(?:1[89]|20)\d\d\b").expect("the year pattern must compile"));

/// The default code the writings of one case state, from the first writing
/// that states one. `None` when no writing does.
///
/// Each writing is given as its path and its whole text.
pub fn of_case<'a>(writings: impl IntoIterator<Item = (&'a str, &'a str)>) -> Option<DefaultCode> {
    writings.into_iter().find_map(|(path, text)| {
        stated(text).map(|default| DefaultCode {
            stated_in: path.to_string(),
            ..default
        })
    })
}

/// The default one text states, with no writing named yet.
fn stated(text: &str) -> Option<DefaultCode> {
    let capture = STATEMENT.captures(text)?;
    let year = capture
        .name("time")
        .and_then(|time| YEAR.find(time.as_str()))
        .map(|year| year.as_str().to_string());
    Some(DefaultCode {
        statement: capture
            .get(0)
            .expect("a match always has a whole")
            .as_str()
            .trim_end()
            .to_string(),
        title: "26",
        year,
        reads_bare_sections: capture.name("bare").is_some(),
        stated_in: String::new(),
    })
}

/// A reference that names the Code and no title: `I.R.C. § 199A(a)`,
/// `IRC § 274(d)`, `IRC section 162(a)`.
static NAMED: LazyLock<Regex> = LazyLock::new(|| {
    let section = law_section();
    Regex::new(&format!(
        r"\b(?:I\.\s?R\.\s?C\.|IRC)\s*(?P<marker>§§?|[Ss]ections?|[Ss]ecs?\.)\s*(?P<section>{section})"
    ))
    .expect("the named-reference pattern must compile")
});

/// A reference that names no code: `section 199A(c)(1)`, `Sections 199A and
/// 280E`, `sec. 174`. A bare `§` is not one: an opinion writes `§` after the
/// name of every other code it cites (`Treas. Reg. § 1.199A-5`, `TCJA § 11011`),
/// and the name may be on the line before.
static BARE: LazyLock<Regex> = LazyLock::new(|| {
    let section = law_section();
    Regex::new(&format!(
        r"\b(?P<marker>[Ss]ections?|[Ss]ecs?\.)\s+(?P<section>{section})"
    ))
    .expect("the bare-reference pattern must compile")
});

/// The word before a reference that makes it a reference to another body of
/// law: `Treas Reg section 1.174-3(b)`, `Treasury Regulation section`,
/// `Rule`, `Stat.`, `C.F.R.`.
static OTHER_LAW_BEFORE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:Reg|Regs|Regulations?|Treas|Rules?|Stat|C\.F\.R|ORS|Act)\.?\s*$")
        .expect("the other-law pattern must compile")
});

/// The words after a reference that name the law it is in: `section 2119 of
/// the Tax Reform Act of 1976`, `Section 1.167(a)-2 of the federal Treasury
/// Regulations`. The
/// Code itself may be named: `Section 162 of the Internal Revenue Code`.
static OF_A_LAW_AFTER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s+of\s+(?:the\s+)?(?P<law>\S+)").expect("the of-a-law pattern must compile")
});

/// Every bare reference in `text` that `default` makes a citation, as a
/// citation to its title, in the order they appear.
///
/// `read` is the citations the U.S.C. patterns already read in the same text,
/// so that `26 U.S.C. section 174` is not read twice. A reference that names
/// the Code (`I.R.C. § 199A`) is read under every default. A reference that
/// names no code (`section 199A`) is read only when the default covers it
/// ([`DefaultCode::reads_bare_sections`]). A list (`Sections 199A and 280E`) is
/// read only after a plural marker. A reference is never read when the words
/// around it name another law: a regulation, a rule, or "of the Tax Reform Act
/// of 1976". A number with a
/// full stop in it (`1.199A-5`) is a regulation's, because no section of title
/// 26 has one.
pub fn find_bare(text: &str, default: &DefaultCode, read: &[UscCitation]) -> Vec<UscCitation> {
    let mut found: Vec<UscCitation> = Vec::new();
    let patterns: &[(&Regex, bool)] = match default.reads_bare_sections {
        true => &[(&NAMED, true), (&BARE, false)],
        false => &[(&NAMED, true)],
    };
    for (pattern, names_the_code) in patterns {
        for capture in pattern.captures_iter(text) {
            let whole = capture.get(0).expect("a match always has a whole");
            let first = capture
                .name("section")
                .expect("the pattern names a section");
            let inside = |at: usize| {
                read.iter()
                    .chain(found.iter())
                    .any(|citation| citation.start <= at && at < citation.end())
            };
            if inside(whole.start()) || inside(first.start()) || !ends_cleanly(text, whole.end()) {
                continue;
            }
            if !names_the_code && names_another_law(text, whole.start(), whole.end()) {
                continue;
            }

            let mut sections = vec![compact(first.as_str())];
            let mut end = whole.end();
            // Only a plural marker names a list. After `section 199(b)(2), 3`
            // the 3 is a footnote mark.
            let marker = capture.name("marker").map_or("", |marker| marker.as_str());
            let plural = marker == "§§" || marker.trim_end_matches('.').ends_with('s');
            while plural && let Some((start, stop)) = further_section(text, end) {
                sections.push(compact(&text[start..stop]));
                end = stop;
            }
            if sections.iter().any(|section| section.contains('.')) {
                continue;
            }
            found.push(UscCitation {
                title: default.title.to_string(),
                sections,
                text: text[whole.start()..end].to_string(),
                start: whole.start(),
                repair: None,
                default_code: Some(default.clone()),
            });
        }
    }
    found.sort_by_key(|citation| citation.start);
    found
}

/// Whether the words just before or just after a bare reference name a law
/// other than the Code.
fn names_another_law(text: &str, start: usize, end: usize) -> bool {
    let from = text[..start]
        .char_indices()
        .rev()
        .nth(24)
        .map_or(0, |(at, _)| at);
    if OTHER_LAW_BEFORE.is_match(&text[from..start]) {
        return true;
    }
    OF_A_LAW_AFTER
        .captures(&text[end..])
        .and_then(|capture| capture.name("law"))
        .is_some_and(|law| {
            let law = law.as_str().trim_end_matches([',', '.', ';']);
            !["Internal", "Code", "I.R.C", "IRC", "title"]
                .iter()
                .any(|code| law.eq_ignore_ascii_case(code))
        })
}
