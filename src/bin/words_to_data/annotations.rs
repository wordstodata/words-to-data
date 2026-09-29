//! `words_to_data annotations` — list annotations, filtered by any combination
//! of window, bill and path.
//!
//! The filters **compose**. They used to be mutually exclusive, because the query
//! beneath them was a sum type that could hold one of them at a time, so
//! *"what did this bill change at § 174"* could not be asked and the way through
//! was to diff a whole title and grep the output (#234).

use clap::Args as ClapArgs;
use words_to_data::dataset::ExpressionId;
use words_to_data::inspect::{self, PathMatch};
use words_to_data::legislature::evidence_matching::RecordedSource;
use words_to_data::query::{LinkQuery, Locator};

use crate::load::{self, with_dataset};

#[derive(ClapArgs)]
pub struct Args {
    /// Dataset file (`.json` compact or `.sqlite`)
    pub dataset: String,

    /// Older expression of the pair, e.g. `uscode/title_9@2025-07-18` (requires `--to`)
    #[arg(long, requires = "to")]
    pub from: Option<ExpressionId>,

    /// Newer expression of the pair (requires `--from`)
    #[arg(long, requires = "from")]
    pub to: Option<ExpressionId>,

    /// Filter to annotations sourced from this bill
    ///
    /// Combines with `--path` and `--from`/`--to`: every filter given is applied
    #[arg(long)]
    pub bill: Option<String>,

    /// Filter to annotations on this structural path or on any path beneath it
    #[arg(long)]
    pub path: Option<String>,

    /// Match `--path` exactly: keep only annotations recorded on that path itself
    #[arg(long, requires = "path")]
    pub exact: bool,

    /// How many to print before stopping. The run says how many it did not show
    ///
    /// Human output stops at 20 unless told otherwise. `--json` carries them all,
    /// as `redesignation-report` does, unless this is given explicitly
    #[arg(long)]
    pub limit: Option<usize>,

    /// Print every match, however many there are
    #[arg(long, conflicts_with = "limit")]
    pub all: bool,

    /// Emit JSON instead of human-readable text
    #[arg(long)]
    pub json: bool,
}

pub fn run(args: Args) {
    let mut locator = Locator::new();
    if let (Some(from), Some(to)) = (&args.from, &args.to) {
        locator = locator
            .in_work(from.work.to_string())
            .in_window(from.at.clone(), to.at.clone());
    }
    if let Some(path) = &args.path {
        locator = locator.at_path(path, path_matching(args.exact));
    }

    let mut query = LinkQuery::new().at(locator);
    if let Some(bill) = &args.bill {
        // The query is class-neutral: it filters on a prefix of the object's
        // reference, and turning a bill into that prefix is the CLI's job
        // (`docs/adr/0002-links-live-in-the-core.md`).
        query = query.with_object_prefix(words_to_data::link::bill_reference_prefix(bill));
    }
    // A screenful for a person, everything for a machine. An explicit `--limit`
    // is honoured either way, and `--all` lifts it.
    let limit = match (args.all, args.limit, args.json) {
        (true, _, _) => None,
        (_, Some(explicit), _) => Some(explicit),
        (_, None, true) => None,
        (_, None, false) => Some(words_to_data::query::DEFAULT_LIMIT),
    };
    if let Some(limit) = limit {
        query = query.with_limit(limit);
    }

    let ds = crate::fail::or_exit(load::open(&args.dataset), "Error opening dataset");
    let found = crate::fail::or_exit(
        with_dataset!(ds, d => inspect::annotations(&d, &query)),
        "Error reading annotations",
    );

    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "annotations": found.rows,
                "total": found.total,
            }))
            .unwrap()
        );
        return;
    }

    for a in &found.rows {
        print_annotation(a);
    }
    println!("{} annotation(s)", found.total);
    // What was left out, said rather than left to be inferred. A listing that
    // stopped in silence would read as the whole answer (#220, #235).
    if found.dropped() > 0 {
        println!(
            "  … {} more not shown; pass --all for every one of them",
            found.dropped()
        );
    }
}

/// Read the `--exact` flag both path-taking commands carry.
///
/// The subtree is the default: a person names a section, and a bill amends a
/// clause inside it. Shared with the `path` command so one flag cannot come to
/// mean two things.
pub fn path_matching(exact: bool) -> PathMatch {
    if exact {
        PathMatch::Exact
    } else {
        PathMatch::Subtree
    }
}

/// Print one annotation's headline: status, operation, bill, short amendment id,
/// confidence, annotator, and the causative instruction snippet. Shared with the
/// `path` command so both surfaces stay consistent.
///
/// Every path is printed beside the id of the link that states it, and that id
/// is what `settle` takes. A record covering three paths is three links, so one
/// id for the record would let a reviewer settle the first and read the record
/// as done (#232).
pub fn print_annotation(a: &words_to_data::inspect::AnnotationSummary) {
    let confidence = a
        .confidence
        .map(|c| format!("{c:.2}"))
        .unwrap_or_else(|| "-".to_string());
    let short_id: String = a.amendment_id.chars().take(12).collect();
    // The method, where the links record one. Every link of a record has one
    // maker, and a maker runs one method at a time.
    let method = a
        .links
        .iter()
        .find_map(|link| link.made.method.as_deref())
        .map(|method| format!(", {method}"))
        .unwrap_or_default();
    println!(
        "  [{}] {} {} -> {}  {} {} amd {} (conf {}, by {}{method})",
        a.status,
        a.work,
        a.from_date,
        a.to_date,
        a.operation,
        a.bill_id,
        short_id,
        confidence,
        a.annotator
    );
    let text = a.causative_text.trim();
    if !text.is_empty() {
        println!("      {}", truncate(text, 100));
    }
    // One line per path, each beside the id of the link that states it. Printed
    // here rather than by the caller so that `path` names its annotations too:
    // both commands are doors into a review, and the id is what opens it.
    for link in &a.links {
        println!("      link {}  {}{}", link.id, link.path, decision(link));
    }
}

/// How a link was decided, in a few words: `  [elimination, address from
/// olrc, 1 of 5 causes]`. The kind is the word `--json` gives as `chosen`, so
/// what a person reads is what an agent filters on. An outdated link says so,
/// with the version that made it and the newer one that did not make it again.
/// Empty for a link with nothing of the kind to say.
fn decision(link: &words_to_data::inspect::LinkMade) -> String {
    let made = &link.made;
    let mut parts: Vec<String> = Vec::new();
    if let Some(outdated) = &link.outdated {
        parts.push(format!(
            "outdated: made by @{}, not made again by @{}",
            outdated.made_by.version, outdated.remade_by.version
        ));
    }
    if let Some(recorded) = &made.recorded {
        parts.push(
            serde_json::to_value(recorded.chosen)
                .ok()
                .and_then(|kind| kind.as_str().map(str::to_string))
                .unwrap_or_default(),
        );
        if recorded.address_source == RecordedSource::Olrc {
            parts.push("address from olrc".to_string());
        }
    }
    if made.causes > 1 {
        parts.push(format!("1 of {} causes", made.causes));
    }
    if parts.is_empty() {
        return String::new();
    }
    format!("  [{}]", parts.join(", "))
}

/// Shorten `text` to `max` characters, adding an ellipsis when clipped.
fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let clipped: String = text.chars().take(max).collect();
    format!("{clipped}…")
}
