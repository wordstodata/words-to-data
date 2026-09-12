//! `words_to_data redesignations` — record the provisions a bill renumbered.
//!
//! Reads the `redesignate` clauses out of a bill's markup, resolves each one to
//! the two paths it moved a provision between, and writes each as a
//! `legislature.redesignated_as` link. The diff then pairs a renumbered provision
//! with what it became instead of with whatever took its number (#93).
//!
//! Takes the bill's XML rather than the bill the dataset stores. Which provision
//! a clause is about comes from where the words sat in the markup — a clause
//! inside "in subsection (a)--" means something different from the same clause
//! outside it — and a stored amendment keeps only the flattened text.
//!
//! **Every statement it cannot place is printed.** A run that resolved nothing
//! and said nothing would read as a corpus with no redesignations in it, and the
//! corpus is full of them.

use clap::Args as ClapArgs;
use words_to_data::dataset::{Dataset, Format};
use words_to_data::legislature::redesignation::RedesignationReport;
use words_to_data::storage::DocumentReader;
use words_to_data::uslm::bill_redesignation::redesignations_stated_in_file;

use crate::span::Span;

#[derive(ClapArgs)]
pub struct Args {
    /// Path to a dataset (compact JSON) holding the works the bill amends
    pub dataset: String,

    /// The bill's USLM XML, such as a `public_law.xml` from the Congress cache
    #[arg(long)]
    pub bill_xml: String,

    /// How the bill is named in links and reports, such as `119-hr-1`
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

    let stated = crate::fail::or_exit(
        redesignations_stated_in_file(&args.bill_id, &args.bill_xml),
        "Error reading the bill",
    );
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

    println!(
        "Recorded {} link(s) across {} work pair(s).",
        report.resolved.len(),
        pairs.len()
    );
    if report.unresolved.is_empty() {
        println!("Every statement was placed.");
    } else {
        println!(
            "\n{} statement(s) could not be placed. Each is a redesignation the \
             corpus states and this build cannot turn into two paths:",
            report.unresolved.len()
        );
        for unresolved in &report.unresolved {
            println!("  {}", unresolved.reason);
            println!("    {}", first_words(&unresolved.text));
        }
    }

    let output = args.output.as_deref().unwrap_or(&args.dataset);
    crate::fail::or_exit(
        dataset.save(output, Format::Compact),
        "Error saving dataset",
    );
    println!("\nWrote {output}");
}

/// The start of a clause, so one statement stays one line.
///
/// A clause that enacts new text carries the whole of it, which runs to
/// thousands of characters and buries every other line of the report.
fn first_words(text: &str) -> String {
    const SHOWN: usize = 140;
    match text.char_indices().nth(SHOWN) {
        Some((end, _)) => format!("{}…", &text[..end]),
        None => text.to_string(),
    }
}
