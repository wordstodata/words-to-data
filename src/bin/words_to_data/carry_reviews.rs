//! `words_to_data carry-reviews` — carry the reviews of citation links from an
//! old dataset into its rebuild (#319).
//!
//! A rebuild under
//! `docs/adr/0014-a-case-is-a-work-and-a-citation-runs-paragraph-to-subdivision.md`
//! gives every `judicial.cites` link a new id, so a review in the old dataset
//! names no link in the new one. This command maps each old link to the new
//! links that say the same thing, records each review again as a review of each
//! of them, and reports every review that has no new link (decision 9).
//!
//! The old dataset is only read. The new dataset is written: a database in
//! place, a W2D file to `--output`.

use clap::Args as ClapArgs;
use words_to_data::citation::carry::{CarryReport, carry};
use words_to_data::dataset::Dataset;
use words_to_data::review::short_id;
use words_to_data::storage::{LinkReader, Storage};

use crate::load::with_dataset;
use crate::ui;

#[derive(ClapArgs)]
pub struct Args {
    /// Path to the rebuilt dataset, which gets the reviews (compact JSON or SQLite)
    pub dataset: String,

    /// Path to the old dataset, which holds the reviews. It is only read
    #[arg(long)]
    pub from: String,

    /// Print each review, the old link it named and the new links it now
    /// names, as JSON
    #[arg(long)]
    pub json: bool,

    /// Where to write a compact JSON dataset. Required for compact JSON, which
    /// is never written back over its input. Ignored for SQLite, which is
    /// changed in place.
    #[arg(long)]
    pub output: Option<String>,
}

pub fn run(args: Args) {
    let old = crate::fail::or_exit(
        crate::load::open(&args.from),
        "Error opening the old dataset",
    );

    let output = if crate::load::is_sqlite(&args.dataset) {
        None
    } else {
        Some(crate::load::output_or_refuse(
            &args.dataset,
            args.output.as_deref(),
            "carry-reviews",
        ))
    };

    let report = match output {
        None => {
            let mut new =
                crate::fail::or_exit(Dataset::open_sqlite(&args.dataset), "Error opening dataset");
            with_dataset!(old, old => carry_into(&old, &mut new))
        }
        Some(output) => {
            let mut new = crate::fail::or_exit(
                crate::load::load_compact(&args.dataset),
                "Error loading dataset",
            );
            let report = with_dataset!(old, old => carry_into(&old, &mut new));
            crate::fail::or_exit(
                crate::load::save_compact(&new, output),
                "Error saving dataset",
            );
            report
        }
    };

    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report).expect("a report should serialize")
        );
    } else {
        print_report(&report);
    }
}

fn carry_into<O: LinkReader, N: Storage>(old: &O, new: &mut N) -> CarryReport {
    crate::fail::or_exit(carry(old, new), "Error carrying the reviews")
}

fn print_report(report: &CarryReport) {
    println!(
        "{} review(s) of citation links in the old dataset. {} carried to the new links that \
         say the same thing.",
        ui::figure(report.reviews.len()),
        ui::figure(report.carried),
    );
    for review in report.reviews.iter().filter(|review| review.why.is_none()) {
        println!(
            "  {} by {}: link {} -> {}",
            review.verdict,
            review.reviewer,
            short_id(&review.old_link),
            review
                .new_links
                .iter()
                .map(|id| short_id(id))
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
    if report.not_carried == 0 {
        return;
    }
    // Never dropped: each one is named, with its reason, so a reviewer can look
    // at it again. The record stays in the old dataset.
    println!(
        "\n{} review(s) have no new link that says the same thing. They stay in the old \
         dataset, and nothing is written for them:",
        ui::figure(report.not_carried),
    );
    for review in report.reviews.iter() {
        let Some(why) = &review.why else {
            continue;
        };
        println!(
            "  {} by {}: link {} ({} cites {} as {}): {why}",
            review.verdict,
            review.reviewer,
            short_id(&review.old_link),
            review.statement.record,
            review.statement.section,
            review.statement.written.join(", "),
        );
    }
}
