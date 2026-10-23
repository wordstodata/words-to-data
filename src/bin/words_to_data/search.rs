//! `words_to_data search` — full-text search, scoped.
//!
//! Unscoped and unbounded, one word over the real corpus answered with 1135
//! lines and 224 KB: the same provision once per release point, and one hit
//! carrying 10,228 characters of a section's text (#235). So the way to read the
//! answer was to grep the output, which is the command handing its job to a
//! shell.
//!
//! The filters are not this command's own. `--work`, `--at` and `--path` build
//! the `Locator` #234 settled, so `search` and `annotations` mean the same thing
//! by a work, a window and a path.

use std::io::Write;

use clap::Args as ClapArgs;
use serde::Serialize;
use words_to_data::dataset::{ExpressionId, SearchAnswer, SearchResult};
use words_to_data::inspect::{AbsentPath, AbsentScope, FEW_WORKS, absent_path, absent_scope};
use words_to_data::query::{DEFAULT_LIMIT, Locator};

use crate::annotations::{absent_notice, path_matching};
use crate::load::{self, with_dataset};

/// How many characters of a hit to show around the match, by default.
///
/// A field is not a snippet. One hit in the measured run carried 10,228
/// characters — a whole section's text — and a window is what makes the answer
/// readable: wide enough to read the sentence the term sits in, short enough
/// that a hit is one or two lines of a terminal.
const DEFAULT_SNIPPET: usize = 160;

#[derive(ClapArgs)]
pub struct Args {
    /// Dataset file (`.json` compact or `.sqlite`)
    pub dataset: String,

    /// Text to search for
    ///
    /// Case is ignored. A run of white space in the text, such as a line
    /// break, matches one space in the query. "U.S.C." and "U. S. C." match
    /// each other, and so do "U.S." and "U. S.", and "§7" and "§ 7". A soft
    /// hyphen or a zero-width character is not read, a ligature such as "ﬁ"
    /// matches its letters, and a hyphen between two letters is not read, so
    /// "substantially" matches "substan-tially". No other spellings are made
    /// equal ("U.S.C.A." stays as it is). Snippets show the text as stored
    pub query: String,

    /// Search only this work, e.g. `uscode/title_26`
    ///
    /// Combines with `--at` and `--path`: every filter given is applied
    #[arg(long)]
    pub work: Option<String>,

    /// Search only the expressions published on this date, e.g. `2025-07-18`
    ///
    /// A three-release-point corpus returns one provision three times without it
    ///
    /// A date alone: `uscode/title_26@2025-07-18` is refused, not read as a date
    /// that no expression has (#305). Name the work with `--work`
    #[arg(long, value_name = "DATE", value_parser = crate::span::publication_date)]
    pub at: Option<String>,

    /// Search only this structural path and the paths beneath it
    #[arg(long)]
    pub path: Option<String>,

    /// Match `--path` exactly: search that provision alone, not its subtree
    #[arg(long, requires = "path")]
    pub exact: bool,

    /// How many hits to print before stopping. The run says how many it did not show
    ///
    /// Human output stops at 20 unless told otherwise. `--json` carries them all,
    /// as `annotations` does, unless this is given explicitly
    #[arg(long)]
    pub limit: Option<usize>,

    /// Print every hit, however many there are
    #[arg(long, conflicts_with = "limit")]
    pub all: bool,

    /// How many characters of each hit to show around the match. `0` shows the whole field
    #[arg(long, value_name = "CHARS", default_value_t = DEFAULT_SNIPPET)]
    pub snippet: usize,

    /// List each occurrence of the query as its own hit, with its character offset
    ///
    /// A field can hold the query many times: a court opinion is one field.
    /// Each occurrence gets its own snippet, and `--limit`/`--all` count
    /// occurrences, not fields
    #[arg(long)]
    pub occurrences: bool,

    /// Centre each snippet on this character of its field, not on the first occurrence
    ///
    /// Reads one window of a long field: take an offset from `--occurrences`,
    /// then `--path P --exact --offset N --snippet K` gives the K characters
    /// around it. An empty query matches every field at the path
    #[arg(long, value_name = "CHAR", conflicts_with = "occurrences")]
    pub offset: Option<usize>,

