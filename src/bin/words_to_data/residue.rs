//! `words_to_data residue` — every amendment of a public law that no standing
//! `legislature.amended_by` link names, with the stage and the reason it
//! stopped (#251). A link stands unless its newest review refutes it (#268).
//! With `--partial`, it also lists each amendment that is linked in part: a
//! standing link names it, and a change under its address is claimed by no
//! link (#271). The default list leaves these out, because it is the work the
//! playbook gives an agent.
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
//! **The counts are of amendments (#469).** An amendment that acts on many
//! sections has one row for each item (#434), so a count gives its rows too
//! when they are more, and the amendments whose rows fall in two categories are
//! named.
//!
//! **A person sees a screenful.** The counts cover every row. The rows follow,
//! work first, and stop at [`DEFAULT_LIMIT`] with the number left out. `--json`
//! carries every row, in the order each bill states its amendments.

use std::collections::{BTreeMap, BTreeSet};

use clap::Args as ClapArgs;
use serde::Serialize;
use words_to_data::legislature::evidence_matching::Stage;
use words_to_data::legislature::outdated::{OutdatedLink, outdated_links};
use words_to_data::legislature::residue::{Category, Rows, Unlinked, unlinked_amendments};
use words_to_data::query::{Answer, DEFAULT_LIMIT};
use words_to_data::review::short_id;

use crate::load::{self, with_dataset};
use crate::ui;

#[derive(ClapArgs)]
pub struct Args {
    /// Dataset file (`.json` compact or `.sqlite`)
    pub dataset: String,

    /// One bill, such as `119-hr-1`, or the public law it became
    /// (`Pub. L. 119-21`, `PL 119-21` or `119-21`)
    ///
    /// Every public law the dataset holds, when this is left out.
    #[arg(long)]
    pub bill: Option<String>,

