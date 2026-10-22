//! Where a search query occurs inside one field of text.
//!
//! A field can be long. A court opinion is one field of 6k to 207k characters,
//! so "this field matched" says little: the Boeing opinion names § 174 five
//! times, and a search that gave one hit for the field hid four of them (#306).
//! This module says where each occurrence is, so a caller can show each one and
//! read the text around it.
//!
//! Offsets are counted in characters, not bytes. The corpus holds `§`, and a
//! byte offset that a reader takes as a character offset points at the wrong
//! place.

/// The character offset of each occurrence of `query` in `text`, first to last.
///
/// The comparison ignores case, as search does. Occurrences do not overlap: after
/// a match, the scan continues after its last character, which is how a reader
/// counts mentions.
///
/// An empty query occurs once, at the start of the field. Search reads an empty
/// query as "every field", and a field that matched holds at least one
/// occurrence.
///
/// ```
/// use words_to_data::search::occurrences;
///
/// assert_eq!(occurrences("26 U.S.C. § 174 and § 174(a)", "174"), vec![12, 22]);
/// assert_eq!(occurrences("Section 174", "SECTION"), vec![0]);
/// assert_eq!(occurrences("Section 174", ""), vec![0]);
/// ```
pub fn occurrences(text: &str, query: &str) -> Vec<usize> {
    let wanted: Vec<char> = query.chars().collect();
    if wanted.is_empty() {
        return vec![0];
    }
    let characters: Vec<char> = text.chars().collect();

    let mut found = Vec::new();
    let mut at = 0;
    while at + wanted.len() <= characters.len() {
        if same_ignoring_case(&characters[at..at + wanted.len()], &wanted) {
            found.push(at);
            at += wanted.len();
        } else {
            at += 1;
        }
    }
    found
}

/// Whether two runs of characters are equal when case is ignored.
///
/// Compared character by character rather than on a lowercased copy of the
/// whole field, because lowercasing can change how many characters a string
/// holds, and an offset read off the copy would then point at the wrong place in
/// the original.
fn same_ignoring_case(found: &[char], asked: &[char]) -> bool {
    found
        .iter()
        .zip(asked)
        .all(|(found, asked)| found.to_lowercase().eq(asked.to_lowercase()))
}
