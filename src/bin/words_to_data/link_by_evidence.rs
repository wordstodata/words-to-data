//! `words_to_data link-by-evidence` — link each amendment of every public law
//! to the change it made, from its address, its window and the words it
//! quotes, with no model call (#250).
//!
//! See [`words_to_data::legislature::evidence_matching`] for the method, and
//! `docs/adr/0013-matching-is-evidence-first-and-the-batch-calls-no-model.md`
//! for why.
//!
//! **It writes links, and only links.** What it could not link is not stored:
//! the reason each amendment stopped is derived again whenever it is asked for
//! (#251), because a stored residue goes false the moment someone links it.
//!
//! **It runs again without harm.** The same dataset gives the same links, and a
//! link is identified by what it says, so a second run writes nothing new.
//!
//! **It takes either form the dataset comes in.** A database is changed where
//! it sits. A W2D file is read into memory and written whole, so it must be
//! told where to write and is never written back over its input (#186).
//!
//! **It warns when a public law was stored without its quoted strings.** A
//! dataset whose bills were stored before #257 kept none, and the matcher
//! then links fewer amendments with nothing to say why. It warns, names the
//! law and says to rebuild. It does not refuse: the store cannot say that a
//! string is missing, only that the law's own words quote one and none is
//! stored, and every other law in the dataset is still matched as it should
//! be.

use clap::Args as ClapArgs;
use words_to_data::dataset::{Dataset, adjacent_expressions};
use words_to_data::legislature::evidence_matching::{
    EvidenceMatching, Outcome, Stage, evidence_method, match_by_evidence_reporting,
};
use words_to_data::storage::{LegislatureReader, Storage};

use crate::ui::{self, Task};

#[derive(ClapArgs)]
pub struct Args {
    /// Path to a dataset (compact JSON or SQLite) that holds public laws as
    /// documents and the release points they amend
    pub dataset: String,

    /// Where to write a compact JSON dataset. Required for compact JSON, which
    /// is never written back over its input. Ignored for SQLite, which is
    /// changed in place.
    #[arg(long)]
    pub output: Option<String>,
}

pub fn run(args: Args) {
    let output = if crate::load::is_sqlite(&args.dataset) {
        None
    } else {
        Some(crate::load::output_or_refuse(
            &args.dataset,
            args.output.as_deref(),
            "link-by-evidence",
        ))
    };

    match output {
        None => {
            let mut dataset =
                crate::fail::or_exit(Dataset::open_sqlite(&args.dataset), "Error opening dataset");
            link(&mut dataset);
            println!("Wrote {}", args.dataset);
        }
        Some(output) => {
            let mut dataset = crate::fail::or_exit(
                crate::load::load_compact(&args.dataset),
                "Error loading dataset",
            );
            link(&mut dataset);
            crate::fail::or_exit(
                crate::load::save_compact(&dataset, output),
                "Error saving dataset",
            );
            println!("Wrote {output}");
        }
    }
}

/// Find, write and report.
fn link<S: Storage + LegislatureReader>(dataset: &mut Dataset<S>) {
    let task = Task::start("Match amendments by evidence");
    let found = crate::fail::or_exit(
        match_by_evidence_reporting(dataset, &task),
        "Error matching amendments",
    );
    task.done(&format!("{} amendment(s)", found.matches.len()));
    for (bill_id, public_law) in &found.laws_quoting_no_strings {
        eprintln!(
            "warning: Pub. L. {public_law} ({bill_id}) is stored with no quoted strings. A \
             dataset built before #257 did not keep them, and without them fewer of the law's \
             amendments are linked. Rebuild the dataset with build-dataset to store them."
        );
    }
    let written = crate::ui::step("Write the links", || {
        let mut written = 0;
        for amendment in &found.matches {
            for link in amendment.links() {
                crate::fail::or_exit(dataset.add_link(link), "Error adding link");
                written += 1;
            }
        }
        written
    });
    // Every window of every work, and not only the windows `found` read. The
    // method considered them all: a work no amendment addresses has nothing in
    // it to link, and that is an answer. A run records that the reasoning was
    // applied to a window, not that it wrote a link there (#179, decision 11),
    // so `validate` does not name those windows as outstanding (#252).
    let considered = crate::fail::or_exit(
        adjacent_expressions(&*dataset),
        "Error listing the dataset's windows",
    );
    for (from, to) in &considered {
        crate::fail::or_exit(
            dataset.record_method_run(evidence_method(), from, to),
            "Error recording what ran",
        );
    }
    report(&found, written);
}

/// The counts a reader needs to judge the run: how many amendments, how many
/// linked, and where the rest stopped.
fn report(found: &EvidenceMatching, written: usize) {
    let total = found.matches.len();
    let linked = found
        .matches
        .iter()
        .filter(|amendment| matches!(amendment.outcome, Outcome::Linked(_)))
        .count();
    let stopped_at = |stage: Stage| {
        found
            .matches
            .iter()
            .filter(|amendment| {
                matches!(&amendment.outcome, Outcome::Residue(residue) if residue.stage == stage)
            })
            .count()
    };
    println!("{} {}", ui::heading("Method:"), evidence_method());
    println!("{} amendment(s) of public laws", ui::figure(total));
    println!(
        "  {} linked, as {} link(s)",
        ui::good(linked),
        ui::figure(written)
    );
    for (stage, name) in [
        (Stage::Address, "stopped at the address"),
        (Stage::Window, "stopped at the window"),
        (Stage::Resolve, "stopped at resolving"),
    ] {
        println!("  {} {name}", ui::attention(stopped_at(stage)));
    }
    println!("{} window(s) read", ui::figure(found.windows.len()));
}
