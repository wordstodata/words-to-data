//! `words_to_data add-bills` — put more Congress bills into a dataset that is
//! already there (#272).
//!
//! A dataset could grow in time (`add-release-points`) and not in law: only
//! `build-dataset --bills` loaded a bill, and it only made a new dataset. So a
//! law enacted after the build could not come in without a rebuild, and a
//! rebuild discards every review.
//!
//! **It loads the bill as a build loads it.** Both commands call
//! [`crate::congress_bills::add_all`], so a dataset that took a bill afterwards
//! and a dataset built with it are the same dataset.
//!
//! **Where the result goes is not the same for the two forms.** A SQLite dataset
//! changes in place. A compact JSON dataset is written whole, so it must be told
//! where to write and is never written back over its input (#186).
//!
//! **It runs no step but the renumbering step.** The run ends by naming the
//! steps that read a new law, for the operator to run.

use clap::Args as ClapArgs;
use words_to_data::dataset::Dataset;
use words_to_data::storage::{LegislatureReader, LegislatureWriter, Storage};

use crate::ui;

#[derive(ClapArgs)]
pub struct Args {
    /// Path to the dataset to add the bills to (compact JSON or SQLite)
    pub dataset: String,

    /// Congress bills to add (e.g. `119-s-1071,119-hr-998`), comma-separated;
    /// requires CONGRESS_API_KEY unless the run is offline
    #[arg(long, value_delimiter = ',', required = true)]
    pub bills: Vec<String>,

    /// Cache directory for Congress responses
    /// (default: the shared `<user cache dir>/words_to_data`)
    #[arg(long)]
    pub cache_dir: Option<String>,

    /// Read only the cache, and never reach the network
    ///
    /// A bill the cache has not got stops the run by name, and the dataset is
    /// not changed.
    #[arg(long)]
    pub offline: bool,

    /// Where to write a compact JSON dataset. Required for compact JSON, which
    /// is never written back over its input. Ignored for SQLite, which changes
    /// in place.
    #[arg(long)]
    pub output: Option<String>,
}

pub fn run(args: Args) {
    // Where the result goes: `None` is SQLite, which changes in place. Compact
    // JSON must be told, and it is asked before anything is downloaded.
    let output = if crate::load::is_sqlite(&args.dataset) {
        None
    } else {
        Some(crate::load::output_or_refuse(
            &args.dataset,
            args.output.as_deref(),
            "add-bills",
        ))
    };

    let client = crate::congress_bills::client(args.cache_dir.as_deref(), args.offline);

    match output {
        None => {
            let mut dataset =
                crate::fail::or_exit(Dataset::open_sqlite(&args.dataset), "Error opening dataset");
            grow(&mut dataset, &client, &args.bills, &args.dataset);
            println!("\nWrote {}", args.dataset);
        }
        Some(output) => {
            let mut dataset = crate::fail::or_exit(
                crate::load::load_compact(&args.dataset),
                "Error loading dataset",
            );
            grow(&mut dataset, &client, &args.bills, output);
            crate::fail::or_exit(
                crate::load::save_compact(&dataset, output),
                "Error saving dataset",
            );
            println!("\nWrote {output}");
        }
    }
}

/// Add the bills, and say what was added and what to run next.
///
/// `written` is the file the result lands in, so the steps this run names can be
/// run as they are printed.
fn grow<S: Storage + LegislatureReader + LegislatureWriter>(
    dataset: &mut Dataset<S>,
    client: &words_to_data::congress::CongressClient,
    bills: &[String],
    written: &str,
) {
    let added = crate::fail::or_exit(
        crate::congress_bills::add_all(dataset, client, bills),
        "Error adding bills",
    );
    for bill in &added.already_held {
        println!("The dataset already holds {bill}. Nothing was added for it.");
    }
    println!(
        "\nAdded {} bill(s): {}",
        added.loaded.len(),
        added.loaded.join(", ")
    );
    report_next_steps(&added.loaded, written);
}

/// Name the steps that read a new law, which this command does not run.
///
/// The renumbering step ran as part of the load. Classification, matching and
/// the residue did not: they are their own commands, as after
/// `build-dataset`. A compact JSON dataset is never written back over its
/// input, so each step there names a new file.
fn report_next_steps(loaded: &[String], written: &str) {
    if loaded.is_empty() {
        return;
    }
    println!("\nNo other step has run over the new bill(s). To run the steps that read a new law:");
    if crate::load::is_sqlite(written) {
        println!(
            "  {}",
            ui::command(&format!("words_to_data add-classifications {written}"))
        );
        println!(
            "  {}",
            ui::command(&format!("words_to_data link-by-evidence {written}"))
        );
        for bill in loaded {
            println!(
                "  {}",
                ui::command(&format!("words_to_data residue {written} --bill {bill}"))
            );
        }
    } else {
        println!(
            "  {}",
            ui::command(&format!(
                "words_to_data add-classifications {written} --output <classified.json>"
            ))
        );
        println!(
            "  {}",
            ui::command("words_to_data link-by-evidence <classified.json> --output <linked.json>")
        );
        for bill in loaded {
            println!(
                "  {}",
                ui::command(&format!(
                    "words_to_data residue <linked.json> --bill {bill}"
                ))
            );
        }
    }
}
