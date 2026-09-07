//! `words_to_data bills` — list every bill a dataset holds.
//!
//! `show-bill` takes an id, and until this existed nothing would tell you what
//! ids there were: `info` reports a count, and annotations carry ids but a
//! dataset has none until `match-amendments` has run (#83).

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
    let bills = crate::fail::or_exit(
        with_dataset!(ds, d => inspect::bills(&d)),
        "Error reading bills",
    );

    if args.json {
        println!("{}", serde_json::to_string_pretty(&bills).unwrap());
        return;
    }

    // The id is the column that goes straight back in to `show-bill`.
    let width = bills
        .iter()
        .map(|b| b.bill_id.len())
        .max()
        .unwrap_or(0)
        .max("BILL".len());

    println!(
        "{:<width$}  {:>10}  {:>12}",
        "BILL", "AMENDMENTS", "WITH CHANGES"
    );
    for b in &bills {
        println!(
            "{:<width$}  {:>10}  {:>12}",
            b.bill_id, b.amendment_count, b.amendments_with_changes
        );
    }
    println!("{} bill(s)", bills.len());

    // Scoring and matching both read the extracted changes, so a dataset where
    // every bill reports zero is one where `extract-changes` has not run.
    if !bills.is_empty() && bills.iter().all(|b| b.amendments_with_changes == 0) {
        println!("\nNo amendment carries extracted changes yet. Run `extract-changes` first.");
    }
}
