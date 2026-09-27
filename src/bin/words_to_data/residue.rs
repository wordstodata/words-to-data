//! `words_to_data residue` — every amendment of a public law that no
//! `legislature.amended_by` link names, with the stage and the reason it
//! stopped (#251).
//!
//! See [`words_to_data::legislature::residue`] for what counts as unlinked,
//! where the reasons come from and what each category means.
//!
//! **It stores nothing.** The list is derived each time, so an amendment leaves
//! it the moment a link names it, from the batch or through `link-amendment`.
//!
//! **A person sees a screenful.** The counts cover every row. The rows follow,
//! work first, and stop at [`DEFAULT_LIMIT`] with the number left out. `--json`
//! carries every row, in the order each bill states its amendments.

use std::collections::BTreeMap;

use clap::Args as ClapArgs;
use serde::Serialize;
use words_to_data::legislature::evidence_matching::Stage;
use words_to_data::legislature::residue::{Category, Unlinked, unlinked_amendments};
use words_to_data::query::{Answer, DEFAULT_LIMIT};

use crate::load::{self, with_dataset};

#[derive(ClapArgs)]
pub struct Args {
    /// Dataset file (`.json` compact or `.sqlite`)
    pub dataset: String,

    /// One bill, as the dataset names it, such as `119-hr-1`
    ///
    /// Every public law the dataset holds, when this is left out.
    #[arg(long)]
    pub bill: Option<String>,

    /// Emit JSON instead of human-readable text
    #[arg(long)]
    pub json: bool,
}

/// What `--json` prints.
#[derive(Serialize)]
struct Listing {
    rows: Vec<Unlinked>,
}

pub fn run(args: Args) {
    let ds = crate::fail::or_exit(load::open(&args.dataset), "Error opening dataset");
    let rows = crate::fail::or_exit(
        with_dataset!(ds, d => unlinked_amendments(&d, args.bill.as_deref())),
        "Error reading the dataset's amendments",
    );

    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&Listing { rows }).unwrap()
        );
        return;
    }

    print_counts(&rows);
    if rows.is_empty() {
        return;
    }

    let mut queue = rows;
    queue.sort_by_key(|row| queue_place(row.category));
    let total = queue.len();
    queue.truncate(DEFAULT_LIMIT);
    let shown = Answer { rows: queue, total };

    println!("\nWork first:");
    for row in &shown.rows {
        print_row(row);
    }
    if shown.dropped() > 0 {
        println!(
            "  … {} more row(s) not shown; pass --json for all of them",
            shown.dropped()
        );
    }
}

/// Where a category sits in the printed queue: the work an agent can do first.
fn queue_place(category: Category) -> u8 {
    match category {
        Category::Work => 0,
        Category::Unwritten => 1,
        Category::NotHeld => 2,
        Category::Quiet => 3,
    }
}

/// How many rows fall in each category, and for the work, at which stage and
/// for which reason.
fn print_counts(rows: &[Unlinked]) {
    let count = |category: Category| rows.iter().filter(|row| row.category == category).count();
    println!("{} amendment(s) that no link names", rows.len());
    println!("  {:>4}  work", count(Category::Work));

    let mut reasons: BTreeMap<(&str, &str), usize> = BTreeMap::new();
    for row in rows.iter().filter(|row| row.category == Category::Work) {
        *reasons
            .entry((stage_name(row.stage), row.reason.as_str()))
            .or_default() += 1;
    }
    for ((stage, reason), count) in &reasons {
        println!("          {count:>4}  {stage}: {reason}");
    }

    println!(
        "  {:>4}  linked by the evidence method, and not written: run link-by-evidence",
        count(Category::Unwritten)
    );
    println!(
        "  {:>4}  not held: the amendment changes something the dataset does not hold",
        count(Category::NotHeld)
    );
    println!(
        "  {:>4}  quiet: nothing under the address changed after the law's enactment; not work",
        count(Category::Quiet)
    );
}

/// One row: the id and where it stopped, then what is known about it.
fn print_row(row: &Unlinked) {
    let id = &row.amendment_id[..row.amendment_id.len().min(12)];
    let category = match row.category {
        Category::Work => "work",
        Category::Unwritten => "unwritten",
        Category::NotHeld => "not held",
        Category::Quiet => "quiet",
    };
    println!(
        "  {id}  [{category}] {}: {}",
        stage_name(row.stage),
        row.reason
    );
    println!(
        "    {} § {}",
        row.public_law,
        row.law_section.as_deref().unwrap_or("?")
    );
    if let Some(section) = &row.address.section {
        let below: String = row
            .address
            .container
            .iter()
            .map(|step| format!("({})", step.number))
            .collect();
        println!("    address: {section}{below}");
    }
    if let Some(not_held) = &row.not_held {
        println!("    not held: {not_held}");
    }
    if let (Some(from), Some(to)) = (&row.from, &row.to) {
        println!("    window: {from} -> {}", to.at);
    }
    for change in &row.changes {
        println!("    change: {change}");
    }
    for classified in &row.olrc {
        let descriptions: Vec<&str> = classified
            .descriptions
            .iter()
            .map(|description| match description.as_str() {
                "" => "amended",
                other => other,
            })
            .collect();
        println!(
            "    OLRC: § {} -> {} ({})",
            classified.law_section,
            classified.code_section,
            descriptions.join(", ")
        );
    }
}

/// The stage, as the output names it.
fn stage_name(stage: Option<Stage>) -> &'static str {
    match stage {
        Some(Stage::Address) => "address",
        Some(Stage::Window) => "window",
        Some(Stage::Resolve) => "resolve",
        None => "linked",
    }
}
