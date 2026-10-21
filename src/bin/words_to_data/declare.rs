//! `words_to_data declare` — set the declaration of a dataset that is already
//! there (#293).
//!
//! Until this existed only `build-dataset --declaration` could set one, so a
//! dataset built without a declaration could never tell "out of scope" from
//! "not found". The declaration is part of the metadata, so this changes no
//! stored shape.

use clap::Args as ClapArgs;
use words_to_data::dataset::{Dataset, Declaration};
use words_to_data::storage::Storage;

#[derive(ClapArgs)]
pub struct Args {
    /// Path to the dataset to declare (compact JSON or SQLite)
    pub dataset: String,

    /// JSON file declaring what this dataset is meant to cover
    pub declaration: String,

    /// Where to write a compact JSON dataset. Required for compact JSON, which
    /// is never written back over its input. Ignored for SQLite, which changes
    /// in place.
    #[arg(long)]
    pub output: Option<String>,
}

/// Read a declaration file, or stop the run.
///
/// A declaration that will not parse is fatal rather than skipped. A dataset
/// that silently declares nothing is how a gap goes unreported, which is the
/// failure a declaration exists to prevent. `build-dataset --declaration` reads
/// with this too, so the two commands accept the same files.
pub fn read(path: &str) -> Declaration {
    let text = crate::fail::or_exit(std::fs::read_to_string(path), "Error reading declaration");
    crate::fail::or_exit(serde_json::from_str(&text), "Error parsing declaration")
}

pub fn run(args: Args) {
    let declaration = read(&args.declaration);

    if crate::load::is_sqlite(&args.dataset) {
        let mut dataset =
            crate::fail::or_exit(Dataset::open_sqlite(&args.dataset), "Error opening dataset");
        set(&mut dataset, declaration);
        drop(dataset);
        println!("Wrote {}", args.dataset);
        // Read back from the file, so the lines printed are what it holds.
        // `set_metadata` does not report a failed write, and this read does.
        let written =
            crate::fail::or_exit(Dataset::open_sqlite(&args.dataset), "Error opening dataset");
        report(&written);
    } else {
        let output =
            crate::load::output_or_refuse(&args.dataset, args.output.as_deref(), "declare");
        let mut dataset = crate::fail::or_exit(
            crate::load::load_compact(&args.dataset),
            "Error loading dataset",
        );
        set(&mut dataset, declaration);
        crate::fail::or_exit(
            crate::load::save_compact(&dataset, output),
            "Error saving dataset",
        );
        println!("Wrote {output}");
        report(&dataset);
    }
}

/// Print the declaration and any gap, as `info` prints them.
fn report<S: Storage>(dataset: &Dataset<S>) {
    let scope = crate::fail::or_exit(dataset.scope(), "Error reading the dataset's scope");
    crate::info::print_declared(&scope);
}

/// Put the declaration in the dataset's metadata, in place of any it had.
///
/// A declaration someone made is a statement, so the run says when it replaces
/// one rather than let it go without a word.
fn set<S: Storage>(dataset: &mut Dataset<S>, declaration: Declaration) {
    let mut metadata = dataset.metadata().clone();
    if metadata.declaration.replace(declaration).is_some() {
        eprintln!("Replaced the declaration this dataset had.");
    }
    dataset.set_metadata(metadata);
}
