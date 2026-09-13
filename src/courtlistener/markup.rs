//! Getting the words out of a marked-up opinion, and saying what was left.
//!
//! An opinion arrives either as plain text or inside markup — Harvard's XML, a
//! donated HTML archive, a court's own HTML. A single-node tree holds one blob of
//! text, so the markup has to come off. This is that, and it is deliberately not
//! a parser: it produces one flat string, keeps no structure, and builds nothing
//! a caller could mistake for a document tree.
//!
//! # Why the entities matter more than the tags
//!
//! Dropping a tag loses nothing we were going to keep. Failing to decode a
//! character reference loses a citation, silently, and the loss looks like a fact
//! about the law.
//!
//! *Snow v. Commissioner*, 416 U.S. 500 (1974), is the Supreme Court's leading
//! case on 26 U.S.C. § 174. CourtListener's `html` for it writes every section
//! sign as `&#167;`, fifteen times. Strip the tags without decoding, and the
//! U.S.C. extractor — which needs a section marker before it will read a number
//! ([`crate::citation::usc`]) — finds nothing, and the query "which of these
//! opinions cite § 174" answers "not this one". That is a confident wrong answer
//! about a real case, produced by a text pass nobody would think to check.
//!
//! So every reference this pass cannot decode is **reported** rather than left to
//! be noticed: [`text_of_markup`] gives back the text and a [`MarkupReport`]
//! naming each entity it did not know, with a count. That is the same contract
//! [`crate::citation::usc::find_with_report`] has, for the same reason.

use std::collections::BTreeMap;

/// The named character references this pass decodes.
///
/// Short on purpose. HTML names over two thousand entities; a court opinion uses
/// a handful, and anything else is reported rather than guessed at. `sect` is
/// here because it is the section sign, which is the one character the whole
/// U.S.C. extractor depends on.
const NAMED: [(&str, char); 16] = [
    ("amp", '&'),
    ("lt", '<'),
    ("gt", '>'),
    ("quot", '"'),
    ("apos", '\''),
    ("nbsp", ' '),
    ("sect", '\u{a7}'),
    ("para", '\u{b6}'),
    ("mdash", '\u{2014}'),
    ("ndash", '\u{2013}'),
    ("hellip", '\u{2026}'),
    ("lsquo", '\u{2018}'),
    ("rsquo", '\u{2019}'),
    ("ldquo", '\u{201c}'),
    ("rdquo", '\u{201d}'),
    ("middot", '\u{b7}'),
];

/// What one pass over some markup could not read.
///
/// Empty means every reference in the text was decoded. A non-empty report is
/// not fatal — the text is still returned, with the reference left exactly as it
/// was written — but it says a character in the opinion is not the character the
/// court printed, and where to look.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MarkupReport {
    /// Each named reference met and not decoded, with how many times, in name
    /// order.
    pub unknown_entities: BTreeMap<String, usize>,
}

impl MarkupReport {
    pub fn is_empty(&self) -> bool {
        self.unknown_entities.is_empty()
    }

    /// One line for each unknown reference, in name order.
    pub fn summary(&self) -> Vec<String> {
        self.unknown_entities
            .iter()
            .map(|(name, count)| {
                format!(
                    "warning: `&{name};` is a character reference this build does \
                     not decode, and it appears {count} time(s). The text keeps it \
                     as written."
                )
            })
            .collect()
    }

    /// Write the report to stderr, so it reaches the person who ran the command.
    pub fn print_to_stderr(&self) {
        for line in self.summary() {
            eprintln!("{line}");
        }
    }
}

