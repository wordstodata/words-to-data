//! Finding the writings inside a record that holds a whole case, by rule.
//!
//! CourtListener often holds a case as one combined record: the Court's
//! opinion, every concurrence and every dissent in one text. When the case has
//! no split records, a rule may find where each writing starts
//! (`docs/adr/0014-a-case-is-a-work-and-a-citation-runs-paragraph-to-subdivision.md`,
//! decision 2). Two rules are here:
//!
//! - [`JUSTICE_HEADINGS`] reads the line a reporter prints at the head of each
//!   writing: "JUSTICE THOMAS, with whom JUSTICE SCALIA joins, dissenting."
//! - [`RUNNING_HEADS`] reads the running head a Supreme Court slip opinion
//!   prints on each page: "Opinion of the Court", "JACKSON, J., dissenting".
//!   The running heads are found by `super::plain`.
//!
//! A rule is a machine's reading of the text, so every writing it finds is
//! `MachineSuggested`, and its provenance names the rule. A rule that finds
//! fewer than two writings splits nothing.

use std::sync::LazyLock;

use regex::Regex;

use super::plain::Section;
use super::structure::{Passage, PassageKind};
use crate::method::Method;

/// The rule that splits a combined record at the line that opens each writing.
pub(crate) const JUSTICE_HEADINGS: &str = "justice-headings";

/// The rule that splits a Supreme Court slip opinion where its running head
/// changes.
pub(crate) const RUNNING_HEADS: &str = "running-heads";

/// The version of both rules. Raise it when the writings a rule finds change
/// (`crate::method::Method`).
pub(crate) const VERSION: u32 = 1;

/// A rule, as a provenance names it: `justice-headings@1`.
pub(crate) fn rule(name: &str) -> Method {
    Method::new(name, VERSION)
}

/// One writing a rule found.
#[derive(Debug, Clone)]
pub(crate) struct FoundWriting {
    /// The words the rule found the writing by: the line that opens it, or its
    /// running head.
    pub heading: String,
    /// The kind of writing, in CourtListener's vocabulary, such as `040dissent`.
    pub opinion_type: Option<&'static str>,
    /// The Justice who wrote it, as CourtListener names an author: `Thomas`.
    pub author: Option<String>,
    pub per_curiam: Option<bool>,
    /// Its paragraphs and footnotes, in order.
    pub passages: Vec<Passage>,
}

/// The line that opens a writing, as the reporters print it:
///
/// - "JUSTICE STEVENS delivered the opinion of the Court."
/// - "MR. JUSTICE DOUGLAS delivered the opinion of the Court."
/// - "Chief Justice ROBERTS, with whom Justice SCALIA and Justice THOMAS join,
///   dissenting."
/// - "JUSTICE KENNEDY, concurring in part and concurring in the judgment."
///
/// The whole paragraph must be the line. A citation that names a dissent,
/// "(Harlan, J., dissenting)", opens with no "Justice" and so never matches.
static HEADING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)^
        (?:MR\.\s+|Mr\.\s+)?
        (?:CHIEF\s+JUSTICE|Chief\s+Justice|JUSTICE|Justice)\s+
        (?P<name>[A-Z][A-Za-z'’\-]+)
        (?P<rest>,?\s.{0,240}?
            (?:delivered\s+the\s+opinion\s+of\s+the\s+Court
              |announced\s+the\s+judgment\s+of\s+the\s+Court
              |dissenting
              |concurring)
        [^.]{0,120})
        \.?\s*$",
    )
    .expect("the heading pattern is valid")
});

/// Whether a line is the whole of the line that opens a writing.
pub(crate) fn opens_writing(line: &str) -> bool {
    let line = line.split_whitespace().collect::<Vec<_>>().join(" ");
    HEADING.is_match(&line.replace('-', ""))
}

