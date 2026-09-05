//! `words_to_data show-bill` — print a bill's amendments.

use clap::Args as ClapArgs;
use words_to_data::inspect;

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
    let summary = crate::fail::or_exit(
        with_dataset!(ds, d => inspect::show_bill(&d, &args.bill_id)),
        "Error reading bill",
    );

    let Some(summary) = summary else {
        eprintln!("Bill not found: {}", args.bill_id);
        std::process::exit(1);
    };

    if args.json {
        println!("{}", serde_json::to_string_pretty(&summary).unwrap());
        return;
    }

    println!("Bill:       {}", summary.bill_id);
    println!("Amendments: {}", summary.amendment_count);
    for a in &summary.amendments {
        let actions = a.action_types.join(", ");
        println!("\n- {} [{}] ({} change(s))", a.id, actions, a.change_count);
        println!("    {}", truncate(&a.amending_text, 200));
    }
}

/// Shorten `text` to `max` characters, adding an ellipsis when clipped.
fn truncate(text: &str, max: usize) -> String {
    let text = text.trim();
    if text.chars().count() <= max {
        return text.to_string();
    }
    let clipped: String = text.chars().take(max).collect();
    format!("{clipped}…")
}
