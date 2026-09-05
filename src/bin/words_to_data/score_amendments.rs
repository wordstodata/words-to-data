//! `words_to_data score-amendments` — score bill amendments against the US Code
//! diff, deterministically.
//!
//! Reads the amendment changes that `extract-changes` wrote into the dataset and
//! compares them against the actual changes between two US Code versions. No LLM
//! is involved — the same dataset always yields the same scores.

use std::fs;
use std::path::{Path, PathBuf};

use clap::Args as ClapArgs;
use words_to_data::dataset::{Dataset, ExpressionId, Format};

#[derive(ClapArgs)]
pub struct Args {
    /// Dataset (compact JSON) with extracted amendment changes and both expressions
    pub dataset: String,

    /// Older expression, e.g. `uscode/title_26@2025-07-18`
    #[arg(long)]
    pub from: ExpressionId,

    /// Newer expression of the same work
    #[arg(long)]
    pub to: ExpressionId,

    /// Only keep similarity scores strictly above this cutoff
    #[arg(long, default_value_t = 0.4)]
    pub similarity_cutoff: f32,

    /// Where to write the scores JSON (defaults to `similarity_scores.json` beside the dataset)
    #[arg(long)]
    pub output: Option<String>,
}

pub fn run(args: Args) {
    let dataset = Dataset::load(&args.dataset, Format::Compact).expect("Error loading dataset");

    let diff = dataset
        .compute_diff(&args.from, &args.to)
        .expect("Error computing diff");

    // Score every amendment (with changes) against the US Code diff.
    let mut scores = Vec::new();
    for bill_id in dataset.list_bill_ids().expect("Error listing bills") {
        let bill = dataset
            .get_bill(&bill_id)
            .expect("Error reading bill")
            .expect("bill id from list_bill_ids should exist");
        scores.extend(diff.calculate_amendment_similarities(&bill).into_values());
    }
    scores.retain(|s| s.score > args.similarity_cutoff);
    scores.sort_by(|a, b| b.score.total_cmp(&a.score));

    let scores_path = args
        .output
        .map(PathBuf::from)
        .unwrap_or_else(|| sibling(&args.dataset, "similarity_scores.json"));
    fs::write(
        &scores_path,
        serde_json::to_string_pretty(&scores).expect("Error serializing scores"),
    )
    .expect("Error writing scores");

    println!("Scored {} amendment matches above cutoff", scores.len());
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