    /// Emit JSON instead of human-readable text
    #[arg(long)]
    pub json: bool,
}

/// One occurrence of the query inside a field, as `--occurrences` lists it.
#[derive(Serialize)]
struct Occurrence {
    expression: ExpressionId,
    path: String,
    field: String,
    /// Where the occurrence begins in the field's text, in characters.
    offset: usize,
    snippet: String,
}

pub fn run(args: Args) {
    let mut locator = Locator::new();
    if let Some(work) = &args.work {
        locator = locator.in_work(work);
    }
    if let Some(at) = &args.at {
        // One date is the window that begins and ends there. A locator holds a
        // window because a link's subject spans two dates; an expression is one
        // work on one date, so the two ends are the same.
        locator = locator.in_window(at, at);
    }
    if let Some(path) = &args.path {
        locator = locator.at_path(path, path_matching(args.exact));
    }

    // A screenful for a person, everything for a machine. An explicit `--limit`
    // is honoured either way, and `--all` lifts it. The same rule `annotations`
    // follows, from the same constant, so two commands cannot come to disagree
    // about what a screenful is.
    let limit = match (args.all, args.limit, args.json) {
        (true, _, _) => None,
        (_, Some(explicit), _) => Some(explicit),
        (_, None, true) => None,
        (_, None, false) => Some(DEFAULT_LIMIT),
    };

    let ds = crate::fail::or_exit(load::open(&args.dataset), "Error opening dataset");
    let found = crate::fail::or_exit(
        with_dataset!(&ds, d => d.search_text_in(&args.query, &locator, limit)),
        "Error searching dataset",
    );
    // Asked only when nothing matched: a hit proves the work, the date and the
    // path are held. An empty search over a guessed path must not read like one
    // over a real path (#305), nor one over a work or date not held (#337).
    let absent = match found.total {
        0 => Absent::find(&ds, &locator, &args.dataset),
        _ => Absent::default(),
    };

    if args.occurrences {
        // The limit went to the search as a count of fields. Each field holds
        // at least one occurrence, so that many fields hold at least that many
        // occurrences, and cutting the list here gives the first ones exactly.
        let mut listed = each_occurrence(&found.rows, args.snippet);
        if let Some(limit) = limit {
            listed.truncate(limit);
        }
        if let Err(error) = report_occurrences(&args, &listed, found.occurrences, &absent)
            && error.kind() != std::io::ErrorKind::BrokenPipe
        {
            crate::fail::refuse(&format!("Error writing the answer: {error}"));
        }
        return;
    }

    // The window is applied to both surfaces. The field is called `snippet`, and
    // a snippet that is a whole section is not one. The field as recorded is a
    // `--snippet 0` away, and `words_to_data path` reads the provision itself.
    let hits: Vec<_> = found
        .rows
        .iter()
        .map(|hit| SearchResult {
            snippet: window_at(
                &hit.snippet,
                args.offset.unwrap_or_else(|| first_offset(hit)),
                args.snippet,
            ),
            ..hit.clone()
        })
        .collect();

    // A reader that closed the pipe has read enough, which is a clean stop and
    // not a failure. `println!` panics on a broken pipe, so `search | head`
    // exited 101 where the same run without a pipe exited 0 (#235).
    if let Err(error) = report(&args, &hits, &found, &absent)
        && error.kind() != std::io::ErrorKind::BrokenPipe
    {
        crate::fail::refuse(&format!("Error writing the answer: {error}"));
    }
}

