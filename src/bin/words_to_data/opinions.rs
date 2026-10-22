//! `words_to_data opinions` — list the court opinions a dataset holds, one row
//! for each writing, under its case (#308, #313).

use clap::Args as ClapArgs;
use words_to_data::judicial::held::{self, HeldOpinion};
use words_to_data::query::DEFAULT_LIMIT;

use crate::load::{self, with_dataset};

#[derive(ClapArgs)]
pub struct Args {
    /// Dataset file (`.json` compact or `.sqlite`)
    pub dataset: String,

    /// How many to print before stopping. The run says how many it did not show
    ///
    /// Human output stops at 20 unless told otherwise. `--json` carries them all
    /// unless this is given explicitly
    #[arg(long)]
    pub limit: Option<usize>,

    /// Print every opinion, however many there are
    #[arg(long, conflicts_with = "limit")]
    pub all: bool,

    /// Emit JSON instead of human-readable text
    #[arg(long)]
    pub json: bool,
}

pub fn run(args: Args) {
    let ds = crate::fail::or_exit(load::open(&args.dataset), "Error opening dataset");
    let rows = crate::fail::or_exit(
        with_dataset!(ds, d => held::held_opinions(&d)),
        "Error reading opinions",
    );

    // A screenful for a person, everything for a machine, as `annotations`
    // does. An explicit `--limit` is honoured either way, and `--all` lifts it.
    let limit = match (args.all, args.limit, args.json) {
        (true, _, _) | (_, None, true) => rows.len(),
        (_, Some(explicit), _) => explicit,
        (_, None, false) => DEFAULT_LIMIT,
    };
    let total = rows.len();
    let shown = &rows[..limit.min(total)];

    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "opinions": shown,
                "total": total,
            }))
            .unwrap()
        );
        return;
    }

    let cases: std::collections::BTreeSet<&str> =
        rows.iter().map(|row| row.case.as_str()).collect();
    println!(
        "This dataset holds {total} {} in {} {}.",
        if total == 1 { "writing" } else { "writings" },
        cases.len(),
        if cases.len() == 1 { "case" } else { "cases" },
    );
    let mut case_shown: Option<&str> = None;
    for row in shown {
        if case_shown != Some(row.case.as_str()) {
            print_case(row);
            case_shown = Some(row.case.as_str());
        }
        print_row(row);
    }
    // What was left out, said rather than left to be inferred (#220, #235).
    let dropped = total - shown.len();
    if dropped > 0 {
        println!("  … {dropped} more not shown; pass --all for every one of them");
    }
}

const NOT_RECORDED: &str = "not recorded";

/// The case, once, above its writings: what to type, what it is, and what of
/// it is not stored as text.
///
/// Every fact is named, and a missing one is said to be missing, so a reader
/// does not take a blank for an answer.
fn print_case(row: &HeldOpinion) {
    println!(
        "{}@{}  {}",
        row.case,
        row.date,
        row.case_name
            .as_deref()
            .unwrap_or("(case name not recorded)")
    );
    let sources = match row.combined_records.as_slice() {
        [] => String::new(),
        ids => format!(
            " · combined record {} kept as a source, not as text",
            ids.iter()
                .map(u64::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ),
    };
    println!(
        "    cluster {} · court {}{sources}",
        row.cluster_id
            .map_or_else(|| NOT_RECORDED.to_string(), |id| id.to_string()),
        row.court.as_deref().unwrap_or(NOT_RECORDED),
    );
}

/// One writing on one line: its path, and what it is.
fn print_row(row: &HeldOpinion) {
    println!(
        "  {}  {} · author {} · text from {} · {} citation link(s)",
        row.path,
        row.opinion_type
            .as_deref()
            .unwrap_or("writing type not recorded"),
        row.author.as_deref().unwrap_or(NOT_RECORDED),
        row.text_field.as_deref().unwrap_or(NOT_RECORDED),
        row.cites,
    );
}
