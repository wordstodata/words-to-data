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
//! - [`JUDGE_HEADINGS`] reads the words that open each writing of a Tax Court
//!   opinion: "TORO, Judge:" and "JENKINS, J., dissenting:".
//!
//! One more rule finds matter that is no part of any writing:
//! [`STAFF_SUMMARY`] takes out the summary that court staff print before a
//! Ninth Circuit opinion ([`take_staff_summary`]).
//!
//! A rule is a machine's reading of the text, so every writing it finds is
//! `MachineSuggested`, and its provenance names the rule. A rule that finds
//! fewer than two writings splits nothing.

use std::sync::LazyLock;

use regex::Regex;

use super::plain::{PARAGRAPHS, REPORTERS_NOTE, Section};
use super::structure::{Passage, PassageKind};
use crate::method::Method;

/// The rule that splits a combined record at the line that opens each writing.
pub(crate) const JUSTICE_HEADINGS: &str = "justice-headings";

/// The rule that splits a Supreme Court slip opinion where its running head
/// changes.
pub(crate) const RUNNING_HEADS: &str = "running-heads";

/// The rule that splits a Tax Court opinion at the words that open each
/// writing.
pub(crate) const JUDGE_HEADINGS: &str = "judge-headings";

/// The rule that takes a court staff summary out of the writing it opens.
pub(crate) const STAFF_SUMMARY: &str = "staff-summary";

/// A rule, as a provenance names it: `justice-headings@1`.
///
/// Each rule has its own version. Raise it when what the rule finds changes
/// (`crate::method::Method`):
///
/// - `running-heads@2` (#315): a running head names its writing in mixed case
///   too, has only words in lower case after "dissenting" or "concurring",
///   and a writing starts at its opening line when that line is not the first
///   of its page. The cover and the Reporter's Note of a preliminary print are
///   no writing.
/// - `plain-text-paragraphs@2` (#315): a rule of em dashes is a footnote rule
///   only at the left margin.
/// - `plain-text-paragraphs@3` (#413): the watermark of a preliminary print is
///   no word of the text, a centred line of stars is a section break and no
///   footnote mark, stars before more stars are an ellipsis, a star against
///   the first word of a footnote is its label, and in a double-spaced text
///   two blank lines go on with a footnote only once a footnote has started.
/// - `plain-text-paragraphs@4` (#430): an indented line that opens with a
///   list designation, "“(i)", after a lead-in that ends with "—" or ":"
///   starts a paragraph.
pub(crate) fn rule(name: &str) -> Method {
    let version = match name {
        PARAGRAPHS => 4,
        RUNNING_HEADS => 2,
        _ => 1,
    };
    Method::new(name, version)
}

/// One writing a rule found.
#[derive(Debug, Clone)]
pub(crate) struct FoundWriting {
    /// What the rule found the writing by, as its provenance says it:
    /// `justice-headings@1: the writing opens with "…"`.
    pub found_by: String,
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
        found_by: opens_with(JUSTICE_HEADINGS, &text),
        passages: Vec::new(),
    })
}

/// What a rule found a writing by, when it found the words that open it.
fn opens_with(name: &str, words: &str) -> String {
    format!("{}: the writing opens with \"{words}\"", rule(name))
}

/// The words that open a separate writing of a Tax Court opinion, at the start
/// of a line: "JENKINS, J., dissenting:", "URDA, C.J., concurring in the
/// result:". The colon is part of them; the words after it are the first
/// sentence of the writing. "JENKINS, J., dissents." and "JENKINS, J., wrote a
/// dissenting opinion." name the writing, and do not open it.
static SEPARATE_WRITING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(?P<name>[A-Z][A-Z'’\-]+(?:\s[A-Z][A-Z'’\-]+)*),\s(?:C\.\s?J|J)\.,\s(?P<rest>(?:concurring|dissenting)(?:,?\s[a-z]+)*):",
    )
    .expect("the heading pattern is valid")
});

/// The words that open the opinion of a Tax Court, at the start of a
/// paragraph: "TORO, Judge:", "CARLUZZO, Chief Special Trial Judge:".
static JUDGE_OPINION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(?P<name>[A-Z][A-Z'’\-]+(?:\s[A-Z][A-Z'’\-]+)*),\s(?:Chief\s)?(?:Special\sTrial\s)?Judge:",
    )
    .expect("the heading pattern is valid")
});

/// The words that open a separate writing of a Tax Court opinion, when a line
/// starts with them ([`SEPARATE_WRITING`]).
pub(crate) fn separate_writing_heading(line: &str) -> Option<String> {
    let line = line.split_whitespace().collect::<Vec<_>>().join(" ");
    SEPARATE_WRITING
        .find(&line)
        .map(|found| found.as_str().to_string())
}

