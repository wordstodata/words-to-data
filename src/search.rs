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
//! - A ligature in [`LIGATURES`] is read as its letters, so "qualified"
//!   matches "quali\u{fb01}ed". Opinions stored from a PDF hold them (#338).
//! - The characters in [`IGNORED`] are not read. The Code puts a soft hyphen
//!   inside about 600 words of each printing, such as "rec\u{ad}ords" (#338).
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

/// Characters that search does not read, in the text or in the query.
///
/// Each is a format character: it does not show in the text, and a person who
/// types the words does not type it. The soft hyphen (U+00AD) is the one the
/// Code holds; the zero-width characters and the byte order mark are the others
/// of the same kind.
pub const IGNORED: &[char] = &[
    '\u{00AD}', // soft hyphen
    '\u{200B}', // zero width space
    '\u{200C}', // zero width non-joiner
    '\u{200D}', // zero width joiner
    '\u{2060}', // word joiner
    '\u{FEFF}', // zero width no-break space (byte order mark)
];

/// The Latin ligatures, each with the letters it is written for.
///
/// These are the characters U+FB00 to U+FB06, and the letters are their
/// compatibility decomposition (NFKC). A PDF or an OCR text can hold them;
/// a person types the letters.
pub const LIGATURES: &[(char, &str)] = &[
    ('\u{FB00}', "ff"),
    ('\u{FB01}', "fi"),
    ('\u{FB02}', "fl"),
    ('\u{FB03}', "ffi"),
    ('\u{FB04}', "ffl"),
    ('\u{FB05}', "st"),
    ('\u{FB06}', "st"),
];

/// A search query, ready to compare with text.
#[derive(Debug, Clone)]
pub struct Query {
    /// The query as it is compared: see [`comparable`].
    wanted: Vec<char>,
    /// The query as [`bare`] reads it, for the quick test in [`Query::is_in`].
    bare: String,
}

impl Query {
    pub fn new(query: &str) -> Self {
        let wanted = comparable(query).into_iter().map(|(c, _)| c).collect();
        Self {
            wanted,
            bare: bare(query),
        }
    }

    /// Whether `text` holds at least one match.
    ///
    /// A back end uses this to keep only the fields that match, and then asks
    /// [`Query::occurrences`] where the matches are. The rule is the one rule
    /// of this module: a back end cannot state it in its own terms, such as a
    /// SQL `LIKE` pattern, because a character that search does not read can
    /// be inside any word.
    ///
    /// Most fields hold no match, so a quick test comes first: the field read
    /// by [`bare`] must hold the query read by [`bare`]. That test cannot
    /// refuse a field that matches, because each rule after [`read_char`]
    /// only takes out spaces and hyphens, and [`bare`] takes out all of them.
    pub fn is_in(&self, text: &str) -> bool {
        bare(text).contains(&self.bare) && !self.occurrences(text).is_empty()
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
/// Each character in [`IGNORED`] is dropped, each ligature becomes its
/// letters, each other character is lowercased,
/// each run of white space becomes one space, and each spelling in
/// [`CLOSED_UP`] loses its spaces. Lowercasing can change how many characters a string holds, so the
/// offsets are carried along rather than counted again: an offset counted in
/// the compared form would point at the wrong place in the original.
fn comparable(text: &str) -> Vec<(char, usize)> {
    let mut folded: Vec<(char, usize)> = Vec::new();
    for (offset, c) in text.chars().enumerate() {
        read_char(c, |read| {
            // A run of white space keeps the offset of its first character.
            if read != ' ' || folded.last().is_none_or(|(last, _)| *last != ' ') {
                folded.push((read, offset));
            }
        });
    }
    for spelling in CLOSED_UP {
        folded = close_up(folded, spelling);
    }
    folded
}

/// The characters that search reads for `c`, given one at a time to `read`.
///
/// A character in [`IGNORED`] gives none, white space gives one space, a
/// ligature in [`LIGATURES`] gives its letters, and any other character gives
/// its lower case. This is the part of the rule that
/// reads one character alone; [`comparable`] and [`bare`] both start here, so
/// they cannot come to disagree about it.
fn read_char(c: char, mut read: impl FnMut(char)) {
    // Most of the corpus is ASCII, and no ASCII character is in `IGNORED`.
    // This path gives the same characters as the general one, faster.
    if c.is_ascii() {
        read(if c.is_whitespace() { ' ' } else { c.to_ascii_lowercase() });
        return;
    }
    if IGNORED.contains(&c) {
        return;
    }
    if c.is_whitespace() {
        read(' ');
    } else if let Some((_, letters)) = LIGATURES.iter().find(|(ligature, _)| *ligature == c) {
        letters.chars().for_each(read);
    } else {
        c.to_lowercase().for_each(read);
    }
}

/// `text` as [`read_char`] reads it, with no space and no hyphen.
///
/// Quick to make, and with no offsets. [`Query::is_in`] compares the two bare
/// forms before it asks where the matches are.
fn bare(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        read_char(c, |read| {
            if read != ' ' && read != '-' {
                out.push(read);
            }
        });
    }
    out
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
