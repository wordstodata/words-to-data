//! `words_to_data votes` — how the House voted on a bill, party by party.
//!
//! Each party is the one the member held on the day of the roll call, not the
//! one they hold now. A vote whose party the date cannot settle is listed on its
//! own rather than counted into a party, because the party history carries years
//! and a change year belongs to two parties (#105).

use clap::Args as ClapArgs;
use words_to_data::inspect::{self, RollCallTally};

use crate::load::{self, with_dataset};

#[derive(ClapArgs)]
pub struct Args {
    /// Dataset file (`.json` compact or `.sqlite`)
    pub dataset: String,

    /// Bill identifier (e.g. `119-hr-1`)
    pub bill_id: String,

    /// Emit JSON instead of human-readable text
    #[arg(long)]
    pub json: bool,
}

pub fn run(args: Args) {
    let ds = crate::fail::or_exit(load::open(&args.dataset), "Error opening dataset");
    let tallies = crate::fail::or_exit(
        with_dataset!(ds, d => inspect::votes(&d, &args.bill_id)),
        "Error reading votes",
    );

    let Some(tallies) = tallies else {
        eprintln!("No votes for bill: {}", args.bill_id);
        std::process::exit(1);
    };

    if args.json {
        println!("{}", serde_json::to_string_pretty(&tallies).unwrap());
        return;
    }

    println!("Bill: {}", args.bill_id);
    for tally in &tallies {
        print_tally(tally);
    }
    if tallies.is_empty() {
        println!("\nThe dataset holds no roll call for this bill.");
    }
}

fn print_tally(tally: &RollCallTally) {
    println!(
        "\nRoll call {} on {} — {}",
        tally.roll_number, tally.date, tally.result
    );
    println!("  {}", tally.question);

    for row in &tally.by_party {
        println!(
            "  {:<12} {:<10} {:>4}",
            row.party.to_string(),
            row.position.to_string(),
            row.count
        );
    }

    if tally.unresolved.is_empty() {
        return;
    }

    // These votes are in no party column above. Saying so is the point: the
    // alternative is a tally that silently does not add up to the roll call.
    println!(
        "  {} vote(s) with no party for this date:",
        tally.unresolved.len()
    );
    for vote in &tally.unresolved {
        let name = vote.name.as_deref().unwrap_or("(member not in dataset)");
        println!(
            "    {:<10} {:<24} {:<10} {}",
            vote.bioguide_id,
            name,
            vote.position.to_string(),
            vote.party
        );
    }
}
