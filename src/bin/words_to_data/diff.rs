//! `words_to_data diff` — the paths that changed between two expressions.
//!
//! Over the real corpus this printed 461 changed paths with no way to name the
//! part of the title being read, so the way through was to grep the output
//! (#235). The window was already there; the path was not.
//!
//! The scope is the `Locator` #234 settled, not a flag of this command's own, so
//! `diff`, `search` and `annotations` mean the same thing by a path.

use clap::Args as ClapArgs;
use words_to_data::dataset::ExpressionId;
use words_to_data::inspect::{self, MovedPath, PathMatch};
use words_to_data::query::Locator;

use crate::annotations::path_matching;
use crate::load::{self, with_dataset};

#[derive(ClapArgs)]
pub struct Args {
    /// Dataset file (`.json` compact or `.sqlite`)
    pub dataset: String,

    /// Older expression, e.g. `uscode/title_9@2025-07-18`
    #[arg(long)]
    pub from: ExpressionId,

    /// Newer expression of the same work
    #[arg(long)]
    pub to: ExpressionId,

    /// List only the changes at this structural path or beneath it
    #[arg(long)]
    pub path: Option<String>,

    /// Match `--path` exactly: list the change at that provision alone
    #[arg(long, requires = "path")]
    pub exact: bool,

    /// Emit JSON instead of human-readable text
    #[arg(long)]
    pub json: bool,
}

pub fn run(args: Args) {
    // The pair is the window, which `diff` already took. Only the path is new.
    let mut locator = Locator::new()
        .in_work(args.from.work.to_string())
        .in_window(args.from.at.clone(), args.to.at.clone());
    if let Some(path) = &args.path {
        locator = locator.at_path(path, path_matching(args.exact));
    }

    let ds = crate::fail::or_exit(load::open(&args.dataset), "Error opening dataset");
    let mut summary = crate::fail::or_exit(
        with_dataset!(ds, d => inspect::diff(&d, &args.from, &args.to)),
        "Error computing diff",
    );

    // Narrowed here rather than inside the diff: a tree diff compares two whole
    // documents, and a change is found before anybody can ask whether it is
    // inside the scope.
    summary.changed_paths.retain(|path| locates(&locator, path));
    summary.added_paths.retain(|path| locates(&locator, path));
    summary.removed_paths.retain(|path| locates(&locator, path));
    // A move has two ends. Either one inside the scope makes it a move the
    // reader asked about: a section that moved out of the subtree is a change to
    // that subtree, and dropping it would say the subtree is unchanged.
    summary
        .moved_paths
        .retain(|moved: &MovedPath| locates(&locator, &moved.from) || locates(&locator, &moved.to));

    if args.json {
        println!("{}", serde_json::to_string_pretty(&summary).unwrap());
        return;
    }

    println!("{} -> {}", summary.from, summary.to);
    if let Some(path) = &args.path {
        // Said aloud, because a short list under a scope and a short list over a
        // whole title read exactly alike on a terminal.
        println!("at {path}{}", if args.exact { " (exactly)" } else { "" });
    }
    print_paths("Changed", &summary.changed_paths);
    print_paths("Added", &summary.added_paths);
    print_paths("Removed", &summary.removed_paths);

    // A move is not a removal plus an addition, so it gets its own list. An
    // empty one is not printed: every dataset has none until `redesignations`
    // has run, and a heading saying "Moved (0)" reads as a claim that nothing
    // moved (#93).
    if !summary.moved_paths.is_empty() {
        println!("\nMoved ({}):", summary.moved_paths.len());
        for moved in &summary.moved_paths {
            println!("  {} -> {}", moved.from, moved.to);
        }
    }
}

/// Whether a locator's path reaches this one.
///
/// The rule is `PathMatch`'s, so a path means here what it means to a link
/// query. `PathMatch::accepts` is crate-private to the library, so the two arms
/// are spelled out.
fn locates(locator: &Locator, path: &str) -> bool {
    match (&locator.path, locator.matching) {
        (None, _) => true,
        (Some(asked), PathMatch::Subtree) => words_to_data::uslm::path::covers_path(asked, path),
        (Some(asked), PathMatch::Exact) => asked == path,
    }
}

fn print_paths(label: &str, paths: &[String]) {
    println!("\n{label} ({}):", paths.len());
    for p in paths {
        println!("  {p}");
    }
}