/// The writing a Tax Court heading opens: the heading of a separate writing,
/// or else the first paragraph that opens the opinion of the Court.
fn judge_writing(opened_by: Option<&str>, passages: &[Passage]) -> FoundWriting {
    let mut writing = FoundWriting {
        found_by: format!(
            "{}: the words before the first separate writing",
            rule(JUDGE_HEADINGS)
        ),
        opinion_type: None,
        author: None,
        per_curiam: None,
        passages: Vec::new(),
    };
    let found = match opened_by {
        Some(heading) => SEPARATE_WRITING
            .captures(heading)
            .map(|found| (found, Some(opinion_type(&heading.replace(':', ""))))),
        None => passages
            .iter()
            .filter(|passage| matches!(passage.kind, PassageKind::Paragraph { .. }))
            .find_map(|passage| JUDGE_OPINION.captures(&passage.text))
            .map(|found| (found, Some("020lead"))),
    };
    if let Some((found, kind)) = found {
        writing.found_by = opens_with(JUDGE_HEADINGS, &found[0]);
        writing.author = Some(author_name(&found["name"]));
        writing.opinion_type = kind;
        writing.per_curiam = Some(false);
    }
    writing
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
    /// What the rule found it by, as its provenance says it:
    /// `running-heads@1: the pages carry the running head "Syllabus"`.
    pub found_by: String,
    pub passages: Vec<Passage>,
}

/// What the running heads of a slip opinion say: the writings in it, in order,
/// and the matter in it that is no part of any writing.
#[derive(Debug, Clone, Default)]
pub(crate) struct RunningHeads {
    pub writings: Vec<FoundWriting>,
    pub matter: Vec<FoundMatter>,
}

