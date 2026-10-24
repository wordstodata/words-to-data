//! Reading the inside of a writing: its paragraphs, the reporter page each one
//! starts on, and its footnotes.
//!
//! Two fields carry this structure, and nothing else does
//! (`docs/adr/0014-a-case-is-a-work-and-a-citation-runs-paragraph-to-subdivision.md`,
//! decisions 3 and 4):
//!
//! - **`xml_harvard`**, from Harvard's Caselaw Access Project, in three
//!   spellings. CAP's own XML marks a page break with
//!   `<page-number label="441">*441</page-number>` and a footnote with
//!   `<footnote label="1">`. CourtListener's conversion of it writes
//!   `<span class="star-pagination" label="564">` or
//!   `<a class="page-label" data-label="2594">`, and
//!   `<div class="footnote" label="53">`. Each paragraph carries CAP's id,
//!   such as `b693-8`.
//! - **`html_columbia`**, from the Columbia archive, marks a page break with
//!   `<span class="star-pagination">*Page 940</span>` and a footnote with
//!   `<div class="footnote">`, and gives no paragraph an id.
//!
//! This is deliberately not an XML parser. Columbia's markup is not well formed:
//! a footnote opens inside a `<p>` and that `<p>` closes inside the footnote.
//! So the reader walks the tags, keeps only what it needs, and loses no words:
//! text outside any paragraph becomes a paragraph of its own.
//!
//! The words of each passage are read by [`text_of_markup`], so a passage holds
//! exactly what the flat reading held, and a page mark's own text, such as
//! `*441`, stays where the page breaks.

use super::markup::{MarkupReport, text_of_markup};

impl Passage {
    /// A paragraph on no page, with no ids and no footnote references.
    pub fn paragraph(text: String) -> Self {
        Self {
            kind: PassageKind::Paragraph { page: None },
            text,
            source_ids: Vec::new(),
            footnote_marks: Vec::new(),
            pages_inside: Vec::new(),
        }
    }
}

/// One passage of a writing, in document order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Passage {
    pub kind: PassageKind,
    /// The words, with the markup off.
    pub text: String,
    /// The ids the source gives the paragraphs of this passage, such as CAP's
    /// `b693-8`. Empty when the source gives none.
    pub source_ids: Vec<String>,
    /// The label of each footnote this passage refers to, in order, when the
    /// source marks the reference: `1` for `<a class="footnote">1</a>`.
    pub footnote_marks: Vec<String>,
    /// The label of each page marked inside this paragraph, after its first
    /// words, in order: a page on which the paragraph runs on, and on which no
    /// paragraph may start. Empty for a footnote.
    pub pages_inside: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PassageKind {
    /// A paragraph of the body, with the label of the page it starts on, when
    /// the source labels pages.
    Paragraph { page: Option<String> },
    /// A footnote, with the label the source prints for it.
    Footnote { label: String },
}

/// The passages of one writing, and what the pass could not decode.
#[derive(Debug, Clone, Default)]
pub(crate) struct Structure {
    pub passages: Vec<Passage>,
    pub report: MarkupReport,
    /// The label of the first page mark in the body, such as `441`, when the
    /// source marks pages.
    pub first_page_mark: Option<String>,
    /// Every page the body is printed on, in order, once each: the page it
    /// starts on, then each page marked. A page on which no paragraph starts is
    /// here too, so a pin cite to it still names a path.
    pub pages: Vec<String>,
}

impl Structure {
    /// Passages that some reader other than [`read_structure`] found, such as
    /// one writing of several in a combined record. Its pages are the pages
    /// its paragraphs start on and the pages they run on, so a page on which
    /// no paragraph starts is a path in the writing that is printed on it.
    pub fn of_passages(passages: Vec<Passage>) -> Self {
        let mut pages: Vec<String> = Vec::new();
        for passage in &passages {
            if let PassageKind::Paragraph { page: Some(page) } = &passage.kind {
                for page in std::iter::once(page).chain(&passage.pages_inside) {
                    if !pages.contains(page) {
                        pages.push(page.clone());
                    }
                }
            }
        }
        Self {
            passages,
            report: MarkupReport::default(),
            first_page_mark: None,
            pages,
        }
    }

    /// The same passages with no page: what a writing gets when the pages the
    /// source labels are not pages of the official reporter.
    pub fn without_pages(mut self) -> Self {
        for passage in &mut self.passages {
            if let PassageKind::Paragraph { page } = &mut passage.kind {
                *page = None;
            }
            passage.pages_inside.clear();
        }
        self.first_page_mark = None;
        self.pages.clear();
        self
    }

