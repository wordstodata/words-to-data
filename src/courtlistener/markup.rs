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
/// A tag that ends a line becomes a newline, a tag inside a line, such as
/// `<em>`, becomes nothing, and a footnote or page mark is a word of its own.
/// Character references become the characters they name, and runs of
/// whitespace collapse. Nothing else happens: no structure is kept.
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

/// Elements that mark words inside a line and end no line.
///
/// Harvard writes `5 U.S.C. §§ 8901</extracted-citation>(5), 8905`: the
/// citation tag ends between the section number and its subdivision, and a line
/// break there would split the one from the other (#412). Columbia's citation
/// tag is `<cross_reference>`. The emphasis tags are the ones met in real
/// records: `<em>Amici Curiae</em>` is a phrase in its line, and a search for
/// it must find it. A link, `<a>`, and a `<span>` are in their line too.
const INLINE: [&str; 9] = [
    "extracted-citation",
    "cross_reference",
    "em",
    "i",
    "b",
    "strong",
    "s",
    "a",
    "span",
];

/// Elements that hold a mark inside a line: a footnote's number, or the number
/// of the page that starts there. A mark is a word of its own: Harvard writes
/// `§ 183<footnotemark>1</footnotemark>`, and the mark joined to the number
/// would cite § 1831. Columbia and Lawbox write a footnote's mark in `<sup>`.
const MARKS: [&str; 3] = ["footnotemark", "page-number", "sup"];

/// Classes that make a link or a span a mark. The converted texts write
/// `767.<a class="footnote">11</a>`, where the mark joined to the number would
/// make it `767.11`, and `(14A), <a class="page-label">*773</a>507(a)`.
const MARK_CLASSES: [&str; 3] = ["footnote", "page-label", "star-pagination"];

/// What a tag puts in the text in its place.
enum TagKind {
    /// Nothing: a tag inside a line ([`INLINE`]).
    Inline,
    /// A space, so that the mark is a word of its own ([`MARKS`],
    /// [`MARK_CLASSES`]).
    Mark,
    /// A newline: every other tag.
    LineEnd,
}

/// What a tag, the text between `<` and `>`, puts in the text.
///
/// A closing tag has no class, so `</a>` after a footnote mark is
/// [`TagKind::Inline`]. That is enough: [`push_words`] keeps the mark apart
/// from a word that follows it.
fn kind_of(tag: &str) -> TagKind {
    let name = element_name(tag);
    if MARKS.contains(&name.as_str()) {
        TagKind::Mark
    } else if INLINE.contains(&name.as_str()) {
        match MARK_CLASSES.iter().any(|class| has_class(tag, class)) {
            true => TagKind::Mark,
            false => TagKind::Inline,
        }
    } else {
        TagKind::LineEnd
    }
}

/// Replace every `<...>` with what [`kind_of`] says: nothing, a space or a
/// newline.
///
/// A newline where a block ends, because `</p><p>` between two words is a word
/// boundary and joining them would invent a word, and because the result is
/// meant to be readable. An element this pass does not name is read as one
/// that ends a line, because a line break too many joins no words.
fn strip_tags(markup: &str) -> String {
    let mut out = String::with_capacity(markup.len());
    let mut rest = markup;

    while let Some(open) = rest.find('<') {
        push_words(&mut out, &rest[..open]);
        let Some(length) = rest[open..].find('>') else {
            // A tag that no `>` closes runs to the end of the markup.
            out.push('\n');
            return out;
        };
        match kind_of(&rest[open + 1..open + length]) {
            TagKind::Inline => {}
            TagKind::Mark => out.push(' '),
            TagKind::LineEnd => out.push('\n'),
        }
        rest = &rest[open + length + 1..];
    }

    push_words(&mut out, rest);
    out
}

/// Add the words that follow a tag, and keep them apart from the words before
/// it.
///
/// Words come only after a tag, so a letter or digit at the end of `out` and
/// another at the start of `words` mean that inline tags alone stood between
/// them. Harvard writes `<s>tens of</s>thousands`, with no space, and the two
/// words joined would be a word the opinion does not hold.
fn push_words(out: &mut String, words: &str) {
    let joins_words = out.chars().next_back().is_some_and(char::is_alphanumeric)
        && words.chars().next().is_some_and(char::is_alphanumeric);
    if joins_words {
        out.push(' ');
    }
    out.push_str(words);
}

/// The value of one attribute of a tag, when it has it.
pub(super) fn attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let pattern = format!("{name}=\"");
    let mut from = 0;
    while let Some(found) = tag[from..].find(&pattern) {
        let at = from + found;
        // `label=` must not match the end of `data-label=`.
        let whole = tag[..at]
            .chars()
            .next_back()
            .is_none_or(char::is_whitespace);
        let value_start = at + pattern.len();
        if whole {
            let value_end = tag[value_start..].find('"')? + value_start;
            return Some(&tag[value_start..value_end]);
        }
        from = value_start;
    }
    None
}

/// Whether a tag's `class` names this class.
pub(super) fn has_class(tag: &str, class: &str) -> bool {
    attribute(tag, "class").is_some_and(|classes| classes.split_whitespace().any(|c| c == class))
}

/// The element name of a tag, in lower case: `em` for `</em>` and for
/// `<em class="x">`.
pub(super) fn element_name(tag: &str) -> String {
    tag.trim_start_matches('/')
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect::<String>()
        .to_ascii_lowercase()
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
