//! `words_to_data residue` — every amendment of a public law that no standing
//! `legislature.amended_by` link names, with the stage and the reason it
//! stopped (#251). A link stands unless its newest review refutes it (#268).
//!
//! See [`words_to_data::legislature::residue`] for what counts as unlinked,
//! where the reasons come from and what each category means.
//!
//! **It stores nothing.** The list is derived each time, so an amendment leaves
//! it the moment a link names it, from the batch or through `link-amendment`.
//!
//! **It ends with the links the current method no longer makes (#185).** They
//! are review work, not resolve work, so they are a section of their own and
//! not rows. See [`words_to_data::legislature::outdated`].
//!
//! **A person sees a screenful.** The counts cover every row. The rows follow,
//! work first, and stop at [`DEFAULT_LIMIT`] with the number left out. `--json`
//! carries every row, in the order each bill states its amendments.

use std::collections::BTreeMap;

use clap::Args as ClapArgs;
use serde::Serialize;
use words_to_data::legislature::evidence_matching::Stage;
use words_to_data::legislature::outdated::{OutdatedLink, outdated_links};
use words_to_data::legislature::residue::{Category, Unlinked, unlinked_amendments};
use words_to_data::query::{Answer, DEFAULT_LIMIT};
use words_to_data::review::short_id;

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
    /// The links the current version of their batch method no longer makes.
    /// Review work, not resolve work, so apart from the rows.
    outdated: Vec<OutdatedLink>,
}

pub fn run(args: Args) {
    let ds = crate::fail::or_exit(load::open(&args.dataset), "Error opening dataset");
    let (rows, outdated) = crate::fail::or_exit(
        with_dataset!(ds, d => unlinked_amendments(&d, args.bill.as_deref())
            .and_then(|rows| Ok((rows, outdated_links(&d, args.bill.as_deref())?)))),
        "Error reading the dataset's amendments",
    );

    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&Listing { rows, outdated }).unwrap()
        );
        return;
    }

    print_counts(&rows);
    if !rows.is_empty() {
        print_queue(rows);
    }
    print_outdated(&outdated);
}

/// The rows, work first, at most a screenful.
fn print_queue(rows: Vec<Unlinked>) {
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

/// The links the current version of their method no longer makes, one line
/// each. Review work: a reviewer settles each one (Job 3 of
/// `docs/agents/working-a-dataset.md`).
fn print_outdated(outdated: &[OutdatedLink]) {
    if outdated.is_empty() {
        return;
    }
    println!(
        "\nLinks the current method no longer makes ({}): review each with settle",
        outdated.len()
    );
    for row in outdated {
        println!(
            "  link {}  {}  {} -> {}",
            short_id(&row.link_id),
            row.path,
            row.made_by,
            row.remade_by
        );
        println!("    window: {}@{}", row.work, row.window);
    }
}

/// Where a category sits in the printed queue: the work an agent can do first.
fn queue_place(category: Category) -> u8 {
    match category {
        Category::Work => 0,
        Category::Unwritten => 1,
        Category::NotHeld => 2,
        Category::Quiet => 3,
        Category::ReviewedNoLink => 4,
    }
}

/// How many rows fall in each category, and for the work, at which stage and
/// for which reason.
fn print_counts(rows: &[Unlinked]) {
    let count = |category: Category| rows.iter().filter(|row| row.category == category).count();
    println!("{} amendment(s) that no standing link names", rows.len());
    println!("  {:>4}  work", count(Category::Work));

    let mut reasons: BTreeMap<(&str, &str), usize> = BTreeMap::new();
    for row in rows.iter().filter(|row| row.category == Category::Work) {
        *reasons
            .entry((stage_name(row), row.reason.as_str()))
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

    let mut concluded: BTreeMap<String, usize> = BTreeMap::new();
    for no_link in rows.iter().filter_map(|row| row.no_link.as_ref()) {
        *concluded.entry(no_link.category.to_string()).or_default() += 1;
    }
    println!(
        "  {:>4}  reviewed: no link: a reviewer concluded the amendment has no correct link; not work",
        count(Category::ReviewedNoLink)
    );
    for (category, count) in &concluded {
        println!("          {count:>4}  {category}");
    }
}

/// One row: the id and where it stopped, then what is known about it.
fn print_row(row: &Unlinked) {
    let id = &row.amendment_id[..row.amendment_id.len().min(12)];
    let category = match (row.category, &row.no_link) {
        (Category::ReviewedNoLink, Some(no_link)) => {
            format!("reviewed: no link ({})", no_link.category)
        }
        (Category::ReviewedNoLink, None) => "reviewed: no link".to_string(),
        (Category::Work, _) => "work".to_string(),
        (Category::Unwritten, _) => "unwritten".to_string(),
        (Category::NotHeld, _) => "not held".to_string(),
        (Category::Quiet, _) => "quiet".to_string(),
    };
    println!("  {id}  [{category}] {}: {}", stage_name(row), row.reason);
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
    if let Some(no_link) = &row.no_link {
        let method = no_link
            .method
            .as_ref()
            .map_or_else(|| "no method recorded".to_string(), ToString::to_string);
        println!(
            "    concluded by {} [{method}] on {}: {}",
            no_link.reviewer,
            no_link.at.date(),
            no_link.reasoning
        );
    }
    for refuted in &row.refuted {
        println!(
            "    refuted link {} by {} on {}: {}",
            short_id(&refuted.link_id),
            refuted.reviewer,
            refuted.at.date(),
            refuted.reasoning.as_deref().unwrap_or("no reason recorded")
        );
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

/// Where the row stopped, as the output names it: the stage of the method, or
/// `review` when a refutation made the row work again.
fn stage_name(row: &Unlinked) -> &'static str {
    if row.category == Category::Work && !row.refuted.is_empty() {
        return "review";
    }
    match row.stage {
        Some(Stage::Address) => "address",
        Some(Stage::Window) => "window",
        Some(Stage::Resolve) => "resolve",
        None => "linked",
    }
}