    /// The same passages with only the pages of one reporter: the reporter of
    /// the first page.
    ///
    /// Harvard's text of *United States v. Windsor* marks the pages of 570 U.S.
    /// and of 133 S. Ct. 2675 in one run, with the same markup: `*776`,
    /// `*2697`, `*777`. A page of the parallel reporter is not a path (ADR 0014,
    /// decision 4), so its mark is no page break here, and a paragraph that
    /// starts after it starts on the page of the first reporter before it
    /// (#341). The mark's own text stays in the words.
    ///
    /// Which reporter a mark counts is not in the markup, so the rule reads it
    /// from the numbers. Each page of a writing is marked, so two pages of one
    /// reporter in one writing are never further apart than the count of pages
    /// marked. A footnote can go back to an earlier page (*Boeing* goes back 15
    /// pages of 19), and that is inside the bound. The pages of the parallel
    /// reporter in *Windsor* are 1,921 pages away from those of 570 U.S. A label
    /// that is not a number is kept, because the rule cannot measure it.
    pub fn pages_of_one_reporter(mut self) -> Self {
        let bound = self.pages.len() as u64;
        let mut last: Option<u64> = None;
        let mut kept: Vec<String> = Vec::new();
        for label in &self.pages {
            let near = match (label.parse::<u64>(), last) {
                (Ok(page), Some(before)) => page.abs_diff(before) <= bound,
                _ => true,
            };
            if near {
                if let Ok(page) = label.parse::<u64>() {
                    last = Some(page);
                }
                kept.push(label.clone());
            }
        }

        let mut current: Option<String> = None;
        for passage in &mut self.passages {
            passage.pages_inside.retain(|page| kept.contains(page));
            if let PassageKind::Paragraph { page: Some(page) } = &mut passage.kind {
                if kept.contains(page) {
                    current = Some(page.clone());
                } else if let Some(official) = &current {
                    *page = official.clone();
                }
            }
        }
        self.pages = kept;
        self
    }
}

/// Elements that hold one paragraph. CAP gives each of them an id.
const BLOCKS: [&str; 12] = [
    "p",
    "blockquote",
    "author",
    "judges",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "pre",
    "center",
];

/// Read the passages of one writing out of its markup.
pub(crate) fn read_structure(markup: &str) -> Structure {
    let mut reader = Reader::new(markup);
    reader.run();
    reader.finish()
}

/// One tag, as it is written.
struct Tag<'a> {
    /// The element name, in lower case.
    name: String,
    closing: bool,
    /// Everything between `<` and `>`.
    inner: &'a str,
    /// Where the tag starts and where it ends, in bytes.
    start: usize,
    end: usize,
}

/// The value of one attribute of a tag, when it has it.
fn attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
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
fn has_class(tag: &str, class: &str) -> bool {
    attribute(tag, "class").is_some_and(|classes| classes.split_whitespace().any(|c| c == class))
}

/// The next tag at or after `from`, when there is one.
///
/// `<?xml …?>` and a comment come back with an empty name: they are no
/// element, and no words either.
fn next_tag(markup: &str, from: usize) -> Option<Tag<'_>> {
    let start = markup[from..].find('<')? + from;
    let end = markup[start..].find('>')? + start + 1;
    let inner = &markup[start + 1..end - 1];
    let closing = inner.starts_with('/');
    let name: String = inner
        .trim_start_matches('/')
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect::<String>()
        .to_ascii_lowercase();
    Some(Tag {
        name,
        closing,
        inner,
        start,
        end,
    })
}

/// Whether a tag marks a page break, and what page it names.
fn page_mark(markup: &str, tag: &Tag) -> Option<(String, usize)> {
    if tag.closing {
        return None;
    }
    let is_mark = tag.name == "page-number"
        || has_class(tag.inner, "star-pagination")
        || has_class(tag.inner, "page-label");
    if !is_mark {
        return None;
    }
    let close = format!("</{}", tag.name);
    let inner_end = markup[tag.end..]
        .find(&close)
        .map_or(markup.len(), |at| tag.end + at);
    let after = markup[inner_end..]
        .find('>')
        .map_or(markup.len(), |at| inner_end + at + 1);
    let label = attribute(tag.inner, "label")
        .or_else(|| attribute(tag.inner, "data-label"))
        .map(str::to_string)
        .unwrap_or_else(|| {
            // Columbia writes the page in the text: `*Page 940`.
            let (text, _) = text_of_markup(&markup[tag.end..inner_end]);
            text.trim()
                .trim_start_matches('*')
                .trim()
                .trim_start_matches("Page")
                .trim()
                .to_string()
        });
    (!label.is_empty()).then_some((label, after))
}

/// Whether a tag opens a footnote, as opposed to marking one in the text.
fn opens_footnote(tag: &Tag) -> bool {
    !tag.closing
        && (tag.name == "footnote" || (tag.name == "div" && has_class(tag.inner, "footnote")))
}

