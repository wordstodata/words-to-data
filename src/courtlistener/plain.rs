//! Reading a `plain_text` field into paragraphs and footnotes, by rule.
//!
//! `plain_text` is the text layer of a court's PDF. Nothing in it marks a
//! paragraph: a line ends where the printed line ended, a page ends with a form
//! feed, and the page number, the running head and the footnotes are printed
//! in the same stream as the words. A blank line is no paragraph break either:
//! *Harris* (5th Cir. 1994) is double spaced, so its text has a blank line
//! between every two lines, 374 blank-line blocks for 36 paragraphs.
//!
//! So the rule [`PARAGRAPHS`] reads the layout a typesetter left:
//!
//! 1. **Pages.** The text is split at each form feed.
//! 2. **Page furniture comes off.** A lone number at the foot of a page is its
//!    page number. A line at the head of a page is a running head when it also
//!    prints the page number, or when the same line, numbers aside, heads half
//!    the pages or more.
//! 3. **Footnotes come out of the body.** The footnotes of a page start after
//!    a rule of em dashes, as the Supreme Court prints them. In any other text
//!    they start at the next footnote number, on a line of its own or at the
//!    start of an indented line, or at a footnote's stars; and in a
//!    double-spaced text also after two blank lines, where a footnote carried
//!    over from the page before goes on.
//! 4. **Paragraphs.** A paragraph starts where a line is indented further than
//!    the line before it; where a line comes back from a block quotation or a
//!    centred heading; where two indented lines stand a blank line apart, or
//!    the first of them is a whole item of a list; after the line that opens
//!    a writing, "JUSTICE KENNEDY delivered the opinion of the Court."; and, in
//!    a text that is not double spaced, after any blank line. Every other line
//!    goes on: a paragraph runs over a page break and round a footnote.
//! 5. **Lines are joined** with one space between two words. A word broken at a
//!    hyphen at the end of a line keeps the hyphen and loses the space, so no
//!    character the court printed is lost or added.
//!
//! A rule is a machine's reading, so every paragraph it finds is
//! `MachineSuggested`, and its provenance names the rule.

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;

use super::structure::{Passage, PassageKind};
use super::writings::opens_writing;

/// The rule that reads `plain_text` into paragraphs and footnotes.
pub(crate) const PARAGRAPHS: &str = "plain-text-paragraphs";

/// A line that prints only a number: a page number, or a footnote's label.
static LONE_NUMBER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(\d{1,4})\s*$").expect("valid pattern"));

/// A footnote that opens with its number and its first words on one indented
/// line: "        1 Unless otherwise indicated".
static NUMBER_THEN_WORDS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s+(\d{1,3})\s+\S").expect("valid pattern"));

/// The stars that mark a footnote with no number, on a line of their own or
/// before its first words: "*", "**".
static STARS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(\*{1,3})(?:\s+(\S.*))?$").expect("valid pattern"));

/// The rule of em dashes the Supreme Court prints above its footnotes.
static FOOTNOTE_RULE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*—{4,}\s*$").expect("valid pattern"));

/// A line at the head of a page that prints the page number too:
/// "2     OBERGEFELL v. HODGES" or "Cite as: 576 U. S. ____ (2015)     3".
static NUMBERED_HEAD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*\d{1,4}\s{3,}\S|\S\s{3,}\d{1,4}\s*$").expect("valid pattern")
});

/// A run of pages that carry the same running head: in a Supreme Court slip
/// opinion, one writing, or the syllabus.
#[derive(Debug, Clone)]
pub(crate) struct Section {
    /// The running head that names the writing, such as "Opinion of the
    /// Court" or "ROBERTS, C. J., dissenting". `None` when the pages print
    /// none.
    pub running_head: Option<String>,
    /// Its paragraphs, then its footnotes.
    pub passages: Vec<Passage>,
}

