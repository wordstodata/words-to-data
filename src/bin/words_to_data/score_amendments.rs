//! `words_to_data score-amendments` — score bill amendments against the US Code
//! diff, deterministically.
//!
//! Reads the amendment changes that `extract-changes` wrote into the dataset and
//! compares them against the actual changes between two US Code versions. No LLM
//! is involved — the same dataset always yields the same scores.

use std::fs;
use std::path::{Path, PathBuf};

use clap::Args as ClapArgs;
use serde::Serialize;
use words_to_data::dataset::{Dataset, Format};
use words_to_data::diff::AmendmentSimilarity;

use crate::span::Span;

#[derive(ClapArgs)]
pub struct Args {
    /// Dataset (compact JSON) with extracted amendment changes and the expressions to score
    pub dataset: String,

    #[command(flatten)]
    pub span: Span,

    /// Only keep similarity scores strictly above this cutoff
    #[arg(long, default_value_t = 0.4)]
    pub similarity_cutoff: f32,

    /// Where to write the scores JSON (defaults to `similarity_scores.json` beside the dataset)
    #[arg(long)]
    pub output: Option<String>,
}

/// One work's scores, tagged with the pair they were computed between.
///
/// The scores themselves say nothing about which document they came from, so a
/// corpus run would otherwise write one flat list in which title 26's matches
/// and title 51's are indistinguishable.
#[derive(Serialize)]
struct ScoredWork {
    work: String,
    from: String,
    to: String,
    scores: Vec<AmendmentSimilarity>,
}

pub fn run(args: Args) {
    let dataset = crate::fail::or_exit(
        Dataset::load(&args.dataset, Format::Compact),
        "Error loading dataset",
    );

    let bills: Vec<_> = crate::fail::or_exit(dataset.list_bill_ids(), "Error listing bills")
        .into_iter()
        .map(|id| {
            crate::fail::or_exit(dataset.get_bill(&id), "Error reading bill")
                .expect("bill id from list_bill_ids should exist")
        })
        .collect();

    let mut scored = Vec::new();
    let mut total = 0;

    for (from, to) in args.span.resolve(&dataset) {
        let diff = crate::fail::or_exit(dataset.compute_diff(&from, &to), "Error computing diff");

        let mut scores: Vec<AmendmentSimilarity> = bills
            .iter()
            .flat_map(|bill| diff.calculate_amendment_similarities(bill).into_values())
            .collect();
        scores.retain(|s| s.score > args.similarity_cutoff);
        scores.sort_by(|a, b| b.score.total_cmp(&a.score));

        println!("{from} -> {to}: {} above cutoff", scores.len());
        total += scores.len();
        scored.push(ScoredWork {
            work: from.work.to_string(),
            from: from.to_string(),
            to: to.to_string(),
            scores,
        });
    }

    let scores_path = args
        .output
        .map(PathBuf::from)
        .unwrap_or_else(|| sibling(&args.dataset, "similarity_scores.json"));
    crate::fail::or_exit(
        fs::write(
            &scores_path,
            serde_json::to_string_pretty(&scored).expect("Error serializing scores"),
        ),
        "Error writing scores",
    );

    println!(
        "Scored {total} amendment match(es) above cutoff across {} work(s)",
        scored.len()
    );
    println!("Wrote {}", scores_path.display());
}

/// Build a path to `filename` in the same directory as `dataset_path`.
fn sibling(dataset_path: &str, filename: &str) -> PathBuf {
    Path::new(dataset_path)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .join(filename)
}
