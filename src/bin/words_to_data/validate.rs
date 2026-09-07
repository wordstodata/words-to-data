//! `words_to_data validate` — check a dataset's internal consistency.
//!
//! Exits non-zero when any issue is found, so it can gate CI or scripts.

use clap::Args as ClapArgs;
use words_to_data::inspect;

use crate::load::{self, with_dataset};

#[derive(ClapArgs)]
pub struct Args {
    /// Dataset file (`.json` compact or `.sqlite`)
    pub dataset: String,

    /// Emit JSON instead of human-readable text
    #[arg(long)]
    pub json: bool,
}

pub fn run(args: Args) {
    let ds = crate::fail::or_exit(load::open(&args.dataset), "Error opening dataset");
    let report = crate::fail::or_exit(
        with_dataset!(ds, d => inspect::validate(&d)),
        "Error validating dataset",
    );

    if args.json {
        println!("{}", serde_json::to_string_pretty(&report).unwrap());
    } else if report.ok {
        println!(
            "OK — {} annotation(s) checked, no issues",
            report.checked_annotations
        );
    } else {
        println!(
            "FAILED — {} issue(s) across {} annotation(s):",
            report.issues.len(),
            report.checked_annotations
        );
        for issue in &report.issues {
            println!("  - {issue}");
        }
    }

    if !report.ok {
        std::process::exit(1);
    }
}
