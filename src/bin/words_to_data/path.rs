//! `words_to_data path` — everything about one structural path: where it
//! exists, its field-level changes for a version pair, and its annotations.

use clap::Args as ClapArgs;
use words_to_data::inspect;

use crate::load::{self, with_dataset};

#[derive(ClapArgs)]
pub struct Args {
    /// Dataset file (`.json` compact or `.sqlite`)
    pub dataset: String,

    /// Structural path to inspect (e.g. `uscode/title_9/chapter_1/section_1`)
    pub path: String,

    /// Older version date for field changes (requires `--to`)
    #[arg(long, requires = "to")]
    pub from: Option<String>,

    /// Newer version date for field changes (requires `--from`)
    #[arg(long, requires = "from")]
    pub to: Option<String>,

    /// Emit JSON instead of human-readable text
    #[arg(long)]
    pub json: bool,
}

pub fn run(args: Args) {
    let pair = match (&args.from, &args.to) {
        (Some(from), Some(to)) => Some((from.as_str(), to.as_str())),
        _ => None,
    };

    let ds = load::open(&args.dataset).expect("Error opening dataset");
    let report = with_dataset!(ds, d => inspect::path_report(&d, &args.path, pair))
        .expect("Error building path report");

    if args.json {
        println!("{}", serde_json::to_string_pretty(&report).unwrap());
        return;
    }

    println!("PATH: {}", report.path);
    println!(
        "Present in: {}",
        if report.present_in.is_empty() {
            "(no version)".to_string()
        } else {
            report.present_in.join(", ")
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
