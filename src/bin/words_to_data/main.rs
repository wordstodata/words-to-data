//! `words_to_data` — the user-facing CLI. Every dev tool is a subcommand so
//! only one friendly name lands on the PATH (`words_to_data <command>`).

use clap::{Parser, Subcommand};

mod annotations;
mod bills;
mod build_dataset;
mod convert_dataset;
mod coverage;
mod diff;
mod expressions;
mod extract_changes;
mod fail;
mod info;
mod load;
mod match_amendments;
mod path;
mod report;
mod score_amendments;
mod search;
mod show_bill;
mod span;
mod validate;

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
    /// Convert a dataset between compact JSON and SQLite (either direction)
    ConvertDataset(convert_dataset::Args),
    /// Extract word-level amendment changes for every bill via an LLM (writes into the dataset)
    ExtractChanges(extract_changes::Args),
    /// Score amendment changes against the US Code diff (deterministic, no LLM)
    ScoreAmendments(score_amendments::Args),
    /// Match bill amendments to US Code changes via an LLM and annotate the dataset
    MatchAmendments(match_amendments::Args),

    // --- Inspection (read-only) ---
    /// Show a dataset's metadata and headline counts
    Info(info::Args),
    /// List every expression (`work@date`) with its size
    Expressions(expressions::Args),
    /// List every bill the dataset holds
    Bills(bills::Args),

    /// Show a bill's amendments
    ShowBill(show_bill::Args),
    /// Full-text search across every expression
    Search(search::Args),
    /// List the paths that changed between two expressions of one work
    Diff(diff::Args),
    /// Report annotation coverage of a diff (the unannotated work queue)
    Coverage(coverage::Args),
    /// List annotations, filtered by expression pair, bill, or path
    Annotations(annotations::Args),
    /// Inspect one path: presence, field changes, and annotations
    Path(path::Args),
    /// Check a dataset's internal consistency (non-zero exit on failure)
    Validate(validate::Args),
}

fn main() {
    match Cli::parse().command {
        Command::BuildDataset(args) => build_dataset::run(args),
        Command::ConvertDataset(args) => convert_dataset::run(args),
        Command::ExtractChanges(args) => extract_changes::run(args),
        Command::ScoreAmendments(args) => score_amendments::run(args),
        Command::MatchAmendments(args) => match_amendments::run(args),
        Command::Info(args) => info::run(args),
        Command::Expressions(args) => expressions::run(args),
        Command::Bills(args) => bills::run(args),
        Command::ShowBill(args) => show_bill::run(args),
        Command::Search(args) => search::run(args),
        Command::Diff(args) => diff::run(args),
        Command::Coverage(args) => coverage::run(args),
        Command::Annotations(args) => annotations::run(args),
        Command::Path(args) => path::run(args),
        Command::Validate(args) => validate::run(args),
    }
}