/// Write the answer to stdout, and say what was left out.
///
/// Returns the write error rather than panicking on it, so the caller can tell a
/// reader that walked away from a disk that filled up.
fn report(
    args: &Args,
    hits: &[SearchResult],
    found: &SearchAnswer,
    absent: &Absent,
) -> std::io::Result<()> {
    let mut out = std::io::stdout().lock();

    if args.json {
        let mut answer = serde_json::json!({
            "hits": hits,
            "total": found.total,
            "occurrences": found.occurrences,
        });
        absent.add_to(&mut answer);
        writeln!(out, "{}", serde_json::to_string_pretty(&answer).unwrap())?;
        return out.flush();
    }

    if let Some(notice) = absent.notice(args.exact) {
        writeln!(out, "{notice}")?;
    }

    for hit in hits {
        writeln!(
            out,
            "{}  {}  [{}]  {}",
            hit.expression,
            hit.path,
            hit.field,
            counted(hit.offsets.len(), "occurrence")
        )?;
        writeln!(out, "    {}", hit.snippet)?;
    }
    // Fields and occurrences both, so a count of fields is not read as a count
    // of mentions. A court opinion is one field (#306).
    writeln!(
        out,
        "{}, {}",
        counted(found.total, "field"),
        counted(found.occurrences, "occurrence")
    )?;
    // What was left out, said rather than left to be inferred. A listing that
    // stopped in silence would read as the whole answer (#220, #235).
    let dropped = found.dropped();
    if dropped > 0 {
        writeln!(
            out,
            "  … {dropped} more not shown; pass --all for every one of them"
        )?;
    }
    out.flush()
}

/// What an empty search reached nothing of: a work or a date not held, or else
/// a path not held. Both are empty when the search found something.
#[derive(Default)]
struct Absent {
    scope: Option<AbsentScope>,
    path: Option<AbsentPath>,
    /// The dataset file, for the command a notice names.
    dataset: String,
}

impl Absent {
    /// Check the work and the date first. A path is "not held" only inside a
    /// work and a date that are held: when the date is not held, the path is
    /// not the filter that matched nothing (#337).
    fn find(ds: &load::OpenDataset, locator: &Locator, dataset: &str) -> Self {
        let scope = crate::fail::or_exit(
            with_dataset!(ds, d => absent_scope(d, locator)),
            "Error checking the work and the date",
        );
        let path = match scope {
            Some(_) => None,
            None => crate::fail::or_exit(
                with_dataset!(ds, d => absent_path(d, locator)),
                "Error checking the path",
            ),
        };
        Self {
            scope,
            path,
            dataset: dataset.to_string(),
        }
    }

    /// Put what is absent into the `--json` answer. Only when something is, so
    /// the answer over a held scope is the same as it was.
    fn add_to(&self, answer: &mut serde_json::Value) {
        if let Some(scope) = &self.scope {
            answer["absent_scope"] = serde_json::json!(scope);
        }
        if let Some(path) = &self.path {
            answer["absent_path"] = serde_json::json!(path);
        }
    }

    /// The lines a person reads before the count.
    fn notice(&self, exact: bool) -> Option<String> {
        if let Some(scope) = &self.scope {
            return Some(scope_notice(scope, &self.dataset, exact));
        }
        let path = self.path.as_ref()?;
        Some(absent_notice(path, exact, "this search reads"))
    }
}

/// The lines that say which filter matched no expression, and what is held
/// near it.
fn scope_notice(scope: &AbsentScope, dataset: &str, exact: bool) -> String {
    match scope {
        AbsentScope::Work {
            work,
            works_held,
            held,
            beneath,
        } => {
            let mut notice = format!("This dataset holds no work {work}.");
            if !beneath.is_empty() {
                notice.push_str(&format!("\nHeld works beneath it: {}", first_few(beneath)));
            } else if !held.is_empty() {
                notice.push_str(&format!("\nHeld works: {}", held.join(", ")));
            } else {
                notice.push_str(&format!(
                    "\nIt holds {works_held} works. \
                     `words_to_data expressions {dataset}` lists them."
                ));
            }
            notice
        }
        AbsentScope::Date {
            date,
            work,
            path,
            held_before,
            held_after,
        } => {
            let missing = match (path, work) {
                (Some(path), _) => {
                    format!("No expression dated {date} holds {path}{}.", beneath(exact))
                }
                (None, Some(work)) => {
                    format!("This dataset holds no expression of {work} dated {date}.")
                }
                (None, None) => format!("This dataset holds no expression dated {date}."),
            };
            let none = || "none".to_string();
            format!(
                "{missing}\nHeld dates around it: {} before, {} after",
                held_before.clone().unwrap_or_else(none),
                held_after.clone().unwrap_or_else(none),
            )
        }
    }
}

