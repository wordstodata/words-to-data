//! `words_to_data` — the user-facing CLI. Every dev tool is a subcommand so
//! only one friendly name lands on the PATH (`words_to_data <command>`).

use clap::{Parser, Subcommand};

mod build_dataset;
mod convert_dataset;
mod llm;
mod match_amendments;
mod score_amendments;

/// Words to Data — legal document tooling
#[derive(Parser)]
#[command(name = "words_to_data", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Build a dataset from US Code release points (and optionally Congress bills)
    BuildDataset(build_dataset::Args),
    /// Convert a serialized JSON dataset into a SQLite file
    ConvertDataset(convert_dataset::Args),
    /// Extract amendment changes via an LLM and score them against the US Code diff
    ScoreAmendments(score_amendments::Args),
    /// Match bill amendments to US Code changes via an LLM and annotate the dataset
    MatchAmendments(match_amendments::Args),
}

fn main() {
    match Cli::parse().command {
        Command::BuildDataset(args) => build_dataset::run(args),
        Command::ConvertDataset(args) => convert_dataset::run(args),
        Command::ScoreAmendments(args) => score_amendments::run(args),
        Command::MatchAmendments(args) => match_amendments::run(args),
    }
}
