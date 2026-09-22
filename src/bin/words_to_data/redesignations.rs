//! `words_to_data redesignations` — record the provisions a bill renumbered.
//!
//! Reads the `redesignate` clauses out of a bill, resolves each one to
//! the two paths it moved a provision between, and writes each as a
//! `legislature.redesignated_as` link. The diff then pairs a renumbered provision
//! with what it became instead of with whatever took its number (#93).
//!
//! Reads the bill out of the dataset. Which provision a clause is about comes
//! from where the words sat in the bill — a clause inside "in subsection (a)--"
//! means something different from the same clause outside it — and the dataset
//! now holds the bill as a document, with that nesting in it. Until #196 it did
//! not, so this command took the bill's XML and read the same file a second
//! time.
//!
//! **It takes either form the dataset comes in.** A database is changed where it
//! sits; a W2D file is read into memory and written out again (#195).
//!
//! **Every statement it cannot place is printed.** A run that resolved nothing
//! and said nothing would read as a corpus with no redesignations in it, and the
//! corpus is full of them.

use clap::Args as ClapArgs;
use words_to_data::dataset::{Dataset, Format};
use words_to_data::legislature::redesignation::RedesignationReport;
use words_to_data::storage::{LegislatureReader, Storage};
use words_to_data::uslm::bill_redesignation::redesignations_stated_in;

use crate::span::Span;

#[derive(ClapArgs)]
pub struct Args {
    /// Path to a dataset (compact JSON or SQLite) holding the works the bill amends
    pub dataset: String,

    /// Which bill to read, as the dataset names it, such as `119-hr-1`
    #[arg(long)]
    pub bill_id: String,

    #[command(flatten)]
    pub span: Span,

    /// Where to write a compact JSON dataset. Required for compact JSON, which
    /// is never written back over its input. Ignored for SQLite, which is
    /// changed in place.
    #[arg(long)]
    pub output: Option<String>,
}

pub fn run(args: Args) {
    // Where the result goes: `None` is a database, which is changed in place.
    // A W2D file is written whole, so it must be told where to write and is
    // never written back over its input (#186).
    let output = if crate::load::is_sqlite(&args.dataset) {
        None
    } else {
        Some(crate::load::output_or_refuse(
            &args.dataset,
            args.output.as_deref(),
            "redesignations",
        ))
    };

    match output {
        None => {
            let mut dataset =
                crate::fail::or_exit(Dataset::open_sqlite(&args.dataset), "Error opening dataset");
            record(&mut dataset, &args);
            println!("\nWrote {}", args.dataset);
        }
        Some(output) => {
            let mut dataset = crate::fail::or_exit(
                Dataset::load(&args.dataset, Format::Compact),
                "Error loading dataset",
            );
            record(&mut dataset, &args);
            crate::fail::or_exit(
                dataset.save(output, Format::Compact),
                "Error saving dataset",
            );
            println!("\nWrote {output}");
        }
    }
}

/// Read what the bill renumbered, and write a link for each statement it can
/// place.
fn record<S: Storage + LegislatureReader>(dataset: &mut Dataset<S>, args: &Args) {
    let bill = crate::fail::or_exit(
        dataset.bill_document(&args.bill_id),
        "Error reading the dataset's bills",
    )
    .unwrap_or_else(|| {
        eprintln!(
            "This dataset holds no document for bill {}. Load the bill with \
             build-dataset, which stores it (#196).",
            args.bill_id
        );
        std::process::exit(1);
    });

    let stated = redesignations_stated_in(&args.bill_id, &bill.root);
    println!("{} states {} redesignation(s).", args.bill_id, stated.len());

    // One report per work. A statement resolves in the work that holds its
    // section and fails in every other, so the reports are folded rather than
    // concatenated (`RedesignationReport::across_works`).
    let pairs = args.span.resolve(&*dataset);
    let mut per_work = Vec::new();
    for (from, to) in &pairs {
        let report = crate::fail::or_exit(
            dataset.record_redesignations(&args.bill_id, &stated, from, to),
            "Error recording redesignations",
        );
        per_work.push(report);
    }
    let report = RedesignationReport::across_works(per_work);

    // The report's own counts. One clause states many renumberings, so a count
    // of links is not a count of statements (#166).
    println!(
        "Recorded {} link(s) across {} work pair(s).",
        report.links(),
        pairs.len()
    );
    if report.unresolved.is_empty() {
        println!("Every statement was placed.");
    } else {
        println!(
            "\n{} statement(s) could not be placed. Each is a redesignation the \
             corpus states and this build cannot turn into two paths:",
            report.unplaced()
        );
        for unresolved in &report.unresolved {
            println!("  {}", unresolved.reason);
            println!("    {}", unresolved.clause_start());
        }
    }
}
