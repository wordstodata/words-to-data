//! `words_to_data coverage` — how much of a diff has been annotated.
//!
//! Reports the annotated vs unannotated split of the change universe. Full
//! coverage is not the goal: many changed paths are not amendment-caused.

use clap::Args as ClapArgs;
use words_to_data::dataset::ExpressionId;
use words_to_data::inspect;

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

    /// Print the full unannotated path list (human output truncates otherwise)
    #[arg(long)]
    pub list: bool,

    /// Emit JSON instead of human-readable text
    #[arg(long)]
    pub json: bool,
}

pub fn run(args: Args) {
    let ds = crate::fail::or_exit(load::open(&args.dataset), "Error opening dataset");
    crate::held::refuse_unless_window_held(&ds, &args.from, &args.to);
    let report = crate::fail::or_exit(
        with_dataset!(ds, d => inspect::coverage(&d, &args.from, &args.to)),
        "Error computing coverage",
    );

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
    if args.list {
        println!("\nUnannotated changed paths:");
        for p in &report.unannotated_paths {
            println!("  {p}");
        }
    } else if !report.unannotated_paths.is_empty() {
        println!(
            "\nUnannotated changed paths (first 10 of {}):",
            report.unannotated_count
        );
        for p in report.unannotated_paths.iter().take(10) {
            println!("  {p}");
        }
        println!("  … pass --list for all, or --json to pipe");
    }
}
