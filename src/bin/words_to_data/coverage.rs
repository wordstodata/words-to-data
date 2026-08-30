//! `words_to_data coverage` — how much of a version diff has been annotated.
//!
//! Reports the annotated vs unannotated split of the change universe. Full
//! coverage is not the goal: many changed paths are not amendment-caused.

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

    /// Print the full unannotated path list (human output truncates otherwise)
    #[arg(long)]
    pub list: bool,

    /// Emit JSON instead of human-readable text
    #[arg(long)]
    pub json: bool,
}

pub fn run(args: Args) {
    let ds = load::open(&args.dataset).expect("Error opening dataset");
    let report = with_dataset!(ds, d => inspect::coverage(&d, &args.from, &args.to))
        .expect("Error computing coverage");

    if args.json {
        println!("{}", serde_json::to_string_pretty(&report).unwrap());
        return;
    }

    println!("{} -> {}", report.from_date, report.to_date);
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
