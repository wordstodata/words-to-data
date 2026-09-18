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
//! **Every statement it cannot place is printed.** A run that resolved nothing
//! and said nothing would read as a corpus with no redesignations in it, and the
//! corpus is full of them.

use clap::Args as ClapArgs;
use words_to_data::dataset::{Dataset, Format};
use words_to_data::legislature::redesignation::RedesignationReport;
use words_to_data::storage::DocumentReader;
use words_to_data::uslm::bill_redesignation::redesignations_stated_in;

use crate::span::Span;

#[derive(ClapArgs)]
pub struct Args {
    /// Path to a dataset (compact JSON) holding the works the bill amends
    pub dataset: String,

    /// Which bill to read, as the dataset names it, such as `119-hr-1`
    #[arg(long)]
    pub bill_id: String,

    #[command(flatten)]
    pub span: Span,

    /// Where to write the dataset (defaults to overwriting the input)
    #[arg(long)]
    pub output: Option<String>,
}

pub fn run(args: Args) {
    crate::load::refuse_sqlite(&args.dataset, "redesignations");
    let mut dataset = crate::fail::or_exit(
        Dataset::load(&args.dataset, Format::Compact),
        "Error loading dataset",
    );

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
    let pairs = args.span.resolve(&dataset as &dyn DocumentReader);
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

    let output = args.output.as_deref().unwrap_or(&args.dataset);
    crate::fail::or_exit(
        dataset.save(output, Format::Compact),
        "Error saving dataset",
    );
    println!("\nWrote {output}");
}
