//! `words_to_data opinions` — list the court opinions a dataset holds (#308).

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

    println!(
        "This dataset holds {total} {}.",
        if total == 1 { "opinion" } else { "opinions" }
    );
    for row in shown {
        print_row(row);
    }
    // What was left out, said rather than left to be inferred (#220, #235).
    let dropped = total - shown.len();
    if dropped > 0 {
        println!("  … {dropped} more not shown; pass --all for every one of them");
    }
}

/// One opinion on two lines: what to type, and what it is.
///
/// Every fact is named, and a missing one is said to be missing, so a reader
/// does not take a blank for an answer.
fn print_row(row: &HeldOpinion) {
    const NOT_RECORDED: &str = "not recorded";
    println!(
        "{}@{}  {}",
        row.work,
        row.date,
        row.case_name
            .as_deref()
            .unwrap_or("(case name not recorded)")
    );
    println!(
        "    cluster {} · {} · author {} · text from {} · court {} · {} citation link(s)",
        row.cluster_id
            .map_or_else(|| NOT_RECORDED.to_string(), |id| id.to_string()),
        row.opinion_type
            .as_deref()
            .unwrap_or("writing type not recorded"),
        row.author.as_deref().unwrap_or(NOT_RECORDED),
        row.text_field.as_deref().unwrap_or(NOT_RECORDED),
        row.court.as_deref().unwrap_or(NOT_RECORDED),
        row.cites,
    );
}
