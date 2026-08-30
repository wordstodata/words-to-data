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
            let dataset =
                Dataset::load(&args.input, Format::Compact).expect("Error loading dataset");
            dataset
                .save_to_sqlite(&output)
                .expect("Error saving SQLite");
        }
        (true, false) => {
            println!("Converting SQLite -> compact JSON...");
            let dataset = Dataset::open_sqlite(&args.input).expect("Error opening SQLite");
            dataset
                .to_memory()
                .expect("Error reading SQLite")
                .save(&output, Format::Compact)
                .expect("Error saving JSON");
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