/// A paragraph being read.
struct OpenBlock {
    /// The element that opened it, or `None` for words outside any element.
    name: Option<String>,
    /// How many elements of the same name are open inside it.
    depth: usize,
    /// Where its words start.
    start: usize,
    page: Option<String>,
    seen_words: bool,
    source_id: Option<String>,
    /// The pages marked after its first words.
    pages_inside: Vec<String>,
}

struct Reader<'a> {
    markup: &'a str,
    at: usize,
    page: Option<String>,
    /// The label of each page mark in the body, in order.
    marks: Vec<String>,
    block: Option<OpenBlock>,
    /// Paragraphs, each with its raw markup, its ids and the pages marked
    /// inside it.
    passages: Vec<(PassageKind, &'a str, Vec<String>, Vec<String>)>,
}

impl<'a> Reader<'a> {
    fn new(markup: &'a str) -> Self {
        Self {
            markup,
            at: 0,
            page: None,
            marks: Vec::new(),
            block: None,
            passages: Vec::new(),
        }
    }

    fn run(&mut self) {
        while let Some(tag) = next_tag(self.markup, self.at) {
            self.words(self.at, tag.start);
            if let Some((label, after)) = page_mark(self.markup, &tag) {
                self.page_break(label);
                if self.block.is_none() {
                    // A mark between paragraphs: its `*441` is no paragraph.
                    self.at = after;
                    continue;
                }
                self.at = tag.end;
                continue;
            }
            if opens_footnote(&tag) {
                self.end_block(tag.start);
                self.at = self.footnote(&tag);
                continue;
            }
            self.at = tag.end;
            self.block_tag(&tag);
        }
        self.words(self.at, self.markup.len());
        self.end_block(self.markup.len());
    }

    /// Words between two tags.
    fn words(&mut self, from: usize, to: usize) {
        if self.markup[from..to].trim().is_empty() {
            return;
        }
        match &mut self.block {
            Some(block) => block.seen_words = true,
            None => {
                self.block = Some(OpenBlock {
                    name: None,
                    depth: 0,
                    start: from,
                    page: self.page.clone(),
                    seen_words: true,
                    source_id: None,
                    pages_inside: Vec::new(),
                })
            }
        }
    }

    fn page_break(&mut self, label: String) {
        if let Some(block) = &mut self.block {
            match block.seen_words {
                false => block.page = Some(label.clone()),
                true => block.pages_inside.push(label.clone()),
            }
        }
        self.marks.push(label.clone());
        self.page = Some(label);
    }

    /// A tag that may open or close a paragraph.
    fn block_tag(&mut self, tag: &Tag) {
        let is_block = BLOCKS.contains(&tag.name.as_str());
        if !is_block {
            return;
        }
        if tag.closing {
            if let Some(block) = &mut self.block
                && block.name.as_deref() == Some(tag.name.as_str())
            {
                if block.depth > 0 {
                    block.depth -= 1;
                } else {
                    self.end_block(tag.start);
                }
            }
            return;
        }
        if let Some(block) = &mut self.block {
            match block.name.as_deref() {
                // A paragraph cannot hold another: a new one ends it, as HTML
                // reads it.
                Some("p") | None => self.end_block(tag.start),
                Some(name) if name == tag.name => {
                    block.depth += 1;
                    return;
                }
                Some(_) => return,
            }
        }
        self.block = Some(OpenBlock {
            name: Some(tag.name.clone()),
            depth: 0,
            start: tag.end,
            page: self.page.clone(),
            seen_words: false,
            source_id: attribute(tag.inner, "id").map(str::to_string),
            pages_inside: Vec::new(),
        });
    }

    fn end_block(&mut self, end: usize) {
        let Some(block) = self.block.take() else {
            return;
        };
        let raw = &self.markup[block.start..end];
        if raw.trim().is_empty() {
            return;
        }
        self.passages.push((
            PassageKind::Paragraph { page: block.page },
            raw,
            block.source_id.into_iter().collect(),
            block.pages_inside,
        ));
    }

    /// Read one footnote whole, and return where it ends.
    fn footnote(&mut self, open: &Tag) -> usize {
        let mut depth = 0;
        let mut at = open.end;
        let mut ids = Vec::new();
        let mut end = self.markup.len();
        let mut after = self.markup.len();
        while let Some(tag) = next_tag(self.markup, at) {
            at = tag.end;
            if tag.name == open.name {
                if tag.closing {
                    if depth == 0 {
                        end = tag.start;
                        after = tag.end;
                        break;
                    }
                    depth -= 1;
                } else {
                    depth += 1;
                }
            } else if !tag.closing
                && BLOCKS.contains(&tag.name.as_str())
                && let Some(id) = attribute(tag.inner, "id")
            {
                ids.push(id.to_string());
            }
        }
        let raw = &self.markup[open.end..end];
        let label = attribute(open.inner, "label").map(str::to_string);
        self.passages.push((
            PassageKind::Footnote {
                label: label.unwrap_or_default(),
            },
            raw,
            ids,
            Vec::new(),
        ));
        after
    }

