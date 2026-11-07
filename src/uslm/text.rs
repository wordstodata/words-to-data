//! A Code provision as the law reads: its number, its heading, its words and
//! its children, in the order USLM prints them (#385).
//!
//! `search "" --path <p> --snippet 0` gave one hit for each field, with no
//! numbers, and it put the continuation of a subsection before its paragraphs.
//! A [`TextNode`] keeps the tree, so a reader gets the provision in document
//! order: number and heading, chapeau, content, the children, proviso,
//! continuation.
//!
//! A whole title is many thousand lines, so an answer stops at a limit of
//! lines. The limit cuts the tree, not only the printed text, so the text and
//! the JSON of one answer hold the same nodes.

use serde::Serialize;

use crate::dataset::{DatasetError, ExpressionId};
use crate::document::{DocumentNode, TextContentField};
use crate::storage::DocumentReader;
use crate::uslm::UslmFacts;
use crate::uslm::path::covers_path;

/// How many lines an answer prints, by default. A section of the Code fits.
pub const DEFAULT_LINE_LIMIT: usize = 400;

/// One provision and the provisions below it, with the words of each.
///
/// An empty field is `None`, and `--json` leaves it out. The same rule holds
/// for every field, `children` too.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TextNode {
    pub path: String,
    /// The number as the publisher prints it, such as `(a)` or `§ 6041.`.
    /// It is read from the stored node, never made from the path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heading: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chapeau: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proviso: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub continuation: Option<String>,
    /// The publisher's footnotes, one on each line (ADR 0015).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub footnotes: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<TextNode>,
}

impl TextNode {
    /// The words of `node` and of the nodes below it, `depth` levels down.
    ///
    /// With no depth, every level. A node at the last level keeps only its
    /// number and heading, so `depth` 1 on a chapter lists what it holds.
    pub fn of(node: &DocumentNode, depth: Option<usize>) -> Self {
        let field = |field: TextContentField| {
            node.data
                .get_text_content(field)
                .and_then(|text| present(&text))
        };
        let head = TextNode {
            path: node.data.path.to_string(),
            num: UslmFacts::of(&node.data).and_then(|facts| present(&facts.number_display)),
            heading: field(TextContentField::Heading),
            chapeau: None,
            content: None,
            proviso: None,
            continuation: None,
            footnotes: None,
            children: vec![],
        };
        if depth == Some(0) {
            return head;
        }
        TextNode {
            chapeau: field(TextContentField::Chapeau),
            content: field(TextContentField::Content),
            proviso: field(TextContentField::Proviso),
            continuation: field(TextContentField::Continuation),
            footnotes: field(TextContentField::Footnotes),
            children: node
                .children
                .iter()
                .map(|child| TextNode::of(child, depth.map(|levels| levels - 1)))
                .collect(),
            ..head
        }
    }

    /// How many nodes this tree holds, this one too.
    pub fn count(&self) -> usize {
        1 + self.children.iter().map(TextNode::count).sum::<usize>()
    }

    /// The tree as the text answer prints it, one line to each item, each
    /// level indented two spaces more than the level above.
    pub fn lines(&self) -> Vec<String> {
        let mut lines = vec![];
        self.write_lines(0, &mut lines);
        lines
    }

    fn write_lines(&self, level: usize, lines: &mut Vec<String>) {
        let indent = "  ".repeat(level);
        lines.extend(self.leading_lines().map(|line| format!("{indent}{line}")));
        for child in &self.children {
            child.write_lines(level + 1, lines);
        }
        lines.extend(self.trailing_lines().map(|line| format!("{indent}{line}")));
    }

    /// The lines before the children: the number and heading on one line,
    /// then chapeau, content and the footnotes. A footnote is about this
    /// node's words, so it comes before the provisions below it.
    fn leading_lines(&self) -> impl Iterator<Item = String> + '_ {
        let head = match (&self.num, &self.heading) {
            // The publisher prints "CHAPTER 79—DEFINITIONS" with no space
            // after the dash, and "§ 6041. Information at source" with one.
            (Some(num), Some(heading)) if num.ends_with('—') => Some(format!("{num}{heading}")),
            (Some(num), Some(heading)) => Some(format!("{num} {heading}")),
            (Some(one), None) | (None, Some(one)) => Some(one.clone()),
            (None, None) => None,
        };
        let words = [&self.chapeau, &self.content]
            .into_iter()
            .flatten()
            .map(|words| format!("  {}", one_line(words)));
        let footnotes = self
            .footnotes
            .iter()
            .flat_map(|notes| notes.lines())
            .map(|note| format!("  [footnote] {}", one_line(note)));
        head.into_iter().chain(words).chain(footnotes)
    }

    /// The lines after the children: proviso, then continuation.
    fn trailing_lines(&self) -> impl Iterator<Item = String> + '_ {
        [&self.proviso, &self.continuation]
            .into_iter()
            .flatten()
            .map(|words| format!("  {}", one_line(words)))
    }
}

