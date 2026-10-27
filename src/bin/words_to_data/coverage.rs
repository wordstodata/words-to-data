//! `words_to_data coverage` — how much of a diff has been annotated.
//!
//! Reports the annotated vs unannotated split of the change universe. Full
//! coverage is not the goal: many changed paths are not amendment-caused.

use clap::Args as ClapArgs;
use words_to_data::dataset::ExpressionId;
use words_to_data::inspect;
use words_to_data::query::DEFAULT_LIMIT;

use crate::load::{self, with_dataset};

#[derive(ClapArgs)]
pub struct Args {
    /// Dataset file (`.json` compact or `.sqlite`)
    pub dataset: String,

    /// Older end of the span, e.g. `uscode/title_9@2025-07-18`
    ///
    /// A span can hold more than one window. The diff is then the diff of the
    /// whole span, and a changed path is annotated when a link of any window
    /// inside the span names it
    #[arg(long)]
    pub from: ExpressionId,

    /// Newer end of the span, an expression of the same work
    #[arg(long)]
    pub to: ExpressionId,

    /// How many unannotated paths to print before stopping. The run says how
    /// many it did not show
    ///
    /// Human output stops at 20 unless told otherwise. `--json` carries them all,
    /// as `annotations` does, unless this is given explicitly
    #[arg(long)]
    pub limit: Option<usize>,

    /// Print every unannotated path, however many there are
    //
    // `--list` is the old name, kept hidden for one release (#396).
    #[arg(long, alias = "list", conflicts_with = "limit")]
    pub all: bool,

    /// Emit JSON instead of human-readable text
    #[arg(long)]
    pub json: bool,
}

pub fn run(args: Args) {
    let ds = crate::fail::or_exit(load::open(&args.dataset), "Error opening dataset");
    crate::held::refuse_unless_window_held(&ds, &args.from, &args.to);
    let mut report = crate::fail::or_exit(
        with_dataset!(ds, d => inspect::coverage(&d, &args.from, &args.to)),
        "Error computing coverage",
    );

    // A screenful for a person, everything for a machine, as every list
    // command does (#396). An explicit `--limit` is honoured either way, and
    // `--all` lifts it. The counts always cover every path.
    let limit = match (args.all, args.limit, args.json) {
        (true, _, _) | (_, None, true) => report.unannotated_paths.len(),
        (_, Some(explicit), _) => explicit,
        (_, None, false) => DEFAULT_LIMIT,
    };
    let left_out = report.unannotated_paths.len().saturating_sub(limit);
    report.unannotated_paths.truncate(limit);

    if args.json {
        println!("{}", serde_json::to_string_pretty(&report).unwrap());
        return;
    }

    println!("{} -> {}", report.from, report.to);
    println!("Changed paths:     {}", report.changed_path_count);
    println!("Annotated:         {}", report.annotated_count);
    println!("Unannotated:       {}", report.unannotated_count);
    println!("Coverage:          {:.1}%", report.coverage * 100.0);

    // Not every changed path is caused by an amendment (editorial edits,
    // reclassifications, etc.), so 100% coverage is not the goal — these are
    // simply the changed paths without an annotation.
    if report.unannotated_paths.is_empty() {
        return;
    }
    match left_out {
        0 => println!("\nUnannotated changed paths:"),
        _ => println!(
            "\nUnannotated changed paths (first {} of {}):",
            report.unannotated_paths.len(),
            report.unannotated_count
        ),
    }
    for p in &report.unannotated_paths {
        println!("  {p}");
    }
    if left_out > 0 {
        println!("  … {left_out} more; pass --all for all of them, or --json to pipe");
    }
}
