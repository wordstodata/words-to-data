//! The OLRC's footnotes, taken out of the words of a provision (#416).
//!
//! The Office of the Law Revision Counsel prints editorial footnotes in the
//! Code: "So in original. The comma probably should not appear.", "See
//! References in Text note below.". USLM writes the mark and the footnote text
//! inside the provision's own text elements:
//!
//! ```xml
//! <chapeau>… December 31, 2017,<ref class="footnoteRef" idref="fn002075">2</ref><note
//!   type="footnote" id="fn002075"><num>2</num> So in original. The comma
//!   probably should not appear.</note>—</chapeau>
//! ```
//!
//! They are not the words of the law, and the OLRC numbers them again in each
//! printing. While the parser kept them in the words, a footnote that only got a
//! new number was a change to the law in `diff`, and `coverage` put it in the
//! work queue.

use roxmltree::Node;

/// One footnote the OLRC printed in the text of a provision.
pub(crate) struct Footnote {
    /// The footnote text, without its number: "So in original. The comma
    /// probably should not appear.". `None` when the document does not hold
    /// the text a mark points at.
    pub text: Option<String>,
    /// The text element that held the mark: `chapeau`, `content`, `num`, ….
    pub field: &'static str,
    /// How many characters of the element's words come before the mark.
    pub at: usize,
}

/// The words of a text element, and the footnotes taken out of them, in the
/// order the element prints them.
pub(crate) struct Words {
    pub text: String,
    pub footnotes: Vec<Footnote>,
}

/// Read a text element: its words without its footnotes, and the footnotes.
///
/// `field` names the element, `chapeau` or `content`, so that each footnote
/// can say where its mark was.
pub(crate) fn read(element: &Node, field: &'static str) -> Words {
    let mut words = Words {
        text: String::new(),
        footnotes: Vec::new(),
    };
    collect(element, field, &mut words);
    words
}

/// The words of a text element, without its footnotes.
pub(crate) fn words_of(element: &Node) -> String {
    read(element, "").text
}

impl Words {
    fn add_footnote(&mut self, field: &'static str, text: Option<String>) {
        let at = self.text.chars().count();
        self.footnotes.push(Footnote { text, field, at });
    }
}

fn collect(element: &Node, field: &'static str, into: &mut Words) {
    for child in element.children() {
        if child.is_text() {
            into.text.push_str(child.text().unwrap_or_default());
        } else if is_footnote_text(&child) {
            into.add_footnote(field, Some(footnote_text(&child)));
        } else if is_footnote_ref(&child) {
            take_space_before_mark(&mut into.text);
            // A mark with its footnote text beside it is read with that text.
            if !has_its_text_beside(&child) {
                let text = note_by_id(&child).map(|note| footnote_text(&note));
                into.add_footnote(field, text);
            }
        } else if is_mark(&child, &into.text) {
            take_space_before_mark(&mut into.text);
            let text = note_by_number(&child).map(|note| footnote_text(&note));
            into.add_footnote(field, text);
        } else if child.is_element() {
            collect(&child, field, into);
        }
    }
}

/// The OLRC sets a mark apart from the word before it with a narrow no-break
/// space. The space is part of the mark, so it goes with the mark: "for whom
/// a\u{202f}<mark> American" reads "for whom a American".
fn take_space_before_mark(text: &mut String) {
    if text.ends_with(NARROW_NO_BREAK_SPACE) {
        text.pop();
    }
}

const NARROW_NO_BREAK_SPACE: char = '\u{202F}';

fn is_footnote_ref(element: &Node) -> bool {
    element.attribute("class") == Some("footnoteRef")
}

/// Whether the footnote text a mark points at follows the mark, as it does at
/// the first mark for a footnote.
fn has_its_text_beside(mark: &Node) -> bool {
    mark.next_sibling_element().is_some_and(|next| {
        is_footnote_text(&next) && next.attribute("id") == mark.attribute("idref")
    })
}

/// Where the OLRC printed the footnote text a second mark points at: in the
/// same section, or failing that anywhere in the document.
fn where_to_look<'a, 'input>(mark: &Node<'a, 'input>) -> Node<'a, 'input> {
    mark.ancestors()
        .find(|n| n.has_tag_name("section"))
        .unwrap_or_else(|| mark.document().root())
}

/// The footnote text a `<ref class="footnoteRef" idref="…">` points at.
fn note_by_id<'a, 'input>(mark: &Node<'a, 'input>) -> Option<Node<'a, 'input>> {
    let id = mark.attribute("idref")?;
    let is_it = |n: &Node| is_footnote_text(n) && n.attribute("id") == Some(id);
    where_to_look(mark)
        .descendants()
        .find(is_it)
        .or_else(|| mark.document().descendants().find(is_it))
}

/// The footnote text a bare `<sup>2</sup>` points at: footnote 2 of the same
/// section, the nearest one printed before the mark, or else the first one
/// after it. The OLRC numbers footnotes within a section.
fn note_by_number<'a, 'input>(mark: &Node<'a, 'input>) -> Option<Node<'a, 'input>> {
    let number = mark.text()?;
    let numbered = |n: &Node| {
        is_footnote_text(n)
            && n.children()
                .find(|c| c.has_tag_name("num"))
                .is_some_and(|num| num.text() == Some(number))
    };
    let at = mark.range().start;
    let (before, after): (Vec<Node>, Vec<Node>) = where_to_look(mark)
        .descendants()
        .filter(numbered)
        .partition(|note| note.range().start < at);
    before.last().or(after.first()).copied()
}

/// Whether a `<sup>` is a footnote mark rather than part of the words.
///
/// The OLRC prints a second mark for a footnote as a bare `<sup>1</sup>`, with
/// no footnote text beside it: "sections 23(g) and 137(e),<sup>2</sup>". The
/// same element also holds the words of the law: the top of a fraction,
/// `<sup>1</sup>⁄<sub>12</sub>`, and a power or an isotope, `kg/m<sup>2</sup>`,
/// `U<sup>235</sup>`, `<sup>235</sup>U`. A mark is digits alone, and stands
/// apart from the letters and digits on each side of it, and before no
/// fraction slash.
///
/// `before` is the text read so far, so its last character is the one before
/// the element.
fn is_mark(element: &Node, before: &str) -> bool {
    let digits = element.text().unwrap_or_default();
    if !element.has_tag_name("sup")
        || element.first_element_child().is_some()
        || digits.is_empty()
        || !digits.chars().all(|c| c.is_ascii_digit())
    {
        return false;
    }
    let joins = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == FRACTION_SLASH);
    let after = element.next_sibling().and_then(|next| match next.is_text() {
        true => next.text().and_then(|text| text.chars().next()),
        false => words_of(&next).chars().next(),
    });
    !joins(before.chars().last()) && !joins(after)
}

/// U+2044, which the Code prints between the two halves of a fraction.
const FRACTION_SLASH: char = '\u{2044}';

fn is_footnote_text(element: &Node) -> bool {
    element.has_tag_name("note") && element.attribute("type") == Some("footnote")
}

/// The text of a `<note type="footnote">`, without the number it starts with.
fn footnote_text(note: &Node) -> String {
    let mut text = String::new();
    for child in note.children() {
        if child.is_text() {
            text.push_str(child.text().unwrap_or_default());
        } else if child.is_element() && !child.has_tag_name("num") {
            text.push_str(&words_of(&child));
        }
    }
    text.trim().to_string()
}
