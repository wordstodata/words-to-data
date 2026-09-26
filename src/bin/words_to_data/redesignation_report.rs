//! `words_to_data redesignation-report` — every renumbering a dataset's bills
//! state, and what became of each.
//!
//! One row for each link, and one row for each statement no reader could place.
//! The weakest claims come first: the statements nothing placed at all, then the
//! placed ones from the least corroborated upwards. That is the queue
//! `docs/adr/0010-two-readers-one-resolver-a-model-never-writes-a-path.md` asks
//! for, with the unplaced statements in front of it.
//!
//! **It reads the dataset and nothing else.** No XML, and no model call. The
//! dataset holds the bill as a document (#196), so the words and the path come
//! back out of it, and resolving them against the windows the dataset holds
//! gives the rest. Nothing is stored beside the links: the same bill leaves 31
//! statements unplaced against title 26 alone and 17 against the whole corpus,
//! so a stored row would go stale as soon as `add-release-points` ran (#180).
//!
//! **It is read-only.** `redesignations` writes the links; this reports them.

use clap::Args as ClapArgs;
use words_to_data::inspect;
use words_to_data::legislature::redesignation::clause_start;

use crate::load::{self, with_dataset};

/// How many rows the human output prints before it stops.
///
/// `--json` carries them all. A corpus of bills gives hundreds of placed rows,
/// and the ones a reviewer acts on are at the top.
const SHOWN: usize = 20;

#[derive(ClapArgs)]
pub struct Args {
    /// Dataset file (`.json` compact or `.sqlite`)
    pub dataset: String,

    /// One bill, as the dataset names it, such as `119-hr-1`
    ///
    /// Every bill the dataset holds, when this is left out.
    #[arg(long)]
    pub bill_id: Option<String>,

    /// Emit JSON instead of human-readable text
    #[arg(long)]
    pub json: bool,
}

pub fn run(args: Args) {
    let ds = crate::fail::or_exit(load::open(&args.dataset), "Error opening dataset");
    let report = crate::fail::or_exit(
        with_dataset!(ds, d => inspect::redesignation_report(&d, args.bill_id.as_deref())),
        "Error reading the dataset's redesignations",
    );

    if args.json {
        println!("{}", serde_json::to_string_pretty(&report).unwrap());
        return;
    }

    // Three numbers that measure three different things, so a reader must read
    // them all and add none of them (#166). One clause can state fourteen
    // renumberings.
    println!("Bills:       {}", report.totals.bills);
    println!("Statements:  {}", report.totals.statements);
    println!("Links:       {}", report.totals.links);
    println!("Not placed:  {}", report.totals.unplaced);

    if !report.totals.reasons.is_empty() {
        println!("\nWhy a statement was not placed:");
        for (reason, count) in &report.totals.reasons {
            println!("  {count:>4}  {reason}");
        }
    }

    if report.rows.is_empty() {
        println!("\nThis dataset holds no bill that states a renumbering.");
        return;
    }

    println!("\nWeakest first:");
    for row in report.rows.iter().take(SHOWN) {
        // The id leads a placed row, because this list is the review queue and
        // the id is the one field a reviewer copies out of it. A row nothing
        // placed has no link and so prints no id field at all — an empty column
        // would read as a link whose id is missing (#227).
        let named = match &row.id {
            Some(id) => format!("{id}  "),
            None => String::new(),
        };
        match (&row.reason, row.corroboration) {
            (Some(reason), _) => println!("  {named}[{}] not placed — {reason}", row.reader),
            (None, Some(figure)) => {
                println!(
                    "  {named}[{}] placed, corroboration {figure:.2}",
                    row.reader
                )
            }
            (None, None) => println!("  {named}[{}] placed", row.reader),
        }
        println!(
            "    {} {}",
            row.bill_id,
            row.bill_path.as_deref().unwrap_or("")
        );
        if let (Some(from), Some(to)) = (&row.from_path, &row.to_path) {
            println!("    {from}");
            println!(" -> {to}");
        }
        // The window the link came from. Two links for one statement are
        // otherwise two rows that differ only by their score, and a reader
        // cannot see which window made either (#184).
        if let Some(window) = &row.window {
            println!("    in {window}");
        }
        println!("    {}", clause_start(&row.clause));
    }
    if report.rows.len() > SHOWN {
        println!(
            "  … {} more row(s); pass --json for all of them",
            report.rows.len() - SHOWN
        );
    }
}