/// The words that say a subtree was asked for, as `absent_notice` says them.
fn beneath(exact: bool) -> &'static str {
    if exact { "" } else { " or a path beneath it" }
}

/// The first few names, and how many more there are.
fn first_few(names: &[String]) -> String {
    let shown = names[..names.len().min(FEW_WORKS)].join(", ");
    match names.len().saturating_sub(FEW_WORKS) {
        0 => shown,
        more => format!("{shown}, and {more} more"),
    }
}

/// `n` and the noun, singular or plural as `n` asks.
fn counted(n: usize, noun: &str) -> String {
    if n == 1 {
        format!("1 {noun}")
    } else {
        format!("{n} {noun}s")
    }
}

/// Each occurrence in each field, as a hit of its own, in the order found.
fn each_occurrence(fields: &[SearchResult], width: usize) -> Vec<Occurrence> {
    fields
        .iter()
        .flat_map(|hit| {
            hit.offsets.iter().map(move |&offset| Occurrence {
                expression: hit.expression.clone(),
                path: hit.path.clone(),
                field: hit.field.clone(),
                offset,
                snippet: window_at(&hit.snippet, offset, width),
            })
        })
        .collect()
}

/// Write the occurrences to stdout, and say how many the limit left out.
///
/// `total` counts every occurrence the search found, listed or not.
fn report_occurrences(
    args: &Args,
    listed: &[Occurrence],
    total: usize,
    absent: &Absent,
) -> std::io::Result<()> {
    let mut out = std::io::stdout().lock();

    if args.json {
        let mut answer = serde_json::json!({
            "hits": listed,
            "total": total,
        });
        absent.add_to(&mut answer);
        writeln!(out, "{}", serde_json::to_string_pretty(&answer).unwrap())?;
        return out.flush();
    }

    if let Some(notice) = absent.notice(args.exact) {
        writeln!(out, "{notice}")?;
    }

    for hit in listed {
        writeln!(
            out,
            "{}  {}  [{}] at {}",
            hit.expression, hit.path, hit.field, hit.offset
        )?;
        writeln!(out, "    {}", hit.snippet)?;
    }
    writeln!(out, "{}", counted(total, "occurrence"))?;
    let dropped = total.saturating_sub(listed.len());
    if dropped > 0 {
        writeln!(
            out,
            "  … {dropped} more not shown; pass --all for every one of them"
        )?;
    }
    out.flush()
}

/// Where the first occurrence in a field begins, or the field's start.
///
/// A field holds the query, which is what made it a hit, but the two back ends
/// find a hit by different means, so a field without an offset falls back to
/// its opening rather than refusing to print.
fn first_offset(hit: &SearchResult) -> usize {
    hit.offsets.first().copied().unwrap_or(0)
}

/// `width` characters of `text` around the character at `centre`.
///
/// The centre is placed in the middle, so the words on both sides of it are
/// what a reader gets. An elision is marked with `…` at whichever end was cut: a
/// snippet that begins mid-sentence with no mark reads as the start of the
/// provision.
///
/// A width of zero returns the whole field, so a reader who wants the section's
/// text can still ask for it.
///
/// Counted in characters and not bytes, as the offsets are. The corpus holds
/// `§`, and a byte slice through a character panics.
fn window_at(text: &str, centre: usize, width: usize) -> String {
    if width == 0 {
        return text.trim().to_string();
    }
    let characters: Vec<char> = text.chars().collect();
    if characters.len() <= width {
        return text.trim().to_string();
    }

    let start = centre
        .saturating_sub(width / 2)
        .min(characters.len() - width);
    let end = start + width;

    // The edges of the field are trimmed, as a whole field is: most fields
    // begin with the white space of their markup. A cut edge is not trimmed,
    // so a window is the characters asked for, exactly.
    let window: String = characters[start..end].iter().collect();
    let mut snippet = String::new();
    if start > 0 {
        snippet.push('…');
        snippet.push_str(&window);
    } else {
        snippet.push_str(window.trim_start());
    }
    if end < characters.len() {
        snippet.push('…');
    } else {
        snippet.truncate(snippet.trim_end().len());
    }
    snippet
}
