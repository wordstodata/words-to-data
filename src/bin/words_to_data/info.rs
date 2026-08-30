//! `words_to_data info` — print a dataset's metadata and headline counts.

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
}
