//! `words_to_data path` — everything about one structural path: where it
//! exists, its field-level changes for an expression pair, and its annotations.

use clap::Args as ClapArgs;
use words_to_data::dataset::ExpressionId;
use words_to_data::inspect;

use crate::load::{self, with_dataset};

#[derive(ClapArgs)]
pub struct Args {
    /// Dataset file (`.json` compact or `.sqlite`)
    pub dataset: String,

    /// Structural path to inspect (e.g. `uscode/title_9/chapter_1/section_1`).
    /// Annotations on this path and on every path beneath it are reported
    pub path: String,

    /// Older expression for field changes, e.g. `uscode/title_9@2025-07-18` (requires `--to`)
    #[arg(long, requires = "to")]
    pub from: Option<ExpressionId>,

    /// Newer expression of the same work (requires `--from`)
    #[arg(long, requires = "from")]
    pub to: Option<ExpressionId>,

    /// Report only the annotations recorded on this path itself, not those beneath it
    #[arg(long)]
    pub exact: bool,

    /// Emit JSON instead of human-readable text
    #[arg(long)]
    pub json: bool,
}

pub fn run(args: Args) {
    let pair = match (&args.from, &args.to) {
        (Some(from), Some(to)) => Some((from, to)),
        _ => None,
    };

    let matching = crate::annotations::path_matching(args.exact);
    let ds = crate::fail::or_exit(load::open(&args.dataset), "Error opening dataset");
    let report = crate::fail::or_exit(
        with_dataset!(ds, d => inspect::path_report(&d, &args.path, pair, matching)),
        "Error building path report",
    );

    if args.json {
        println!("{}", serde_json::to_string_pretty(&report).unwrap());
        return;
    }

    println!("PATH: {}", report.path);
    println!(
        "Present in: {}",
        if report.present_in.is_empty() {
            "(no expression)".to_string()
        } else {
            report
                .present_in
                .iter()
                .map(|p| format!("{} ({})", p.expression, provisions(p.provisions)))
                .collect::<Vec<_>>()
                .join(", ")
        }
    );

    if let (Some(from), Some(to)) = (&args.from, &args.to) {
        println!(
            "\nProvisions ({from} -> {to}) ({}):",
            report.provisions.len()
        );
        for p in &report.provisions {
            println!("  {} — {}", verdict(&p.presence), positions(p));
            for link in &p.via {
                println!("      {}", stated_by(link));
            }
            for c in &p.changes {
                println!("      {}: {:?} -> {:?}", c.field, c.old_value, c.new_value);
            }
        }
    }

    print_unfollowed(&report.unfollowed_redesignations);

    // Say which paths the count covers. A bare "Annotations (0)" reads as
    // "nothing was attributed here", which is a different statement.
    let scope = if args.exact {
        "on this path exactly"
    } else {
        "at or beneath this path"
    };
    println!("\nAnnotations {scope} ({}):", report.annotations.len());
    for a in &report.annotations {
        crate::annotations::print_annotation(a);
    }
}

fn provisions(n: usize) -> String {
    if n == 1 {
        "1 provision".to_string()
    } else {
        format!("{n} provisions")
    }
}

fn verdict(presence: &inspect::Presence) -> String {
    match presence {
        inspect::Presence::InBoth => "in both".to_string(),
        inspect::Presence::Added => "added".to_string(),
        inspect::Presence::Removed => "removed".to_string(),
        // The destination is named here rather than left to a second command.
        // A move whose other end is not shown is the answer this report exists
        // to replace.
        inspect::Presence::MovedOut { to_path } => format!("moved out to {to_path}"),
        inspect::Presence::MovedIn { from_path } => format!("moved in from {from_path}"),
    }
}

/// Who said a provision moved, when, and how far it can be trusted.
///
/// Every link is followed whatever its corroboration figure, so the state and
/// the provenance are printed beside the claim instead. Corroboration is
/// evidence for a reviewer, not a substitute for one.
fn stated_by(link: &inspect::RedesignationLink) -> String {
    let bill = match &link.bill_id {
        Some(bill) => format!("stated by {bill}"),
        None => "stated by an unnamed source".to_string(),
    };
    format!(
        "{bill}, between {} and {} ({})",
        link.from_date,
        link.to_date,
        trust(link.verification)
    )
}

fn trust(verification: words_to_data::link::VerificationState) -> &'static str {
    use words_to_data::link::VerificationState as V;
    match verification {
        V::Asserted => "asserted by a source",
        V::MachineSuggested => "machine suggested, unconfirmed",
        V::HumanConfirmed => "human confirmed",
        V::Disputed => "disputed",
        V::Refuted => "refuted",
    }
}

/// Say which redesignation links the report saw and did not follow.
///
/// Silence here would put the reader back where #165 found them: a path named
/// by redesignation links, answered as though no link existed.
fn print_unfollowed(unfollowed: &[inspect::UnfollowedRedesignation]) {
    if unfollowed.is_empty() {
        return;
    }
    let no_window = unfollowed
        .iter()
        .all(|u| u.reason == inspect::NotFollowed::NoWindow);
    if no_window {
        println!(
            "\n{} redesignation link(s) name this path. Give --from and --to to resolve them.",
            unfollowed.len()
        );
        return;
    }

    println!("\nRedesignations not followed ({}):", unfollowed.len());
    for u in unfollowed {
        let why = match u.reason {
            inspect::NotFollowed::NoWindow => "no expression pair was given",
            inspect::NotFollowed::Refuted => "refuted, so it was checked and found wrong",
        };
        println!("  {} -> {} — {why}", u.link.from_path, u.link.to_path);
        println!("      {}", stated_by(&u.link));
    }
}

/// Where the provision sits on each side, using the same indices as `--json`.
///
/// A provision that shares its path with another is only addressable by
/// position, so the position is printed rather than left to the reader to
/// count off the list.
fn positions(p: &inspect::ProvisionAtPath) -> String {
    match (p.from_position, p.to_position) {
        (Some(from), Some(to)) => format!("from position {from}, to position {to}"),
        (Some(from), None) => format!("was at position {from}"),
        (None, Some(to)) => format!("now at position {to}"),
        (None, None) => "no position".to_string(),
    }
}