/// Read one heading line, when the passage is one.
fn heading(passage: &Passage) -> Option<FoundWriting> {
    if !matches!(passage.kind, PassageKind::Paragraph { .. }) {
        return None;
    }
    let text = passage
        .text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    // A line of a slip opinion broken at a hyphen keeps it: "dis-senting".
    let unbroken = text.replace('-', "");
    let found = HEADING.captures(&unbroken)?;
    let rest = &found["rest"];
    Some(FoundWriting {
        opinion_type: Some(opinion_type(rest)),
        author: Some(author_name(&found["name"])),
        per_curiam: Some(false),
        heading: text.clone(),
        passages: Vec::new(),
    })
}

/// The kind of writing a heading or a running head says it opens, in
/// CourtListener's vocabulary.
pub(crate) fn opinion_type(said: &str) -> &'static str {
    let dissents = said.contains("dissenting");
    let concurs = said.contains("concurring");
    if said.contains("announced the judgment") {
        "025plurality"
    } else if dissents && concurs {
        "035concurrenceinpart"
    } else if dissents {
        "040dissent"
    } else if concurs {
        "030concurrence"
    } else {
        "020lead"
    }
}

/// A surname as CourtListener writes an author: `ROBERTS` is `Roberts`, and
/// `O'CONNOR` is `O'Connor`.
pub(crate) fn author_name(printed: &str) -> String {
    let mut name = String::new();
    let mut capital = true;
    for c in printed.chars() {
        if capital {
            name.extend(c.to_uppercase());
        } else {
            name.extend(c.to_lowercase());
        }
        capital = matches!(c, '\'' | '’' | '-');
    }
    name
}

/// Split a combined record's passages at the line that opens each writing.
///
/// `None` when the rule finds fewer than two writings: the record then stays
/// one writing. Words before the first heading, such as the caption and the
/// names of counsel a reporter prints, stay with the first writing, so no word
/// is lost. Each footnote goes to the writing that refers to it
/// ([`give_footnotes`]).
pub(crate) fn split_at_justice_headings(passages: &[Passage]) -> Option<Vec<FoundWriting>> {
    let mut writings: Vec<FoundWriting> = Vec::new();
    let mut before_first = Vec::new();
    let mut footnotes = Vec::new();
    for passage in passages {
        if matches!(passage.kind, PassageKind::Footnote { .. }) {
            footnotes.push(passage.clone());
            continue;
        }
        if let Some(mut opened) = heading(passage) {
            opened.passages.push(passage.clone());
            writings.push(opened);
            continue;
        }
        match writings.last_mut() {
            Some(writing) => writing.passages.push(passage.clone()),
            None => before_first.push(passage.clone()),
        }
    }
    if writings.len() < 2 {
        return None;
    }
    before_first.append(&mut writings[0].passages);
    writings[0].passages = before_first;
    give_footnotes(&mut writings, footnotes);
    Some(writings)
}

/// Give each footnote to the writing that refers to it.
///
/// The writings of one case number their footnotes each from 1, and a
/// combined text prints them in the order of the writings. So each footnote
/// goes to the first writing, at or after the one the previous footnote went
/// to, that refers to its label and has not had that label yet. A footnote no
/// writing refers to goes with the previous footnote, or to the last writing,
/// where the combined text printed it.
fn give_footnotes(writings: &mut [FoundWriting], footnotes: Vec<Passage>) {
    let mut taken: Vec<Vec<String>> = vec![Vec::new(); writings.len()];
    let last = writings.len() - 1;
    let mut previous: Option<usize> = None;
    for footnote in footnotes {
        let PassageKind::Footnote { label } = &footnote.kind else {
            continue;
        };
        let refers = |writing: &FoundWriting| {
            writing
                .passages
                .iter()
                .any(|passage| passage.footnote_marks.contains(label))
        };
        let owner = (previous.unwrap_or(0)..writings.len())
            .find(|&index| refers(&writings[index]) && !taken[index].contains(label))
            .unwrap_or(previous.unwrap_or(last));
        taken[owner].push(label.clone());
        previous = Some(owner);
        writings[owner].passages.push(footnote);
    }
}

