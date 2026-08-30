//! `words_to_data annotations` — list annotations, filtered by pair, bill, or path.

use clap::Args as ClapArgs;
use words_to_data::inspect::{self, AnnotationQuery};

use crate::load::{self, with_dataset};

#[derive(ClapArgs)]
pub struct Args {
    /// Dataset file (`.json` compact or `.sqlite`)
    pub dataset: String,

    /// Filter to a version pair (requires `--to`)
    #[arg(long, requires = "to")]
    pub from: Option<String>,

    /// Newer version date of the pair (requires `--from`)
    #[arg(long, requires = "from")]
    pub to: Option<String>,

    /// Filter to annotations sourced from this bill
    #[arg(long, conflicts_with_all = ["from", "path"])]
    pub bill: Option<String>,

    /// Filter to annotations touching this structural path
    #[arg(long, conflicts_with_all = ["from", "bill"])]
    pub path: Option<String>,

    /// Emit JSON instead of human-readable text
    #[arg(long)]
    pub json: bool,
}

pub fn run(args: Args) {
    let query = if let (Some(from), Some(to)) = (&args.from, &args.to) {
        AnnotationQuery::Pair { from, to }
    } else if let Some(bill) = &args.bill {
        AnnotationQuery::Bill(bill)
    } else if let Some(path) = &args.path {
        AnnotationQuery::Path(path)
    } else {
        eprintln!("Provide a filter: --from/--to, --bill, or --path");
        std::process::exit(2);
    };

    let ds = load::open(&args.dataset).expect("Error opening dataset");
    let anns =
        with_dataset!(ds, d => inspect::annotations(&d, query)).expect("Error reading annotations");

    if args.json {
        println!("{}", serde_json::to_string_pretty(&anns).unwrap());
        return;
    }

    for a in &anns {
        print_annotation(a);
        println!("    paths: {}", a.paths.join(", "));
    }
    println!("{} annotation(s)", anns.len());
}

/// Print one annotation's headline: status, operation, bill, short amendment id,
/// confidence, annotator, and the causative instruction snippet. Shared with the
/// `path` command so both surfaces stay consistent.
pub fn print_annotation(a: &words_to_data::inspect::AnnotationSummary) {
    let confidence = a
        .confidence
        .map(|c| format!("{c:.2}"))
        .unwrap_or_else(|| "-".to_string());
    let short_id: String = a.amendment_id.chars().take(12).collect();
    println!(
        "  [{}] {} -> {}  {} {} amd {} (conf {}, by {})",
        a.status, a.from_date, a.to_date, a.operation, a.bill_id, short_id, confidence, a.annotator
    );
    let text = a.causative_text.trim();
    if !text.is_empty() {
        println!("      {}", truncate(text, 100));
    }
}

/// Shorten `text` to `max` characters, adding an ellipsis when clipped.
fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let clipped: String = text.chars().take(max).collect();
    format!("{clipped}…")
}
