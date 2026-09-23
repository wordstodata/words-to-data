//! `words_to_data validate` — check a dataset's internal consistency, and say
//! which steps it still needs.
//!
//! Exits non-zero when any issue is found, so it can gate CI or scripts.
//!
//! **Two lists, and a reader acts on them differently.** An issue is a fault in
//! what the dataset holds. A bill and window pair is work nobody has run yet
//! (#183): the dataset is sound, and one step over it is outstanding. Both make
//! the run fail, because a dataset that is not finished must not report that it
//! is.

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
        if !report.issues.is_empty() {
            println!(
                "FAILED — {} issue(s) across {} annotation(s):",
                report.issues.len(),
                report.checked_annotations
            );
            for issue in &report.issues {
                println!("  - {issue}");
            }
        }

        // The work-list, in its own section (#183). It is not a fault in the
        // file: it is a step nobody has run, and each line is the command that
        // runs it. `add-release-points` names the steps a new window needs in
        // the same words.
        if !report.unresolved_redesignations.is_empty() {
            println!(
                "\n{} bill and window pair(s) hold redesignation statements no step has \
                 resolved:",
                report.unresolved_redesignations.len()
            );
            for outstanding in &report.unresolved_redesignations {
                println!("  - {outstanding}");
            }
        }
    }

    if !report.ok {
        std::process::exit(1);
    }
}
