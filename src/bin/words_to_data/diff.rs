//! `words_to_data diff` — list the paths that changed between two expressions.

use clap::Args as ClapArgs;
use words_to_data::dataset::ExpressionId;
use words_to_data::inspect;

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

    /// Emit JSON instead of human-readable text
    #[arg(long)]
    pub json: bool,
}

pub fn run(args: Args) {
    let ds = crate::fail::or_exit(load::open(&args.dataset), "Error opening dataset");
    let summary = crate::fail::or_exit(
        with_dataset!(ds, d => inspect::diff(&d, &args.from, &args.to)),
        "Error computing diff",
    );

    if args.json {
        println!("{}", serde_json::to_string_pretty(&summary).unwrap());
        return;
    }

    println!("{} -> {}", summary.from, summary.to);
    print_paths("Changed", &summary.changed_paths);
    print_paths("Added", &summary.added_paths);
    print_paths("Removed", &summary.removed_paths);
}

fn print_paths(label: &str, paths: &[String]) {
    println!("\n{label} ({}):", paths.len());
    for p in paths {
        println!("  {p}");
    }
}
