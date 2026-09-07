//! `words_to_data info` — print a dataset's metadata and headline counts.

use clap::Args as ClapArgs;
use words_to_data::inspect;

use crate::load::{self, with_dataset};

/// How many works the human output lists before it summarizes.
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
    let ds = crate::fail::or_exit(load::open(&args.dataset), "Error opening dataset");
    let info = crate::fail::or_exit(
        with_dataset!(ds, d => inspect::info(&d)),
        "Error reading dataset",
    );

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
    println!("Works:       {}", info.work_count);
    println!("Expressions: {}", info.expression_count);
    println!("Bills:       {}", info.bill_count);

    // Scope is the answer to "why did my query find nothing". Print it, so a
    // reader of this dataset knows what it does not hold. Each work carries its
    // own dates, because a dataset need not hold every work on every date.
    match info.scope.held.len() {
        0 => println!("Covers:      nothing"),
        // A full US Code dataset holds 60+ works, which is a wall of text.
        // Show a sample and the count; `--json` carries them all.
        count if count > SCOPE_SAMPLE => {
            println!("Covers:      {count} works, including");
            for held in &info.scope.held[..SCOPE_SAMPLE] {
                println!("  {}  {}", held.work, held.dates.join(", "));
            }
        }
        _ => {
            println!("Covers:");
            for held in &info.scope.held {
                println!("  {}  {}", held.work, held.dates.join(", "));
            }
        }
    }

    // A dataset that declared nothing prints exactly what it always printed.
    let Some(declared) = &info.scope.declared else {
        return;
    };

    if !declared.intends.is_empty() {
        println!("Declared:    {}", declared.intends.join(", "));
    }
    if let Some(dates) = &declared.dates {
        println!("Dates:       {} to {}", dates.from, dates.to);
    }
    if !declared.namespaces.is_empty() {
        println!("Namespaces:  {}", declared.namespaces.join(", "));
    }

    // A stated hole is not a fault, but a reader seeing "title 26" has to know
    // section 174 is deliberately absent, and why.
    for hole in &declared.excludes {
        println!("Excluded:    {} — {}", hole.path, hole.reason);
    }

    // Only printed when there is one. A gap means the build did not do what it
    // said it would, which is the one thing here a reader must not miss.
    let gaps = info.scope.gaps();
    if !gaps.is_empty() {
        println!("INCOMPLETE:  declared but not held: {}", gaps.join(", "));
    }
}
