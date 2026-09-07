//! `words_to_data path` — everything about one structural path: where it
//! exists, its field-level changes for an expression pair, and its annotations.

use clap::Args as ClapArgs;
use words_to_data::dataset::ExpressionId;
use words_to_data::inspect;

use crate::load::{self, with_dataset};

#[derive(ClapArgs)]
pub struct Args {
    /// Dataset file (`.json` compact or `.sqlite`)
    pub dataset: String,

    /// Structural path to inspect (e.g. `uscode/title_9/chapter_1/section_1`)
    pub path: String,

    /// Older expression for field changes, e.g. `uscode/title_9@2025-07-18` (requires `--to`)
    #[arg(long, requires = "to")]
    pub from: Option<ExpressionId>,

    /// Newer expression of the same work (requires `--from`)
    #[arg(long, requires = "from")]
    pub to: Option<ExpressionId>,

    /// Emit JSON instead of human-readable text
    #[arg(long)]
    pub json: bool,
}

pub fn run(args: Args) {
    let pair = match (&args.from, &args.to) {
        (Some(from), Some(to)) => Some((from, to)),
        _ => None,
    };

    let ds = crate::fail::or_exit(load::open(&args.dataset), "Error opening dataset");
    let report = crate::fail::or_exit(
        with_dataset!(ds, d => inspect::path_report(&d, &args.path, pair)),
        "Error building path report",
    );

    if args.json {
        println!("{}", serde_json::to_string_pretty(&report).unwrap());
        return;
    }

    println!("PATH: {}", report.path);
    println!(
        "Present in: {}",
        if report.present_in.is_empty() {
            "(no expression)".to_string()
        } else {
            report
                .present_in
                .iter()
                .map(|p| format!("{} ({})", p.expression, provisions(p.provisions)))
                .collect::<Vec<_>>()
                .join(", ")
        }
    );

    if let (Some(from), Some(to)) = (&args.from, &args.to) {
        println!(
            "\nProvisions ({from} -> {to}) ({}):",
            report.provisions.len()
        );
        for p in &report.provisions {
            println!("  {} — {}", verdict(p.presence), positions(p));
            for c in &p.changes {
                println!("      {}: {:?} -> {:?}", c.field, c.old_value, c.new_value);
            }
        }
    }

    println!("\nAnnotations ({}):", report.annotations.len());
    for a in &report.annotations {
        crate::annotations::print_annotation(a);
    }
}

fn provisions(n: usize) -> String {
    if n == 1 {
        "1 provision".to_string()
    } else {
        format!("{n} provisions")
    }
}

fn verdict(presence: inspect::Presence) -> &'static str {
    match presence {
        inspect::Presence::InBoth => "in both",
        inspect::Presence::Added => "added",
        inspect::Presence::Removed => "removed",
    }
}

/// Where the provision sits on each side, using the same indices as `--json`.
///
/// A provision that shares its path with another is only addressable by
/// position, so the position is printed rather than left to the reader to
/// count off the list.
fn positions(p: &inspect::ProvisionAtPath) -> String {
    match (p.from_position, p.to_position) {
        (Some(from), Some(to)) => format!("from position {from}, to position {to}"),
        (Some(from), None) => format!("was at position {from}"),
        (None, Some(to)) => format!("now at position {to}"),
        (None, None) => "no position".to_string(),
    }
}
