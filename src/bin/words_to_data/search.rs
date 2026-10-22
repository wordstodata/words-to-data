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
use words_to_data::dataset::SearchResult;
use words_to_data::inspect::{AbsentPath, absent_path};
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

    /// Text to search for (case-insensitive)
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

    /// Emit JSON instead of human-readable text
    #[arg(long)]
    pub json: bool,
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
    // Asked only when nothing matched: a hit proves the path is held. An empty
    // search over a guessed path must not read like one over a real path (#305).
    let absent = match found.total {
        0 => crate::fail::or_exit(
            with_dataset!(&ds, d => absent_path(d, &locator)),
            "Error checking the path",
        ),
        _ => None,
    };

    // The window is applied to both surfaces. The field is called `snippet`, and
    // a snippet that is a whole section is not one. The field as recorded is a
    // `--snippet 0` away, and `words_to_data path` reads the provision itself.
    let hits: Vec<_> = found
        .rows
        .iter()
        .map(|hit| SearchResult {
            snippet: window_around(&hit.snippet, &args.query, args.snippet),
            ..hit.clone()
        })
        .collect();

    // A reader that closed the pipe has read enough, which is a clean stop and
    // not a failure. `println!` panics on a broken pipe, so `search | head`
    // exited 101 where the same run without a pipe exited 0 (#235).
    if let Err(error) = report(&args, &hits, found.total, found.dropped(), absent.as_ref())
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
    total: usize,
    dropped: usize,
    absent: Option<&AbsentPath>,
) -> std::io::Result<()> {
    let mut out = std::io::stdout().lock();

    if args.json {
        let mut answer = serde_json::json!({
            "hits": hits,
            "total": total,
        });
        // Only when the path is absent, so the answer over a real path is the
        // same as it was.
        if let Some(absent) = absent {
            answer["absent_path"] = serde_json::json!(absent);
        }
        writeln!(out, "{}", serde_json::to_string_pretty(&answer).unwrap())?;
        return out.flush();
    }

    if let Some(absent) = absent {
        writeln!(
            out,
            "{}",
            absent_notice(absent, args.exact, "this search reads")
        )?;
    }

    for hit in hits {
        writeln!(out, "{}  {}  [{}]", hit.expression, hit.path, hit.field)?;
        writeln!(out, "    {}", hit.snippet)?;
    }
    writeln!(out, "{total} match(es)")?;
    // What was left out, said rather than left to be inferred. A listing that
    // stopped in silence would read as the whole answer (#220, #235).
    if dropped > 0 {
        writeln!(
            out,
            "  … {dropped} more not shown; pass --all for every one of them"
        )?;
    }
    out.flush()
}

/// `width` characters of `text` around the first match of `query`.
///
/// The match is centred, so the words on both sides of it are what a reader
/// gets. An elision is marked with `…` at whichever end was cut: a snippet that
/// begins mid-sentence with no mark reads as the start of the provision.
///
/// A width of zero returns the whole field, so a reader who wants the section's
/// text can still ask for it.
///
/// Counted in characters and not bytes. The corpus holds `§`, and a byte slice
/// through a character panics.
fn window_around(text: &str, query: &str, width: usize) -> String {
    let text = text.trim();
    if width == 0 {
        return text.to_string();
    }
    let characters: Vec<char> = text.chars().collect();
    if characters.len() <= width {
        return text.to_string();
    }

    // Where the match begins, in characters. A field holds the query — that is
    // what made it a hit — but the two backends match by different means, so a
    // miss falls back to the opening rather than refusing to print.
    let found = first_match(&characters, &query.chars().collect::<Vec<_>>()).unwrap_or(0);

    let start = found
        .saturating_sub(width / 2)
        .min(characters.len() - width);
    let end = start + width;

    let mut snippet = String::new();
    if start > 0 {
        snippet.push('…');
    }
    snippet.extend(&characters[start..end]);
    if end < characters.len() {
        snippet.push('…');
    }
    snippet
}

/// The character position where `needle` first appears in `haystack`, whatever
/// the case of either.
///
/// Compared character by character rather than on a lowercased copy of the whole
/// field, because lowercasing can change how many characters a string holds, and
/// a position read off the copy would then point at the wrong place in the
/// original.
fn first_match(haystack: &[char], needle: &[char]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    haystack.windows(needle.len()).position(|window| {
        window
            .iter()
            .zip(needle)
            .all(|(found, asked)| found.to_lowercase().eq(asked.to_lowercase()))
    })
}
