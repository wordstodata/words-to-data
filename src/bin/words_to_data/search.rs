//! `words_to_data search` — full-text search across every version.

use clap::Args as ClapArgs;
use words_to_data::inspect;

use crate::load::{self, with_dataset};

#[derive(ClapArgs)]
pub struct Args {
    /// Dataset file (`.json` compact or `.sqlite`)
    pub dataset: String,

    /// Text to search for (case-insensitive)
    pub query: String,

    /// Emit JSON instead of human-readable text
    #[arg(long)]
    pub json: bool,
}

pub fn run(args: Args) {
    let ds = crate::fail::or_exit(load::open(&args.dataset), "Error opening dataset");
    let hits = crate::fail::or_exit(
        with_dataset!(ds, d => inspect::search(&d, &args.query)),
        "Error searching dataset",
    );

    if args.json {
        println!("{}", serde_json::to_string_pretty(&hits).unwrap());
        return;
    }

    for h in &hits {
        println!("{}  {}  [{}]", h.expression, h.path, h.field);
        println!("    {}", h.snippet.trim());
    }
    println!("{} match(es)", hits.len());
}