/// Read each section of a `plain_text` record as a writing, or as the
/// syllabus.
///
/// The syllabus says it "constitutes no part of the opinion of the Court", so
/// it is matter of the case and no writing. The other running heads name the
/// writing: "Per Curiam", "Opinion of the Court", "Opinion of ALITO, J." (a
/// plurality), "THOMAS, J., dissenting". The running head over the Court's
/// opinion names no author, so the author is read from the line that opens it,
/// "JUSTICE KENNEDY delivered the opinion of the Court." "Opinion of
/// SOTOMAYOR, J." heads a plurality, a concurrence or a dissent, so its kind is
/// read from that line too.
///
/// A Tax Court opinion has no running heads. When a section starts at the
/// words that open a separate writing, "JENKINS, J., dissenting:", each
/// section is a writing found by [`JUDGE_HEADINGS`] ([`judge_writing`]).
pub(crate) fn read_running_heads(sections: Vec<Section>) -> RunningHeads {
    let mut found = RunningHeads::default();
    let judged = sections.iter().any(|section| section.opened_by.is_some());
    let headed = sections
        .iter()
        .any(|section| section.running_head.is_some());
    for (index, section) in sections.into_iter().enumerate() {
        if index == 0 && headed && is_cover(&section) {
            found.matter.push(FoundMatter {
                field: "headmatter",
                found_by: format!(
                    "{}: the first page carries no running head and says \"PRELIMINARY PRINT\"",
                    rule(RUNNING_HEADS)
                ),
                passages: section.passages,
            });
            continue;
        }
        if judged && section.running_head.is_none() {
            let mut writing = judge_writing(section.opened_by.as_deref(), &section.passages);
            writing.passages = section.passages;
            found.writings.push(writing);
            continue;
        }
        let head = section.running_head.unwrap_or_default();
        if head == REPORTERS_NOTE {
            found.matter.push(FoundMatter {
                field: "reporters_note",
                found_by: format!("{}: the page is headed \"{head}\"", rule(RUNNING_HEADS)),
                passages: section.passages,
            });
            continue;
        }
        if head == "Syllabus" {
            found.matter.push(FoundMatter {
                field: "syllabus",
                found_by: format!(
                    "{}: the pages carry the running head \"{head}\"",
                    rule(RUNNING_HEADS)
                ),
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

/// Whether a section is the cover of a preliminary print of the United States
/// Reports: it carries no running head, and it says "PRELIMINARY PRINT". The
/// running heads of the writings start on the pages after it.
fn is_cover(section: &Section) -> bool {
    section.running_head.is_none()
        && section.opened_by.is_none()
        && section
            .passages
            .iter()
            .any(|passage| passage.text.trim() == "PRELIMINARY PRINT")
}

/// How many passages into a writing its opening line may stand: after the
/// caption, the docket numbers and the parties of every case decided with it.
const OPENING_WITHIN: usize = 40;

/// "SCALIA, J., dissenting", "ROBERTS, C. J., dissenting", and in a
/// preliminary print "Jackson, J., concurring".
static JUSTICE_HEAD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?P<name>[A-Z][A-Za-z'’\-]+),\s+(?:C\.\s+)?J\.,\s+(?P<rest>.+)$")
        .expect("the running head pattern is valid")
});

/// "Opinion of ALITO, J.", "Statement of SOTOMAYOR, J.", "Opinion of Gorsuch,
/// J."
static OF_JUSTICE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(?P<kind>Opinion|Statement)\s+of\s+(?P<name>[A-Z][A-Za-z'’\-]+),\s+(?:C\.\s+)?J\.$",
    )
    .expect("the running head pattern is valid")
});

/// The writing a running head names, with no passages yet.
fn writing_of_head(head: &str) -> FoundWriting {
    let mut writing = FoundWriting {
        found_by: format!(
            "{}: the writing's pages carry the running head \"{head}\"",
            rule(RUNNING_HEADS)
        ),
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

/// The words with which court staff say a summary is theirs: "This summary
/// constitutes no part of the opinion of the court."
static DISCLAIMER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)this\s+summary\s+constitutes\s+no\s+part\s+of\s+the\s+opinion\s+of\s+the\s+court\.?",
    )
    .expect("the disclaimer pattern is valid")
});

/// The heading a staff summary opens with, and the star of the footnote that
/// holds the disclaimer: "SUMMARY *".
static SUMMARY_HEADING: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^SUMMARY\s*\**$").expect("the heading pattern is valid"));

/// A heading in capitals on a line of its own, such as "COUNSEL", "OPINION"
/// or "ORDER": no digit, and no letter in lower case.
fn capital_heading(text: &str) -> bool {
    text.chars().filter(|c| c.is_alphabetic()).count() >= 2
        && text.chars().all(|c| {
            c.is_uppercase() || matches!(c, ' ' | ',' | '.' | '&' | '\'' | '’' | '-' | '*')
        })
}

/// Take a court staff summary out of the passages of a writing.
///
/// The Ninth Circuit prints a summary after the caption of a published
/// opinion, under the heading "SUMMARY", and its footnote says: "This summary
/// constitutes no part of the opinion of the court. It has been prepared by
/// court staff for the convenience of the reader." So the summary is matter of
/// the case and no writing
/// (`docs/adr/0014-a-case-is-a-work-and-a-citation-runs-paragraph-to-subdivision.md`,
/// decision 1). It runs from its heading to the next heading in capitals, such
/// as "COUNSEL" or "OPINION", and the footnote that says it is no part of the
/// opinion goes with it.
///
/// `None`, and the passages do not change, unless the passages hold the
/// disclaimer, the heading and a heading after it. A rule that cannot find
/// where the summary ends takes nothing out.
pub(crate) fn take_staff_summary(passages: &mut Vec<Passage>) -> Option<FoundMatter> {
    let words = |passage: &Passage| {
        passage
            .text
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    };
    let paragraph = |passage: &Passage| matches!(passage.kind, PassageKind::Paragraph { .. });
    let (disclaimer, said) = passages.iter().enumerate().find_map(|(index, passage)| {
        DISCLAIMER
            .find(&words(passage))
            .map(|found| (index, found.as_str().to_string()))
    })?;
    let start = passages
        .iter()
        .position(|passage| paragraph(passage) && SUMMARY_HEADING.is_match(&words(passage)))?;
    let end = start
        + 1
        + passages[start + 1..]
            .iter()
            .position(|passage| paragraph(passage) && capital_heading(&words(passage)))?;
    let inside = (start..end).contains(&disclaimer);
    // The footnote that holds the disclaimer is printed with the other
    // footnotes, after the paragraphs.
    let footnote = !inside && matches!(passages[disclaimer].kind, PassageKind::Footnote { .. });
    if !inside && !footnote {
        return None;
    }

    let mut summary: Vec<Passage> = passages.drain(start..end).collect();
    if footnote {
        let index = if disclaimer >= end {
            disclaimer - (end - start)
        } else {
            disclaimer
        };
        summary.push(passages.remove(index));
    }
    Some(FoundMatter {
        field: "summary",
        found_by: format!("{}: the summary says \"{said}\"", rule(STAFF_SUMMARY)),
        passages: summary,
    })
}