/// The words of a marked-up document, and what could not be decoded.
///
/// Tags become whitespace, character references become the characters they name,
/// and runs of whitespace collapse. Nothing else happens: no tag is interpreted,
/// no structure is kept, and no element name is read.
///
/// ```
/// use words_to_data::courtlistener::markup::text_of_markup;
///
/// // The form Snow v. Commissioner is really written in.
/// let (text, report) = text_of_markup("<p>under 26 U.S.C. &#167; 174(a)(1), a</p>");
/// assert_eq!(text, "under 26 U.S.C. § 174(a)(1), a");
/// assert!(report.is_empty());
///
/// // A reference this build does not know survives as written, and is named.
/// let (text, report) = text_of_markup("costs &permil; of the year");
/// assert_eq!(text, "costs &permil; of the year");
/// assert_eq!(report.unknown_entities.get("permil"), Some(&1));
/// ```
pub fn text_of_markup(markup: &str) -> (String, MarkupReport) {
    let mut report = MarkupReport::default();
    let without_tags = strip_tags(markup);
    let decoded = decode_entities(&without_tags, &mut report);
    (squeeze_whitespace(&decoded), report)
}

/// Replace every `<...>` with a newline.
///
/// A newline rather than nothing, because `</p><p>` between two words is a word
/// boundary and joining them would invent a word. A newline rather than a space
/// because a tag is usually where a line ends, and the result is meant to be
/// readable.
fn strip_tags(markup: &str) -> String {
    let mut out = String::with_capacity(markup.len());
    let mut inside = false;

    for character in markup.chars() {
        match character {
            '<' => {
                inside = true;
                out.push('\n');
            }
            // A stray `>` outside a tag is a greater-than sign and is kept.
            '>' if inside => inside = false,
            _ if inside => {}
            _ => out.push(character),
        }
    }

    out
}

/// Turn every character reference this build knows into its character, and count
/// the rest.
fn decode_entities(text: &str, report: &mut MarkupReport) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;

    while let Some(start) = rest.find('&') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];

        // A reference is `&name;` or `&#123;`, and a name is short. Anything
        // longer than this is an ampersand that happens to be followed by
        // words, which is most of them.
        const LONGEST: usize = 12;
        let end = after
            .char_indices()
            .take_while(|(at, _)| *at < LONGEST)
            .find(|(_, character)| *character == ';')
            .map(|(at, _)| at);

        match end.and_then(|end| decoded(&after[..end], report).map(|c| (c, end))) {
            Some((character, end)) => {
                out.push(character);
                rest = &after[end + 1..];
            }
            None => {
                // Not a reference we read: keep the ampersand as written and
                // carry on from the next character.
                out.push('&');
                rest = after;
            }
        }
    }

    out.push_str(rest);
    out
}

/// The character a reference body names, or `None` when this build does not read
/// it.
///
/// An unknown *named* reference is recorded. An unknown numeric one is not: a
/// number out of Unicode's range is a broken document rather than a gap in this
/// list, and there is nothing to add here in answer to it.
fn decoded(body: &str, report: &mut MarkupReport) -> Option<char> {
    if let Some(digits) = body.strip_prefix('#') {
        let code = match digits.strip_prefix(['x', 'X']) {
            Some(hex) => u32::from_str_radix(hex, 16).ok()?,
            None => digits.parse().ok()?,
        };
        return char::from_u32(code);
    }

    if body.is_empty() || !body.chars().all(|c| c.is_ascii_alphanumeric()) {
        return None;
    }

    match NAMED.iter().find(|(name, _)| *name == body) {
        Some((_, character)) => Some(*character),
        None => {
            *report.unknown_entities.entry(body.to_string()).or_insert(0) += 1;
            None
        }
    }
}

/// Collapse runs of whitespace, keeping one newline where the run held one.
///
/// Markup is indented, so a strip leaves long runs of spaces and newlines that
/// carry no information and make every offset in the text less readable.
fn squeeze_whitespace(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut run = String::new();

    for character in text.chars() {
        if character.is_whitespace() {
            run.push(character);
            continue;
        }
        if !run.is_empty() {
            if !out.is_empty() {
                out.push(if run.contains('\n') { '\n' } else { ' ' });
            }
            run.clear();
        }
        out.push(character);
    }

    out
}