/// A provision's words at one date, cut to a limit of lines.
///
/// `nodes_total` and `truncated` come before `node`, so a reader that cuts
/// the JSON still sees them (#447).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProvisionText {
    /// The expression read, as `work@date`.
    #[serde(serialize_with = "as_text")]
    pub expression: ExpressionId,
    pub path: String,
    pub at: String,
    /// How many nodes the answer is about, to the depth asked for.
    pub nodes_total: usize,
    /// Whether the limit left out a node or a part of one.
    pub truncated: bool,
    /// How many nodes the limit did not let show whole.
    #[serde(skip)]
    pub nodes_not_shown: usize,
    pub node: TextNode,
}

/// The expression that holds `path` on the date `at`, and the provision there
/// with its words, `depth` levels down and cut at `line_limit` lines.
///
/// `None` when no expression dated `at` holds a provision at `path`. The work
/// is the held work whose path covers `path`, read from the dataset and not
/// made from the path, as `judicial::reliance` reads it.
pub fn provision_text<R: DocumentReader + ?Sized>(
    reader: &R,
    path: &str,
    at: &str,
    depth: Option<usize>,
    line_limit: usize,
) -> Result<Option<ProvisionText>, DatasetError> {
    let Some(work) = reader
        .works()?
        .into_iter()
        .filter(|work| covers_path(work.as_str(), path))
        .max_by_key(|work| work.as_str().len())
    else {
        return Ok(None);
    };
    let expression = ExpressionId::new(work, at);
    let Some(document) = reader.get_expression(&expression)? else {
        return Ok(None);
    };
    let Some(found) = document.root.find(path) else {
        return Ok(None);
    };

    let whole = TextNode::of(found, depth);
    let nodes_total = whole.count();
    let mut cut = Cut {
        lines_left: line_limit,
        kept_any: false,
        whole_nodes: 0,
        stopped: false,
    };
    let node = cut
        .keep(whole)
        .expect("the cut always keeps the first node");
    let nodes_not_shown = nodes_total - cut.whole_nodes;
    Ok(Some(ProvisionText {
        expression,
        path: path.to_string(),
        at: at.to_string(),
        nodes_total,
        truncated: nodes_not_shown > 0,
        nodes_not_shown,
        node,
    }))
}

/// A walk in document order that keeps lines while the limit has room.
///
/// The cut is made where the text stops: a node whose first lines do not fit
/// is left out with every node after it, and a node whose proviso or
/// continuation does not fit loses them, because they come after its children.
struct Cut {
    lines_left: usize,
    /// Whether a node was kept. The first node is kept even when its own lines
    /// are more than the limit, so an answer always names the provision.
    kept_any: bool,
    /// The nodes kept with all their lines.
    whole_nodes: usize,
    stopped: bool,
}

impl Cut {
    fn keep(&mut self, mut node: TextNode) -> Option<TextNode> {
        let leading = node.leading_lines().count();
        if self.stopped || (self.kept_any && leading > self.lines_left) {
            self.stopped = true;
            return None;
        }
        self.kept_any = true;
        self.lines_left = self.lines_left.saturating_sub(leading);

        let children = std::mem::take(&mut node.children);
        node.children = children
            .into_iter()
            .map_while(|child| self.keep(child))
            .collect();

        let trailing = node.trailing_lines().count();
        if !self.stopped && trailing <= self.lines_left {
            self.lines_left -= trailing;
            self.whole_nodes += 1;
        } else {
            self.stopped = true;
            node.proviso = None;
            node.continuation = None;
        }
        Some(node)
    }
}

/// An expression id as one string, `work@date`, as the commands print it.
fn as_text<S: serde::Serializer>(id: &ExpressionId, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.collect_str(id)
}

/// `text` without the white space at its ends, or `None` when nothing is left.
fn present(text: &str) -> Option<String> {
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

/// The words on one line: each run of white space is one space.
fn one_line(words: &str) -> String {
    words.split_whitespace().collect::<Vec<_>>().join(" ")
}
