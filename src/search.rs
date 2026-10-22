//! What search counts as a match, and where each match is inside a field.
//!
//! A field can be long. A court opinion is one field of 6k to 207k characters,
//! so "this field matched" says little: the Boeing opinion names § 174 five
//! times, and a search that gave one hit for the field hid four of them (#306).
//! This module says where each occurrence is, so a caller can show each one and
//! read the text around it.
//!
//! The rule for a match is here and nowhere else, so the two back ends and the
//! offsets cannot come to disagree about it:
//!
//! - Case is ignored.
//! - A run of white space is one space, so a line break or several spaces in
//!   the text match one space in the query. Plain-text opinions break lines
//!   inside phrases (#307).
//! - The spellings in [`CLOSED_UP`] are read without their spaces, so "U. S. C."
//!   and "U.S.C." match each other. Court opinions write the citation both
//!   ways (#307).
//!
//! Offsets are counted in characters, not bytes. The corpus holds `§`, and a
//! byte offset that a reader takes as a character offset points at the wrong
//! place.

/// Spellings that search reads with their spaces taken out, written in lower
/// case.
///
/// The list is short on purpose. It is not fuzzy matching: each entry is one
/// spelling that the sources really use for another. "U.S.C.A." is not here,
/// and stays a different spelling.
pub const CLOSED_UP: &[&str] = &["u. s. c."];

/// A search query, ready to compare with text.
#[derive(Debug, Clone)]
pub struct Query {
    /// The query as it is compared: see [`comparable`].
    wanted: Vec<char>,
    /// Runs of characters that every match holds, in order.
    pieces: Vec<String>,
}

impl Query {
    pub fn new(query: &str) -> Self {
        let wanted: Vec<char> = comparable(query).into_iter().map(|(c, _)| c).collect();

        // A spelling that was closed up can appear with its spaces in the
        // text, so it gives one piece for each part between the spaces. The
        // closed-up form holds each part too, in the same order.
        let mut spaced: String = wanted.iter().collect();
        for spelling in CLOSED_UP {
            spaced = spaced.replace(&closed_up(spelling), spelling);
        }
        let pieces = spaced.split_whitespace().map(str::to_string).collect();

        Self { wanted, pieces }
    }

    /// Runs of lower-case characters that every match holds, in this order.
    ///
    /// A necessary condition and not a sufficient one: a back end can use it
    /// to skip text quickly, for example as a SQL `LIKE` pattern, and must then
    /// ask [`Query::occurrences`] for the answer.
    pub fn pieces(&self) -> &[String] {
        &self.pieces
    }

    /// The character offset of each match in `text`, first to last.
    ///
    /// Matches do not overlap: after a match, the scan continues after it,
    /// which is how a reader counts mentions.
    ///
    /// An empty query matches once, at the start of the field. Search reads an
    /// empty query as "every field", and a field that matched holds at least
    /// one occurrence.
    pub fn occurrences(&self, text: &str) -> Vec<usize> {
        if self.wanted.is_empty() {
            return vec![0];
        }
        let text = comparable(text);

        let mut found = Vec::new();
        let mut at = 0;
        while at + self.wanted.len() <= text.len() {
            let here = &text[at..at + self.wanted.len()];
            if here.iter().map(|(c, _)| c).eq(&self.wanted) {
                found.push(text[at].1);
                at += self.wanted.len();
            } else {
                at += 1;
            }
        }
        found
    }
}

/// The character offset of each occurrence of `query` in `text`, first to last.
///
/// See [`Query::occurrences`]. Build a [`Query`] once to compare one query with
/// many fields.
///
/// ```
/// use words_to_data::search::occurrences;
///
/// assert_eq!(occurrences("26 U.S.C. § 174 and § 174(a)", "174"), vec![12, 22]);
/// assert_eq!(occurrences("Section 174", "SECTION"), vec![0]);
/// assert_eq!(occurrences("Section 174", ""), vec![0]);
/// assert_eq!(occurrences("26 U. S. C. § 174", "U.S.C. § 174"), vec![3]);
/// assert_eq!(occurrences("qualified research\neach year", "research each"), vec![10]);
/// ```
pub fn occurrences(text: &str, query: &str) -> Vec<usize> {
    Query::new(query).occurrences(text)
}

/// The text as search compares it, each character with the offset of the
/// character in `text` it came from.
///
/// Each character is lowercased, each run of white space becomes one space,
/// and each spelling in [`CLOSED_UP`] loses its spaces. Lowercasing can change how many characters a string holds, so the
/// offsets are carried along rather than counted again: an offset counted in
/// the compared form would point at the wrong place in the original.
fn comparable(text: &str) -> Vec<(char, usize)> {
    let mut folded: Vec<(char, usize)> = Vec::new();
    for (offset, c) in text.chars().enumerate() {
        if c.is_whitespace() {
            // A run keeps the offset of its first character.
            if folded.last().is_none_or(|(last, _)| *last != ' ') {
                folded.push((' ', offset));
            }
        } else {
            folded.extend(c.to_lowercase().map(|lower| (lower, offset)));
        }
    }
    for spelling in CLOSED_UP {
        folded = close_up(folded, spelling);
    }
    folded
}

/// `folded` with the spaces taken out of each place that spells `spelling`.
fn close_up(folded: Vec<(char, usize)>, spelling: &str) -> Vec<(char, usize)> {
    let spelling: Vec<char> = spelling.chars().collect();
    let mut out = Vec::with_capacity(folded.len());
    let mut at = 0;
    while at < folded.len() {
        let end = at + spelling.len();
        if end <= folded.len() && folded[at..end].iter().map(|(c, _)| c).eq(&spelling) {
            out.extend(folded[at..end].iter().filter(|(c, _)| !c.is_whitespace()));
            at = end;
        } else {
            out.push(folded[at]);
            at += 1;
        }
    }
    out
}

/// `spelling` with its spaces taken out.
fn closed_up(spelling: &str) -> String {
    spelling.chars().filter(|c| !c.is_whitespace()).collect()
}
