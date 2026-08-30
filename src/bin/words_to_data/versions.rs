//! `words_to_data versions` — list every version snapshot with its size.

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
    let ds = load::open(&args.dataset).expect("Error opening dataset");
    let versions = with_dataset!(ds, d => inspect::versions(&d)).expect("Error reading versions");

    if args.json {
        println!("{}", serde_json::to_string_pretty(&versions).unwrap());
        return;
    }

    println!("{:<12}  {:>10}  LABEL", "DATE", "ELEMENTS");
    for v in &versions {
        println!(
            "{:<12}  {:>10}  {}",
            v.date,
            v.element_count,
            v.label.as_deref().unwrap_or("")
        );
    }
    println!("{} version(s)", versions.len());
}