/// Matter a rule found that is no part of any writing, such as the syllabus a
/// slip opinion opens with.
#[derive(Debug, Clone)]
pub(crate) struct FoundMatter {
    /// The cluster field the same matter has when the publisher fills it, and
    /// so the node's name: `syllabus`.
    pub field: &'static str,
    /// The words the rule found it by: its running head.
    pub heading: String,
    pub passages: Vec<Passage>,
}

/// What the running heads of a slip opinion say: the writings in it, in order,
/// and the matter in it that is no part of any writing.
#[derive(Debug, Clone, Default)]
pub(crate) struct RunningHeads {
    pub writings: Vec<FoundWriting>,
    pub matter: Vec<FoundMatter>,
}

/// Read each section of a slip opinion as a writing, or as the syllabus.
///
/// The syllabus says it "constitutes no part of the opinion of the Court", so
/// it is matter of the case and no writing. The other running heads name the
/// writing: "Per Curiam", "Opinion of the Court", "Opinion of ALITO, J." (a
/// plurality), "THOMAS, J., dissenting". The running head over the Court's
/// opinion names no author, so the author is read from the line that opens it,
/// "JUSTICE KENNEDY delivered the opinion of the Court." "Opinion of
/// SOTOMAYOR, J." heads a plurality, a concurrence or a dissent, so its kind is
/// read from that line too.
pub(crate) fn read_running_heads(sections: Vec<Section>) -> RunningHeads {
    let mut found = RunningHeads::default();
    for section in sections {
        let head = section.running_head.unwrap_or_default();
        if head == "Syllabus" {
            found.matter.push(FoundMatter {
                field: "syllabus",
                heading: head,
                passages: section.passages,
            });
            continue;
        }
        let mut writing = writing_of_head(&head.replace('-', ""));
        // The line that opens the writing follows the caption printed on its
        // first page.
        let opening = section
            .passages
            .iter()
            .take(OPENING_WITHIN)
            .find_map(heading);
        if let Some(opening) = opening {
            if writing.author.is_none() {
                writing.author = opening.author;
            }
            // "Opinion of SOTOMAYOR, J." heads a plurality, a concurrence or
            // a dissent alike; the opening line says which.
            if head.starts_with("Opinion of") && !head.ends_with("of the Court") {
                writing.opinion_type = opening.opinion_type;
            }
        }
        writing.passages = section.passages;
        found.writings.push(writing);
    }
    found
}

/// How many passages into a writing its opening line may stand: after the
/// caption, the docket numbers and the parties of every case decided with it.
const OPENING_WITHIN: usize = 40;

/// "SCALIA, J., dissenting", "ROBERTS, C. J., dissenting".
static JUSTICE_HEAD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?P<name>[A-Z][A-Z'’\-]+),\s+(?:C\.\s+)?J\.,\s+(?P<rest>.+)$")
        .expect("the running head pattern is valid")
});

/// "Opinion of ALITO, J.", "Statement of SOTOMAYOR, J."
static OF_JUSTICE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?P<kind>Opinion|Statement)\s+of\s+(?P<name>[A-Z][A-Z'’\-]+),\s+(?:C\.\s+)?J\.$")
        .expect("the running head pattern is valid")
});

/// The writing a running head names, with no passages yet.
fn writing_of_head(head: &str) -> FoundWriting {
    let mut writing = FoundWriting {
        heading: head.to_string(),
        opinion_type: None,
        author: None,
        per_curiam: None,
        passages: Vec::new(),
    };
    if head == "Per Curiam" {
        writing.opinion_type = Some("020lead");
        writing.per_curiam = Some(true);
    } else if head.ends_with("of the Court") {
        writing.opinion_type = Some("020lead");
        writing.per_curiam = Some(false);
    } else if let Some(found) = OF_JUSTICE.captures(head) {
        writing.author = Some(author_name(&found["name"]));
        if &found["kind"] == "Opinion" {
            writing.opinion_type = Some("025plurality");
        }
    } else if let Some(found) = JUSTICE_HEAD.captures(head) {
        writing.author = Some(author_name(&found["name"]));
        writing.opinion_type = Some(opinion_type(&found["rest"]));
        writing.per_curiam = Some(false);
    }
    writing
}