    /// Also list each amendment that is linked in part
    ///
    /// A standing link names the amendment, and a change under its address,
    /// in its window, is claimed by no link. Its row has the category
    /// `linked_in_part` and lists only the unclaimed changes. The row does not
    /// say that a change is the amendment's: read the words and decide. These
    /// rows are not in the default list.
    #[arg(long)]
    pub partial: bool,

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

pub fn run(mut args: Args) {
    let ds = crate::fail::or_exit(load::open(&args.dataset), "Error opening dataset");
    if let Some(bill) = &args.bill {
        args.bill = Some(crate::held::bill_argument(&ds, &args.dataset, bill));
    }
    let listed = if args.partial {
        Rows::AlsoLinkedInPart
    } else {
        Rows::Unlinked
    };
    let (rows, outdated) = crate::fail::or_exit(
        with_dataset!(ds, d => unlinked_amendments(&d, args.bill.as_deref(), listed)
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

    print_counts(&rows, args.partial);
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

    println!("\n{}", ui::heading("Work first:"));
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
        Category::LinkedInPart => 1,
        Category::Unwritten => 2,
        Category::NotHeld => 3,
        Category::Quiet => 4,
        Category::ReviewedNoLink => 5,
    }
}

/// How many amendments fall in each category, and for the work, at which
/// stage and for which reason.
///
/// Every count is of amendments. An amendment that acts on many sections has
/// one row for each item (#434), so where a count covers more rows than
/// amendments, the rows follow it: "180 work (181 rows)".
fn print_counts(rows: &[Unlinked], partial: bool) {
    let of = |category: Category| Tally::of(rows.iter().filter(|row| row.category == category));

    let unlinked = Tally::of(
        rows.iter()
            .filter(|row| row.category != Category::LinkedInPart),
    );
    if unlinked.rows == unlinked.amendments.len() {
        println!(
            "{} amendment(s) that no standing link names",
            ui::figure(unlinked.amendments.len())
        );
    } else {
        println!(
            "{} row(s) for {} amendment(s) that no standing link names",
            ui::figure(unlinked.rows),
            ui::figure(unlinked.amendments.len())
        );
    }
    let work = of(Category::Work);
    println!(
        "  {:>4}  {}{}",
        ui::attention(work.amendments.len()).bold(),
        ui::heading("work"),
        work.rows_note()
    );

    let mut reasons: BTreeMap<(&str, &str), Tally> = BTreeMap::new();
    for row in rows.iter().filter(|row| row.category == Category::Work) {
        reasons
            .entry((stage_name(row), row.reason.as_str()))
            .or_default()
            .add(row);
    }
    for ((stage, reason), count) in &reasons {
        println!(
            "          {:>4}  {}{}: {reason}",
            ui::attention(count.amendments.len()),
            ui::quiet(stage),
            count.rows_note()
        );
    }

    if partial {
        let linked_in_part = of(Category::LinkedInPart);
        println!(
            "  {:>4}  {}{}: a link names it, and a change under its address is claimed by no link",
            ui::attention(linked_in_part.amendments.len()).bold(),
            ui::heading("linked in part"),
            linked_in_part.rows_note()
        );
    }
    let unwritten = of(Category::Unwritten);
    println!(
        "  {:>4}  linked by the evidence method, and not written{}: run link-by-evidence",
        unwritten.amendments.len(),
        unwritten.rows_note()
    );
    let not_held = of(Category::NotHeld);
    println!(
        "  {:>4}  not held{}: the amendment changes something the dataset does not hold",
        not_held.amendments.len(),
        not_held.rows_note()
    );
    let quiet = of(Category::Quiet);
    println!(
        "  {:>4}  quiet{}: nothing under the address changed after the law's enactment; not work",
        quiet.amendments.len(),
        quiet.rows_note()
    );

    let mut concluded: BTreeMap<String, Tally> = BTreeMap::new();
    for row in rows {
        if let Some(no_link) = &row.no_link {
            concluded
                .entry(no_link.category.to_string())
                .or_default()
                .add(row);
        }
    }
    let reviewed = of(Category::ReviewedNoLink);
    println!(
        "  {:>4}  reviewed: no link{}: a reviewer concluded the amendment has no correct link; not work",
        reviewed.amendments.len(),
        reviewed.rows_note()
    );
    for (category, count) in &concluded {
        println!(
            "          {:>4}  {category}{}",
            count.amendments.len(),
            count.rows_note()
        );
    }
    print_counted_twice(rows);
}

/// Each amendment whose rows fall in more than one category, with those
/// categories: each of the counts above counts it, so the counts add up to
/// more than the amendments. At most a screenful.
fn print_counted_twice(rows: &[Unlinked]) {
    let mut categories: BTreeMap<&str, BTreeSet<(u8, &str)>> = BTreeMap::new();
    for row in rows {
        categories
            .entry(&row.amendment_id)
            .or_default()
            .insert((queue_place(row.category), category_name(row.category)));
    }
    let twice: Vec<(&str, Vec<&str>)> = categories
        .into_iter()
        .filter(|(_, of)| of.len() > 1)
        .map(|(id, of)| (id, of.into_iter().map(|(_, name)| name).collect()))
        .collect();
    if twice.is_empty() {
        return;
    }
    println!(
        "{} amendment(s) have rows in more than one category, and each of those categories counts them:",
        ui::figure(twice.len())
    );
    for (id, names) in twice.iter().take(DEFAULT_LIMIT) {
        println!("  {}  {}", ui::figure(short_id(id)), names.join(", "));
    }
    if twice.len() > DEFAULT_LIMIT {
        println!("  … {} more", twice.len() - DEFAULT_LIMIT);
    }
}

/// A category as the text output names it.
fn category_name(category: Category) -> &'static str {
    match category {
        Category::Work => "work",
        Category::LinkedInPart => "linked in part",
        Category::Unwritten => "unwritten",
        Category::NotHeld => "not held",
        Category::Quiet => "quiet",
        Category::ReviewedNoLink => "reviewed: no link",
    }
}

/// The rows one count covers, and the amendments they are for.
#[derive(Default)]
struct Tally<'a> {
    rows: usize,
    amendments: BTreeSet<&'a str>,
}

impl<'a> Tally<'a> {
    fn of(rows: impl Iterator<Item = &'a Unlinked>) -> Self {
        let mut tally = Self::default();
        for row in rows {
            tally.add(row);
        }
        tally
    }

    fn add(&mut self, row: &'a Unlinked) {
        self.rows += 1;
        self.amendments.insert(&row.amendment_id);
    }

    /// " (181 rows)" when the rows are more than the amendments, else nothing.
    fn rows_note(&self) -> String {
        if self.rows == self.amendments.len() {
            String::new()
        } else {
            format!(" ({} rows)", self.rows)
        }
    }
}

/// One row: the id and where it stopped, then what is known about it.
fn print_row(row: &Unlinked) {
    let id = &row.amendment_id[..row.amendment_id.len().min(12)];
    let category = match (row.category, &row.no_link) {
        (Category::ReviewedNoLink, Some(no_link)) => {
            format!("reviewed: no link ({})", no_link.category)
        }
        (other, _) => category_name(other).to_string(),
    };
    println!(
        "  {}  {} {}: {}",
        ui::figure(id),
        ui::attention(format!("[{category}]")),
        ui::quiet(stage_name(row)),
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
        println!("    {} {section}{below}", ui::quiet("address:"));
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
        println!("    {} {not_held}", ui::quiet("not held:"));
    }
    if let (Some(from), Some(to)) = (&row.from, &row.to) {
        println!("    {} {from} -> {}", ui::quiet("window:"), to.at);
    }
    let change_label = match row.category {
        Category::LinkedInPart => "unclaimed change:",
        _ => "change:",
    };
    for change in &row.changes {
        println!("    {} {change}", ui::quiet(change_label));
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