/// Read the paragraphs and footnotes of a `plain_text` field, in order, in
/// sections by running head.
///
/// A page with no running head that names a writing goes on with the section
/// before it, and so does an appendix to a writing. Each writing numbers its
/// footnotes from 1, so each section reads its own.
pub(crate) fn read_plain_text(text: &str) -> Vec<Section> {
    let pages: Vec<Vec<&str>> = text
        .split('\u{c}')
        .map(|page| page.lines().map(str::trim_end).collect())
        .collect();
    let heads = running_heads(&pages);
    let layout = Layout::of(&pages);
    let mut sections: Vec<(Option<String>, Vec<Line>, Footnotes)> = Vec::new();
    for page in &pages {
        let (lines, label) = without_furniture(page, &heads);
        let opens = match (&label, sections.last()) {
            (_, None) => true,
            (Some(label), Some((current, _, _))) => {
                !label.contains("Appendix") && current.as_ref() != Some(label)
            }
            (None, Some(_)) => false,
        };
        if opens {
            sections.push((label, Vec::new(), Footnotes::default()));
        }
        let (_, body, footnotes) = sections.last_mut().expect("a section is open");
        let (page_body, page_notes) = split_footnotes(lines, footnotes, layout);
        footnotes.read(page_notes);
        body.extend(from_page_margin(page_body));
    }
    sections
        .into_iter()
        .map(|(running_head, body, footnotes)| {
            let mut passages = paragraphs(&body, layout);
            passages.extend(footnotes.into_passages());
            Section {
                running_head,
                passages,
            }
        })
        .collect()
}

/// One line of the body, with what the paragraph rule needs to know of it.
#[derive(Debug, Clone, Copy)]
struct Line<'a> {
    text: &'a str,
    /// How far it is indented past the left margin of its page. The margin
    /// moves from page to page, as a book's does.
    indent: usize,
    /// How many blank lines stand before it on its page.
    blanks_before: usize,
}

/// The lines of one page, each indented from the page's own margin: the
/// least indent of any of them.
fn from_page_margin(lines: Vec<Line>) -> Vec<Line> {
    let margin = lines.iter().map(|line| line.indent).min().unwrap_or(0);
    lines
        .into_iter()
        .map(|line| Line {
            indent: line.indent - margin,
            ..line
        })
        .collect()
}

fn indent(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

/// A line with each number written `#` and its spaces closed up, so that the
/// running head of page 9 and the running head of page 10 read the same.
fn shape(line: &str) -> String {
    let words = line.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut shaped = String::with_capacity(words.len());
    for c in words.chars() {
        match c.is_ascii_digit() {
            true if shaped.ends_with('#') => {}
            true => shaped.push('#'),
            false => shaped.push(c),
        }
    }
    shaped
}

/// How many lines at the head of a page may be running heads, besides the
/// one that names the writing.
const HEAD_LINES: usize = 3;

/// The lines, by [`shape`], that head half the pages or more.
fn running_heads(pages: &[Vec<&str>]) -> Vec<String> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for page in pages {
        let mut seen = Vec::new();
        for line in page
            .iter()
            .filter(|line| !line.trim().is_empty())
            .take(HEAD_LINES)
        {
            let key = shape(line);
            if !seen.contains(&key) {
                *counts.entry(key.clone()).or_insert(0) += 1;
                seen.push(key);
            }
        }
    }
    counts
        .into_iter()
        .filter(|(_, count)| *count >= 2 && count * 2 >= pages.len())
        .map(|(key, _)| key)
        .collect()
}

/// The running head of a Supreme Court slip opinion that names the writing a
/// page belongs to: "Syllabus", "Per Curiam", "Opinion of the Court",
/// "Opinion of ALITO, J.", "ROBERTS, C. J., dissenting", "Statement of
/// SOTOMAYOR, J.", and an appendix, which pdftotext may print over two lines.
static WRITING_HEAD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)^(?:
            Syllabus
          | Per\s+Curiam
          | Opinion(?:\s+of\s+.+)?
          | Statement\s+of\s+.+
          | Appendix.*
          | .*\bof\s+the\s+Court
          | [A-Z][A-Z'’\-]+,\s+(?:C\.\s+)?J\.,\s+\S.*
        )$",
    )
    .expect("valid pattern")
});

