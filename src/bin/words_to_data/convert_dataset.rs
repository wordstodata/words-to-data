//! `words_to_data convert-dataset` — convert a dataset between compact JSON and
//! SQLite, in either direction. Direction is inferred from the file extensions.

use clap::Args as ClapArgs;
use words_to_data::dataset::Dataset;

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
            let dataset = crate::fail::or_exit(
                crate::load::load_compact(&args.input),
                "Error loading dataset",
            );
            crate::fail::or_exit(crate::load::save(&dataset, &output), "Error saving SQLite");
        }
        (true, false) => {
            let dataset =
                crate::fail::or_exit(Dataset::open_sqlite(&args.input), "Error opening SQLite");
            let memory = crate::fail::or_exit(
                crate::ui::step(&format!("Read {}", args.input), || dataset.to_memory()),
                "Error reading SQLite",
            );
            // Writing JSON truncates, so this direction already replaces.
            crate::fail::or_exit(
                crate::load::save_compact(&memory, &output),
                "Error saving JSON",
            );
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
