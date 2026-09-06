//! `words_to_data convert-dataset` — convert a dataset between compact JSON and
//! SQLite, in either direction. Direction is inferred from the file extensions.

use clap::Args as ClapArgs;
use words_to_data::dataset::{Dataset, Format};

use crate::load::is_sqlite;

#[derive(ClapArgs)]
pub struct Args {
    /// Input dataset (`.json` compact or `.sqlite`)
    pub input: String,

    /// Output dataset. If omitted, the input's extension is swapped
    /// (`.json` <-> `.sqlite`).
    pub output: Option<String>,
}

pub fn run(args: Args) {
    let output = args.output.unwrap_or_else(|| default_output(&args.input));

    match (is_sqlite(&args.input), is_sqlite(&output)) {
        (false, true) => {
            println!("Converting compact JSON -> SQLite...");
            let dataset = crate::fail::or_exit(
                Dataset::load(&args.input, Format::Compact),
                "Error loading dataset",
            );
            // Start from an empty file. Writing into an existing database keeps
            // whatever it already held: rows of a different dataset that share
            // no key survive, and a table from an older build keeps its narrower
            // shape. Converting names an output, so producing that output rather
            // than a mixture of it and its predecessor is the whole job.
            crate::fail::or_exit(
                remove_existing(&output),
                "Error replacing the output dataset",
            );
            crate::fail::or_exit(dataset.save_to_sqlite(&output), "Error saving SQLite");
        }
        (true, false) => {
            println!("Converting SQLite -> compact JSON...");
            let dataset =
                crate::fail::or_exit(Dataset::open_sqlite(&args.input), "Error opening SQLite");
            let memory = crate::fail::or_exit(dataset.to_memory(), "Error reading SQLite");
            // Writing JSON truncates, so this direction already replaces.
            crate::fail::or_exit(memory.save(&output, Format::Compact), "Error saving JSON");
        }
        _ => {
            eprintln!(
                "Nothing to convert: input and output are the same format ({} -> {}). \
                 Expected .json <-> .sqlite.",
                args.input, output
            );
            std::process::exit(2);
        }
    }

    println!("{output}");
}

/// Delete the output file if it is already there, so the conversion starts clean.
///
/// A missing file is the normal case and not an error.
fn remove_existing(path: &str) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

/// Swap a `.json` input for `.sqlite` output and vice versa.
fn default_output(input: &str) -> String {
    if is_sqlite(input) {
        let stem = input
            .strip_suffix(".sqlite")
            .or_else(|| input.strip_suffix(".db"))
            .unwrap_or(input);
        format!("{stem}.json")
    } else {
        let stem = input.strip_suffix(".json").unwrap_or(input);
        format!("{stem}.sqlite")
    }
}