/// A page without its running heads and its page number, and the running
/// head that names its writing, when it prints one.
fn without_furniture<'a>(page: &[&'a str], heads: &[String]) -> (Vec<&'a str>, Option<String>) {
    let mut lines: Vec<&str> = page.to_vec();
    while lines.last().is_some_and(|line| line.trim().is_empty()) {
        lines.pop();
    }
    if lines.last().is_some_and(|line| LONE_NUMBER.is_match(line)) {
        lines.pop();
    }
    let mut start = 0;
    let mut heads_seen = 0;
    let mut names: Vec<&str> = Vec::new();
    while let Some(offset) = lines[start..]
        .iter()
        .position(|line| !line.trim().is_empty())
    {
        let line = lines[start + offset];
        let names_writing = heads_seen > 0 && WRITING_HEAD.is_match(line.trim());
        let is_head = heads.contains(&shape(line))
            || (heads_seen == 0 && NUMBERED_HEAD.is_match(line))
            || names_writing;
        if heads_seen > HEAD_LINES || !is_head {
            break;
        }
        if names_writing {
            names.push(line.trim());
        }
        heads_seen += 1;
        start += offset + 1;
    }
    // The blank lines under the running head are no break in the words.
    while lines.get(start).is_some_and(|line| line.trim().is_empty()) {
        start += 1;
    }
    let label = (!names.is_empty()).then(|| shape(&names.join(" ")));
    (lines.split_off(start), label)
}

/// What the whole text says about how it is set.
#[derive(Debug, Clone, Copy)]
struct Layout {
    /// Most lines have a blank line after them, so a blank line is no
    /// paragraph break.
    double_spaced: bool,
    /// The footnotes stand under a rule of em dashes, as the Supreme Court
    /// prints them. A number on a line of its own is then a heading of the
    /// body, never a footnote.
    ruled_footnotes: bool,
}

impl Layout {
    /// The first page is left out of the spacing when there are others: its
    /// caption is set in single lines over a double-spaced body.
    fn of(pages: &[Vec<&str>]) -> Self {
        let (mut lines, mut followed) = (0, 0);
        let ruled_footnotes = pages
            .iter()
            .any(|page| page.iter().any(|line| FOOTNOTE_RULE.is_match(line)));
        let body_pages = match pages {
            [first, rest @ ..]
                if rest
                    .iter()
                    .all(|page| page.iter().all(|line| line.trim().is_empty())) =>
            {
                std::slice::from_ref(first)
            }
            [_, rest @ ..] => rest,
            [] => pages,
        };
        for page in body_pages {
            for pair in page.windows(2) {
                if !pair[0].trim().is_empty() {
                    lines += 1;
                    if pair[1].trim().is_empty() {
                        followed += 1;
                    }
                }
            }
        }
        Self {
            double_spaced: lines > 0 && followed * 10 > lines * 6,
            ruled_footnotes,
        }
    }
}

/// The footnotes read so far, and the number the next one should have.
#[derive(Debug, Default)]
struct Footnotes {
    notes: Vec<(String, Vec<String>)>,
}

impl Footnotes {
    /// The number the next footnote should print.
    fn next(&self) -> String {
        let last = self
            .notes
            .iter()
            .rev()
            .find_map(|(label, _)| label.parse::<u32>().ok())
            .unwrap_or(0);
        (last + 1).to_string()
    }

    /// Take the footnote lines of one page: a line that opens with the next
    /// number starts a footnote, and any other line goes on with the one before.
    fn read(&mut self, lines: Vec<&str>) {
        for line in lines {
            if line.trim().is_empty() {
                continue;
            }
            let next = self.next();
            if let Some(found) = LONE_NUMBER.captures(line)
                && found[1] == next
            {
                self.notes.push((next, Vec::new()));
                continue;
            }
            if let Some(found) = NUMBER_THEN_WORDS.captures(line)
                && found[1] == next
            {
                let words = line.trim_start()[next.len()..].to_string();
                self.notes.push((next, vec![words]));
                continue;
            }
            if let Some(found) = STARS.captures(line) {
                let words = found.get(2).map(|words| words.as_str().to_string());
                self.notes
                    .push((found[1].to_string(), words.into_iter().collect()));
                continue;
            }
            match self.notes.last_mut() {
                Some(note) => note.1.push(line.to_string()),
                // The rest of a footnote that started on the page before.
                None => self.notes.push((String::new(), vec![line.to_string()])),
            }
        }
    }

