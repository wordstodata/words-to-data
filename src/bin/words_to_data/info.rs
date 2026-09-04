//! `words_to_data info` — print a dataset's metadata and headline counts.

use clap::Args as ClapArgs;
use words_to_data::inspect;

use crate::load::{self, with_dataset};

/// How many coverage units the human output lists before it summarizes.
const SCOPE_SAMPLE: usize = 5;

#[derive(ClapArgs)]
pub struct Args {
    /// Dataset file (`.json` compact or `.sqlite`)
    pub dataset: String,

    /// Emit JSON instead of human-readable text
    #[arg(long)]
    pub json: bool,
}

pub fn run(args: Args) {
    let ds = load::open(&args.dataset).expect("Error opening dataset");
    let info = with_dataset!(ds, d => inspect::info(&d)).expect("Error reading dataset");

    if args.json {
        println!("{}", serde_json::to_string_pretty(&info).unwrap());
        return;
    }

    println!("Name:        {}", info.name);
    println!("Description: {}", info.description);
    println!("Author:      {}", info.author);
    println!("License:     {}", info.license);
    println!("Version:     {}", info.version);
    if !info.source_urls.is_empty() {
        println!("Sources:     {}", info.source_urls.join(", "));
    }
    println!("Versions:    {}", info.version_count);
    println!("Bills:       {}", info.bill_count);

    // Scope is the answer to "why did my query find nothing". Print it, so a
    // reader of this dataset knows what it does not hold.
    match info.scope.held.len() {
        0 => println!("Covers:      nothing"),
        // A full US Code dataset holds 60+ units, which is a wall of text on
        // one line. Show a sample and the count; `--json` carries them all.
        count if count > SCOPE_SAMPLE => println!(
            "Covers:      {} units, including {}",
            count,
            info.scope.held[..SCOPE_SAMPLE].join(", ")
        ),
        _ => println!("Covers:      {}", info.scope.held.join(", ")),
    }
}
