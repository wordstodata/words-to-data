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
            describe_presence(&report.present_in)
        }
    );

    if let (Some(from), Some(to)) = (&args.from, &args.to) {
        println!("\nChanges ({from} -> {to}) ({}):", report.changes.len());
        for c in &report.changes {
            println!("  {}: {:?} -> {:?}", c.field, c.old_value, c.new_value);
        }
    }

    println!("\nAnnotations ({}):", report.annotations.len());
    for a in &report.annotations {
        crate::annotations::print_annotation(a);
    }
}

/// Name each expression once, saying how many provisions sit at the path there.
///
/// A path can name more than one provision, so an expression can appear more
/// than once in the report. Repeating the same `work@date` reads as a bug;
/// counting it says what is actually true.
fn describe_presence(present_in: &[String]) -> String {
    let mut counted: Vec<(&str, usize)> = Vec::new();
    for id in present_in {
        match counted.last_mut() {
            Some((seen, n)) if *seen == id.as_str() => *n += 1,
            _ => counted.push((id.as_str(), 1)),
        }
    }
    counted
        .into_iter()
        .map(|(id, n)| {
            if n == 1 {
                id.to_string()
            } else {
                format!("{id} ({n} provisions)")
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}
