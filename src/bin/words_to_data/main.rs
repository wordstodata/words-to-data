//! `words_to_data` — the user-facing CLI. Every dev tool is a subcommand so
//! only one friendly name lands on the PATH (`words_to_data <command>`).

use clap::{Parser, Subcommand};

mod add_bills;
mod add_classifications;
mod add_opinions;
mod add_release_points;
mod amendment_addresses;
mod annotations;
mod bills;
mod build_dataset;
mod cases_citing;
mod congress_bills;
mod contradictions;
mod convert_dataset;
mod coverage;
mod diff;
mod expressions;
mod fail;
mod info;
mod link_amendment;
mod link_by_evidence;
mod load;
mod path;
mod redesignation_report;
mod redesignations;
mod release_points;
mod residue;
mod search;
mod section_agreement;
mod settle;
mod show_bill;
mod span;
mod ui;
mod validate;
mod votes;

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
    /// Link each amendment of every public law to its change from address, window and quoted words (no LLM)
    LinkByEvidence(link_by_evidence::Args),
    /// Record the provisions a bill renumbered, as links (deterministic, no LLM)
    Redesignations(redesignations::Args),
    /// Settle one link: say whether it is right, and record the review as its own link
    Settle(settle::Args),
    /// Record an amendment link an agent found, refusing any path that did not change
    LinkAmendment(link_amendment::Args),
    /// Add court opinions from CourtListener, with their U.S.C. citations as links
    AddOpinions(add_opinions::Args),
    /// Add more US Code release points to a dataset that is already there
    AddReleasePoints(add_release_points::Args),
    /// Add Congress bills to a dataset that is already there
    AddBills(add_bills::Args),
    /// Add the OLRC's classification of each public law the dataset holds, as links
    AddClassifications(add_classifications::Args),

    // --- Inspection (read-only) ---
    /// Show a dataset's metadata and headline counts
    Info(info::Args),
    /// List every expression (`work@date`) with its size
    Expressions(expressions::Args),
    /// List every bill the dataset holds
    Bills(bills::Args),
    /// Report every renumbering the dataset's bills state, weakest first
    RedesignationReport(redesignation_report::Args),
    /// List the subjects the dataset holds more than one link about
    Contradictions(contradictions::Args),
    /// Report the amendments whose own words name a section their link does not sit in
    SectionAgreement(section_agreement::Args),
    /// Show the Code address each amendment of a public law acts on, or why it is not known
    AmendmentAddresses(amendment_addresses::Args),
    /// List the amendments of public laws that no standing link names, with the stage and reason each stopped
    Residue(residue::Args),

    /// Show a bill's amendments
    ShowBill(show_bill::Args),
    /// Show how the House voted on a bill, by the party held on the vote's date
    Votes(votes::Args),
    /// Full-text search across every expression
    Search(search::Args),
    /// Which opinions cite a provision, and whether it changed after each was filed
    CasesCiting(cases_citing::Args),
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
        Command::LinkByEvidence(args) => link_by_evidence::run(args),
        Command::Redesignations(args) => redesignations::run(args),
        Command::Settle(args) => settle::run(args),
        Command::LinkAmendment(args) => link_amendment::run(args),
        Command::AddOpinions(args) => add_opinions::run(args),
        Command::AddReleasePoints(args) => add_release_points::run(args),
        Command::AddBills(args) => add_bills::run(args),
        Command::AddClassifications(args) => add_classifications::run(args),
        Command::CasesCiting(args) => cases_citing::run(args),
        Command::Info(args) => info::run(args),
        Command::Expressions(args) => expressions::run(args),
        Command::Bills(args) => bills::run(args),
        Command::RedesignationReport(args) => redesignation_report::run(args),
        Command::Contradictions(args) => contradictions::run(args),
        Command::SectionAgreement(args) => section_agreement::run(args),
        Command::AmendmentAddresses(args) => amendment_addresses::run(args),
        Command::Residue(args) => residue::run(args),
        Command::ShowBill(args) => show_bill::run(args),
        Command::Votes(args) => votes::run(args),
        Command::Search(args) => search::run(args),
        Command::Diff(args) => diff::run(args),
        Command::Coverage(args) => coverage::run(args),
        Command::Annotations(args) => annotations::run(args),
        Command::Path(args) => path::run(args),
        Command::Validate(args) => validate::run(args),
    }
}
