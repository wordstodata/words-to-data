//! `words_to_data section-agreement` — the amendment links whose own words name
//! a section their path does not sit in.
//!
//! A queue for a reviewer, not a verdict. A disagreement has an innocent
//! explanation — an amendment may name one section and act on a provision in
//! another, because the drafter said so — so the run says that in as many words
//! rather than leaving a reader to call every row a fault (#239).
//!
//! **It reads the dataset and nothing else.** No XML, no model call, and no
//! diff: the link names the amendment, the bill the dataset holds says where
//! that amendment acts, and the section a link sits in is in its path. Nothing
//! is stored, because the answer is derived from what the dataset already holds
//! (`docs/adr/0007-a-record-is-what-was-said-everything-else-is-derived.md`).
//!
//! # Where the amendment's section comes from
//!
//! The address resolver reads it out of the bill's own markup, the same reading
//! `amendment-addresses` shows (#248): the section the amending line cites, the
//! publisher's `<ref>` beside a section of an Act, and a new section's own
//! `SEC.` heading. The markup sets quoted text apart, so a section the
//! amendment only quotes or cross-references is never read as the one it acts
//! on.
//!
//! **Known limit.** An amendment the resolver cannot address — one that names
//! a part or a subchapter, or a table of sections — is reported as *could not
//! be read*, with the resolver's reason, and never as a disagreement. So is a
//! link whose bill the dataset holds no document for.

use clap::Args as ClapArgs;
use words_to_data::dataset::ExpressionId;
use words_to_data::legislature::section_agreement::{self, Outcome, Row, WindowTally};
use words_to_data::query::{DEFAULT_LIMIT, LinkQuery, Locator};

use crate::load::{self, with_dataset};

#[derive(ClapArgs)]
pub struct Args {
    /// Dataset file (`.json` compact or `.sqlite`)
    pub dataset: String,

    /// Older expression of the pair, e.g. `uscode/title_26@2025-07-18` (requires `--to`)
    #[arg(long, requires = "to")]
    pub from: Option<ExpressionId>,

    /// Newer expression of the pair (requires `--from`)
    #[arg(long, requires = "from")]
    pub to: Option<ExpressionId>,

    /// Check only the links sourced from this bill
    ///
    /// Combines with `--path` and `--from`/`--to`: every filter given is applied
    #[arg(long)]
    pub bill: Option<String>,

    /// Check only the links on this structural path or on any path beneath it
    #[arg(long)]
    pub path: Option<String>,

    /// Match `--path` exactly: keep only the links recorded on that path itself
    #[arg(long, requires = "path")]
    pub exact: bool,

    /// How many rows to print before stopping. The run says how many it held back
    #[arg(long)]
    pub limit: Option<usize>,

    /// Print every row, however many there are
    #[arg(long, conflicts_with = "limit")]
    pub all: bool,

    /// Emit JSON instead of human-readable text
    #[arg(long)]
    pub json: bool,
}

pub fn run(args: Args) {
    let mut locator = Locator::new();
    if let (Some(from), Some(to)) = (&args.from, &args.to) {
        locator = locator
            .in_work(from.work.to_string())
            .in_window(from.at.clone(), to.at.clone());
    }
    if let Some(path) = &args.path {
        locator = locator.at_path(path, crate::annotations::path_matching(args.exact));
    }

    let mut query = LinkQuery::new().at(locator);
    if let Some(bill) = &args.bill {
        // The query is class-neutral: it filters on a prefix of the object's
        // reference, and turning a bill into that prefix is the CLI's job
        // (`docs/adr/0002-links-live-in-the-core.md`).
        query = query.with_object_prefix(words_to_data::link::bill_reference_prefix(bill));
    }

    let ds = crate::fail::or_exit(load::open(&args.dataset), "Error opening dataset");
    let report = crate::fail::or_exit(
        with_dataset!(ds, d => section_agreement::section_agreement(&d, &query)),
        "Error checking the amendments against their paths",
    );

    if args.json {
        println!("{}", serde_json::to_string_pretty(&report).unwrap());
        return;
    }

    if report.windows.is_empty() {
        println!("This dataset holds no amendment link to check.");
        return;
    }

    // Per window, because the split between windows is what filed this: one
    // dataset showed 4% of its first window's links disagreeing and 29% of its
    // second window's, and a figure folded over a whole dataset hides that.
    println!("Per window:");
    for tally in &report.windows {
        print_window(tally);
    }

    if report.rows.is_empty() {
        println!("\nEvery amendment names the section its link points into.");
        return;
    }

    // Said before the rows, not after them. A reader who meets the list first
    // has already read it as a fault list by the time the caveat arrives.
    println!(
        "\nThis is a queue for a person, not a list of faults. An amendment may \
         name one section\nand act on a provision in another, because the \
         drafter said so, and a citation this\nbuild cannot read is a limit of \
         the reader rather than a fault in the link."
    );

    let shown = match (args.all, args.limit) {
        (true, _) => report.rows.len(),
        (_, Some(explicit)) => explicit,
        (_, None) => DEFAULT_LIMIT,
    };
    println!("\nSuspect first:");
    for row in report.rows.iter().take(shown) {
        print_row(row);
    }
    if report.rows.len() > shown {
        println!(
            "  … {} more row(s); pass --all for every one of them",
            report.rows.len() - shown
        );
    }
}

/// One window's counts and its share.
///
/// The three outcomes add up to the links checked, and the line says so by
/// printing the total first. The counts sit beside the share so the denominator
/// is visible rather than guessed: it is every link in the window, the unread
/// ones included.
fn print_window(tally: &WindowTally) {
    println!(
        "  {}  {} link(s): {} agree, {} disagree ({:.1}%), {} could not be read",
        tally.window,
        tally.checked,
        tally.agrees,
        tally.disagrees,
        tally.disagreeing_share * 100.0,
        tally.could_not_be_read,
    );
}

/// One queued row: the id first, then what the check found, then where.
///
/// The id leads because this list is the review queue and the id is the one
/// field a reviewer copies out of it, into `settle` (#227).
fn print_row(row: &Row) {
    match row.outcome {
        Outcome::Disagrees => println!(
            "  {}  disagrees — the amendment names § {}, the link sits in § {}",
            row.id,
            row.named_section.as_deref().unwrap_or("?"),
            row.path_section.as_deref().unwrap_or("?"),
        ),
        Outcome::CouldNotBeRead => println!(
            "  {}  could not be read — {}",
            row.id,
            row.reason.as_deref().unwrap_or("no reason recorded"),
        ),
        // Never queued, so never printed. Matched rather than left to a
        // catch-all, so a fourth outcome cannot arrive here unnoticed.
        Outcome::Agrees => return,
    }
    println!("    {}", row.path);
    if let Some(window) = &row.window {
        println!("    in {window}");
    }
}
