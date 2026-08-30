//! `words_to_data diff` — list the paths that changed between two versions.

use clap::Args as ClapArgs;
use words_to_data::inspect;

use crate::load::{self, with_dataset};

#[derive(ClapArgs)]
pub struct Args {
    /// Dataset file (`.json` compact or `.sqlite`)
    pub dataset: String,

    /// Older version date (YYYY-MM-DD)
    #[arg(long)]
    pub from: String,

    /// Newer version date (YYYY-MM-DD)
    #[arg(long)]
    pub to: String,

    /// Emit JSON instead of human-readable text
    #[arg(long)]
    pub json: bool,
}

pub fn run(args: Args) {
    let ds = load::open(&args.dataset).expect("Error opening dataset");
    let summary = with_dataset!(ds, d => inspect::diff(&d, &args.from, &args.to))
        .expect("Error computing diff");

    if args.json {
        println!("{}", serde_json::to_string_pretty(&summary).unwrap());
        return;
    }

    println!("{} -> {}", summary.from_date, summary.to_date);
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
