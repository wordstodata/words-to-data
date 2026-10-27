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
use words_to_data::inspect::{self, MovedPath, PathMatch, absent_path};
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

pub fn run(mut args: Args) {
    let ds = crate::fail::or_exit(load::open(&args.dataset), "Error opening dataset");
    crate::held::refuse_unless_window_held(&ds, &args.from, &args.to);
    // The pair is the window, which `diff` already took. Only the path is new.
    let mut locator = Locator::new()
        .in_work(args.from.work.to_string())
        .in_window(args.from.at.clone(), args.to.at.clone());
    if let Some(typed) = &args.path {
        let path = with_dataset!(&ds, d => crate::held::path_argument(d, typed));
        locator = locator.at_path(&path, path_matching(args.exact));
        args.path = Some(path);
    }

    refuse_a_path_neither_end_holds(&ds, &args, &locator);
    let mut summary = crate::fail::or_exit(
        with_dataset!(&ds, d => inspect::diff(d, &args.from, &args.to)),
        "Error computing diff",
    );

    // Narrowed here rather than inside the diff: a tree diff compares two whole
    // documents, and a change is found before anybody can ask whether it is
    // inside the scope.
    summary.changed_paths.retain(|path| locates(&locator, path));
    // A provision added or removed whole is one path, with nothing listed
    // beneath it. A scope inside it was added or removed with it, so the
    // provision that holds the scope is kept (#336).
    summary
        .added_paths
        .retain(|path| locates(&locator, path) || holds_the_scope(&locator, path));
    summary
        .removed_paths
        .retain(|path| locates(&locator, path) || holds_the_scope(&locator, path));
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
    // Said aloud, because the path listed is not the path asked for.
    for (whole, how) in [
        (&summary.added_paths, "added"),
        (&summary.removed_paths, "removed"),
    ] {
        for path in whole {
            if let Some(asked) = args.path.as_deref().filter(|asked| asked != path)
                && holds_the_scope(&locator, path)
            {
                println!("\n{asked} is inside {path}, which was {how} whole.");
            }
        }
    }

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

/// Stop when neither expression holds the path, or a path beneath it (#305).
///
/// "Changed (0)" over a guessed path read as proof that nothing changed there,
/// and an agent reported a held section as missing from the dataset. A diff
/// that compared no provision has no answer to give, so it fails and says why.
/// A path that one end holds is a real addition or removal, and is listed.
fn refuse_a_path_neither_end_holds(ds: &load::OpenDataset, args: &Args, locator: &Locator) {
    let at_end = |id: &ExpressionId| {
        let one_end = locator.clone().in_window(id.at.clone(), id.at.clone());
        crate::fail::or_exit(
            with_dataset!(ds, d => absent_path(d, &one_end)),
            "Error checking the path",
        )
    };
    let (Some(at_from), Some(at_to)) = (at_end(&args.from), at_end(&args.to)) else {
        return;
    };
    // Each end may hold a different part of the path. The longer part is
    // nearer to what was asked.
    let nearest_held = [at_from.nearest_held, at_to.nearest_held]
        .into_iter()
        .flatten()
        .max_by_key(String::len);
    let beneath = if args.exact {
        ""
    } else {
        " or a path beneath it"
    };
    let mut message = format!(
        "Neither {} nor {} holds {}{beneath}, so the diff compares no provision there.",
        args.from, args.to, at_from.path
    );
    if let Some(nearest) = nearest_held {
        message.push_str(&format!("\nNearest held path: {nearest}"));
    }
    crate::fail::refuse(&message);
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

/// Whether a path is a provision above the locator's path: the scope is
/// inside it.
fn holds_the_scope(locator: &Locator, path: &str) -> bool {
    locator
        .path
        .as_deref()
        .is_some_and(|asked| words_to_data::uslm::path::covers_path(path, asked))
}

fn print_paths(label: &str, paths: &[String]) {
    println!("\n{label} ({}):", paths.len());
    for p in paths {
        println!("  {p}");
    }
}
