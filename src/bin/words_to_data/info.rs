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

    // The legislature block prints whole or not at all. Zeroes here say "this
    // dataset speaks legislature and holds none of it", and no block at all
    // says "legislature is not a concept here", which is what a dataset of
    // court opinions holds. A printed zero could never say the second (#133).
    if let Some(counts) = &info.legislature {
        for (label, count) in [
            ("Bills:", counts.bills),
            ("Members:", counts.members),
            ("Sponsors:", counts.sponsors),
            ("Roll calls:", counts.roll_calls),
            ("Votes:", counts.member_votes),
        ] {
            println!("{label:<13}{count}");
        }
    }

    // Links are what this project produces; the document text is the input. The
    // breakdown names each kind in full, namespace included, because a reader
    // that does not know a namespace can still report it, and a total hides it
    // (`docs/adr/0002-links-live-in-the-core.md`).
    if info.link_count > 0 {
        println!("Links:       {}", info.link_count);
        for (kind, count) in &info.link_counts_by_kind {
            println!("  {kind}  {count}");
        }
    }

    // Printed only where there is something to report. A dataset that recorded
    // no model replies holds none, and a zero there reads as a tool that
    // measured nothing rather than a dataset that holds nothing.
    if info.reply_count > 0 {
        println!("Replies:     {}", info.reply_count);
    }

    // One line, and no detail. A reader must be able to see that the corpus
    // said something this build could not place, because otherwise the tool's
    // silence reads as the corpus's silence (#153). The rows live in
    // `redesignation-report`.
    //
    // The three numbers measure three different things and are never added
    // (#166): one clause can state fourteen renumberings.
    let renumbering = &info.redesignations;
    if !renumbering.is_silent() {
        println!(
            "Renumbering: {} statement(s), {} link(s), {} not placed",
            renumbering.statements, renumbering.links, renumbering.unplaced
        );
    }

    // What has been done to this dataset, so a missing step does not read as a
    // complete file (#182). The method is named, not the command: a command
    // keeps its name while the reasoning under it changes. Printed only where
    // there is something to report, on the same rule as the counts above — an
    // empty list means nothing was recorded, not that nothing ran.
    if !info.method_runs.is_empty() {
        println!("Methods run:");
        for run in &info.method_runs {
            println!(
                "  {}  {} {} -> {}",
                run.method, run.work, run.from_date, run.to_date
            );
        }
    }

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
