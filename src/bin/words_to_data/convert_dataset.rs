//! `words_to_data convert-dataset` — convert a JSON dataset to SQLite.

use clap::Args as ClapArgs;
use words_to_data::dataset::{Dataset, Format};

#[derive(ClapArgs)]
pub struct Args {
    /// Input dataset file (`.json`); output is written alongside as `.sqlite`
    pub input: String,
}

pub fn run(args: Args) {
    let file = &args.input;
    if !file.ends_with(".json") {
        eprintln!("Expected a .json dataset, got: {file}");
        return;
    }

    println!("Serialized JSON detected, loading...");
    let dataset = Dataset::load(file, Format::Compact).expect("Error loading dataset");
    println!("Saving to sqlite");
    dataset
        .save_to_sqlite(file.replace(".json", ".sqlite"))
        .expect("Error saving to sqlite");
    println!("{}", file.replace(".json", ".sqlite"));
}