    fn finish(self) -> Structure {
        let mut report = MarkupReport::default();
        let mut passages = Vec::new();
        for (kind, raw, source_ids, pages_inside) in self.passages {
            let (text, one) = text_of_markup(raw);
            if text.is_empty() {
                // Markup with no words in it, such as `<p></p>`.
                continue;
            }
            for (name, count) in one.unknown_entities {
                *report.unknown_entities.entry(name).or_insert(0) += count;
            }
            let kind = match kind {
                PassageKind::Footnote { label } => footnote_kind(label, &text),
                paragraph => paragraph,
            };
            let text = match &kind {
                PassageKind::Footnote { label } => without_label(&text, label),
                PassageKind::Paragraph { .. } => text,
            };
            passages.push(Passage {
                kind,
                text,
                source_ids,
                footnote_marks: footnote_marks(raw),
                pages_inside,
            });
        }
        let first_page_mark = self.marks.first().cloned();
        if let Some(first_mark) = &first_page_mark {
            first_page(&mut passages, first_mark);
        }
        let pages = pages_printed(&passages, self.marks);
        Structure {
            passages,
            report,
            first_page_mark,
            pages,
        }
    }
}

/// The label of each footnote a passage refers to, in order.
///
/// CourtListener's conversion of Harvard's text writes a reference as
/// `<a class="footnote" href="#fn1">1</a>`, and CAP's own XML writes
/// `<footnotemark>1</footnotemark>`. A writing found by rule inside a combined
/// record takes the footnotes it refers to (`super::writings`).
fn footnote_marks(raw: &str) -> Vec<String> {
    let mut marks = Vec::new();
    let mut at = 0;
    while let Some(tag) = next_tag(raw, at) {
        at = tag.end;
        let is_mark = !tag.closing
            && (tag.name == "footnotemark"
                || (tag.name == "a" && has_class(tag.inner, "footnote")));
        if !is_mark {
            continue;
        }
        let close = format!("</{}", tag.name);
        let inner_end = raw[tag.end..]
            .find(&close)
            .map_or(raw.len(), |end| tag.end + end);
        let (label, _) = text_of_markup(&raw[tag.end..inner_end]);
        if !label.is_empty() {
            marks.push(label);
        }
    }
    marks
}

/// Every page the body is printed on, in order, once each.
///
/// Empty when no paragraph has a page, which is a writing with no page level.
fn pages_printed(passages: &[Passage], marks: Vec<String>) -> Vec<String> {
    let first = passages.iter().find_map(|passage| match &passage.kind {
        PassageKind::Paragraph { page } => Some(page.clone()),
        PassageKind::Footnote { .. } => None,
    });
    let Some(Some(first)) = first else {
        return Vec::new();
    };
    let mut pages = vec![first];
    for mark in marks {
        if !pages.contains(&mark) {
            pages.push(mark);
        }
    }
    pages
}

/// A footnote's label: the one the source states, or else the mark its text
/// opens with, as Columbia prints it.
fn footnote_kind(label: String, text: &str) -> PassageKind {
    let label = match label.is_empty() {
        false => label,
        true => text
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_string(),
    };
    PassageKind::Footnote { label }
}

/// A footnote's words without the mark it opens with, when it opens with one.
fn without_label(text: &str, label: &str) -> String {
    match text.strip_prefix(label) {
        Some(rest) if !label.is_empty() => rest.trim_start().to_string(),
        _ => text.to_string(),
    }
}

/// The page the writing starts on, for the paragraphs before its first page
/// mark: the page before the first one marked.
///
/// A mark stands at each page break, so the words before the first mark are on
/// the page before it. When that page cannot be counted, because the first
/// label is not a number, no paragraph has a page: a path that names a page
/// names one the reporter printed.
fn first_page(passages: &mut [Passage], first_label: &str) {
    let before = first_label
        .parse::<u64>()
        .ok()
        .and_then(|page| page.checked_sub(1))
        .map(|page| page.to_string());
    for passage in passages.iter_mut() {
        if let PassageKind::Paragraph { page } = &mut passage.kind {
            if page.is_some() {
                break;
            }
            *page = before.clone();
        }
    }
    if before.is_none() {
        for passage in passages.iter_mut() {
            if let PassageKind::Paragraph { page } = &mut passage.kind {
                *page = None;
            }
        }
    }
}