    fn into_passages(self) -> Vec<Passage> {
        self.notes
            .into_iter()
            .map(|(label, lines)| Passage {
                kind: PassageKind::Footnote { label },
                ..Passage::paragraph(joined(lines.iter().map(String::as_str)))
            })
            .collect()
    }
}

/// Split the lines of one page into its body and its footnotes.
fn split_footnotes<'a>(
    lines: Vec<&'a str>,
    footnotes: &Footnotes,
    layout: Layout,
) -> (Vec<Line<'a>>, Vec<&'a str>) {
    let next = footnotes.next();
    let mut blanks = 0;
    let mut body = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if line.trim().is_empty() {
            blanks += 1;
            continue;
        }
        if FOOTNOTE_RULE.is_match(line) {
            return (body, lines[index + 1..].to_vec());
        }
        let numbered =
            |pattern: &Regex| pattern.captures(line).is_some_and(|found| found[1] == next);
        let opens_footnote = !layout.ruled_footnotes
            && (numbered(&LONE_NUMBER)
                || (blanks > 0 && numbered(&NUMBER_THEN_WORDS))
                || (blanks > 0 && STARS.is_match(line) && !body.is_empty())
                || (layout.double_spaced && blanks >= 2 && !body.is_empty()));
        if opens_footnote {
            return (body, lines[index..].to_vec());
        }
        body.push(Line {
            text: line,
            indent: indent(line),
            blanks_before: blanks,
        });
        blanks = 0;
    }
    (body, Vec::new())
}

/// How far past the margin a line starts when it is centred, such as a
/// heading, rather than the indented first line of a paragraph.
const CENTRED: usize = 10;

/// The body's lines, joined into paragraphs.
fn paragraphs(body: &[Line], layout: Layout) -> Vec<Passage> {
    let mut paragraphs: Vec<Vec<&str>> = Vec::new();
    let mut previous: Option<&Line> = None;
    for line in body {
        let here = line.indent;
        let starts = match previous {
            None => true,
            Some(before) => {
                let there = before.indent;
                let first_line = paragraphs.last().is_some_and(|lines| lines.len() == 1);
                let blank = line.blanks_before > 0;
                (!layout.double_spaced && blank)
                    // "JUSTICE KENNEDY delivered the opinion of the Court."
                    || opens_writing(before.text)
                    // A first line, a block quotation, a heading.
                    || here > there
                    // Two indented lines a blank line apart: two first lines.
                    || (here == there && here > 0 && blank)
                    // Two indented lines, the first a whole sentence or
                    // citation of its own, as in a list.
                    || (here == there && here > 0 && first_line && !layout.double_spaced && item_ends(before.text, line.text))
                    // Back from a block quotation or a heading. Back from an
                    // indented first line is the same paragraph.
                    || (here < there && (!first_line || there > CENTRED))
            }
        };
        if starts {
            paragraphs.push(Vec::new());
        }
        paragraphs
            .last_mut()
            .expect("a paragraph is open")
            .push(line.text);
        previous = Some(line);
    }
    paragraphs
        .into_iter()
        .map(|lines| Passage::paragraph(joined(lines)))
        .collect()
}

/// Whether a line of one line ends an item of a list, and the next line
/// starts another: the first ends a sentence or a citation, and the second
/// opens with a capital letter.
fn item_ends(line: &str, next: &str) -> bool {
    let ends = line.trim_end().ends_with(['.', ')', ';']);
    let opens = next
        .trim_start()
        .chars()
        .next()
        .is_some_and(char::is_uppercase);
    ends && opens
}

/// Lines joined into one run of words, one space between two words. A word
/// broken at a hyphen keeps its hyphen and loses the space.
fn joined<'a>(lines: impl IntoIterator<Item = &'a str>) -> String {
    let mut text = String::new();
    for line in lines {
        let words = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if words.is_empty() {
            continue;
        }
        let broken = text.ends_with('-') || text.ends_with('—');
        let continues = words.chars().next().is_some_and(char::is_lowercase);
        if !text.is_empty() && !(broken && continues) {
            text.push(' ');
        }
        text.push_str(&words);
    }
    text
}
