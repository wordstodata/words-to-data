//! `words_to_data link-amendment` — record an `amended_by` link an agent found.
//!
//! `settle` reviews a link that exists, and until this existed no command could
//! create one. On 2026-09-27 an agent with only the CLI resolved 20 amendments
//! the matcher had missed, and could record none of them (#249).
//!
//! **The door cannot invent a path.** Every path it is given must have changed
//! in the window it is given, so the recorder chooses among the changes the diff
//! produced (`docs/adr/0010-two-readers-one-resolver-a-model-never-writes-a-path.md`,
//! `docs/adr/0013-matching-is-evidence-first-and-the-batch-calls-no-model.md`).
//!
//! **The link is the shape the batch writes.** It is `MachineSuggested`, its
//! source is the name the recorder gives, and its reasoning is the reason the
//! recorder writes. So `annotations`, `path`, `settle`, `section-agreement` and
//! `info` read it with no change, and `settle` reviews it like any other link.
//!
//! **It takes either form the dataset comes in.** A database is changed where it
//! sits; a W2D file is read into memory and written out again (#195).

use clap::Args as ClapArgs;
use words_to_data::dataset::{Dataset, ExpressionId, Format};
use words_to_data::inspect;
use words_to_data::legislature::{AmendingAction, BillAmendment};
use words_to_data::link::{
    Evidence, KindPayload, Link, LinkKind, Provenance, Target, VerificationState,
    amendment_reference,
};
use words_to_data::storage::{LegislatureReader, Storage};

#[derive(ClapArgs)]
pub struct Args {
    /// Path to a dataset (compact JSON or SQLite) that holds the bill and both
    /// expressions of the window
    pub dataset: String,

    /// The bill the amendment belongs to, e.g. `119-hr-1`
    #[arg(long)]
    pub bill: String,

    /// The amendment's id, in full. `show-bill` prints it
    #[arg(long)]
    pub amendment: String,

    /// Older expression of the window, e.g. `uscode/title_26@2025-07-18`
    #[arg(long)]
    pub from: ExpressionId,

    /// The next expression of the same work
    #[arg(long)]
    pub to: ExpressionId,

    /// A path that changed in the window. Give it once for each path
    #[arg(long = "path", required = true)]
    pub paths: Vec<String>,

    /// Who is recording the link: `agent:claude`, `human:jesse`
    #[arg(long)]
    pub source: String,

    /// Why the amendment caused these changes, in the recorder's own words
    #[arg(long)]
    pub reason: String,

    /// Where to write a compact JSON dataset. Required for compact JSON, which
    /// is never written back over its input. Ignored for SQLite, which is
    /// changed in place.
    #[arg(long)]
    pub output: Option<String>,
}

pub fn run(args: Args) {
    // The reason is the link's evidence, and a reviewer reads it to judge the
    // link. A link with none has nothing behind it, so it is refused before the
    // dataset is even opened.
    if args.reason.trim().is_empty() {
        eprintln!(
            "The reason is empty, so nothing was written.\n\
             Say why the amendment caused these changes: a reviewer judges the link by it."
        );
        std::process::exit(1);
    }

    // Where the result goes: `None` is a database, which is changed in place.
    // A W2D file is written whole, so it must be told where to write and is
    // never written back over its input (#186).
    let output = if crate::load::is_sqlite(&args.dataset) {
        None
    } else {
        Some(crate::load::output_or_refuse(
            &args.dataset,
            args.output.as_deref(),
            "link-amendment",
        ))
    };

    match output {
        None => {
            let mut dataset =
                crate::fail::or_exit(Dataset::open_sqlite(&args.dataset), "Error opening dataset");
            record(&mut dataset, &args);
            println!("\nWrote {}", args.dataset);
        }
        Some(output) => {
            let mut dataset = crate::fail::or_exit(
                Dataset::load(&args.dataset, Format::Compact),
                "Error loading dataset",
            );
            record(&mut dataset, &args);
            crate::fail::or_exit(
                dataset.save(output, Format::Compact),
                "Error saving dataset",
            );
            println!("\nWrote {output}");
        }
    }
}

/// Write one link for each path, and say what was written.
///
/// Every check runs before the first write, so a refusal writes nothing.
fn record<S: Storage + LegislatureReader>(dataset: &mut Dataset<S>, args: &Args) {
    let amendment = find_amendment(dataset, args);
    refuse_unless_adjacent(dataset, args);
    refuse_unchanged_paths(dataset, args);
    for path in &args.paths {
        let link = link_for(&amendment, path, args);
        let id = link.id();
        crate::fail::or_exit(dataset.add_link(link), "Error adding link");
        println!(
            "Recorded link {}  {path}",
            words_to_data::review::short_id(&id)
        );
    }
}

/// The amendment a run names, or a refusal that says how to name one.
fn find_amendment<S: Storage + LegislatureReader>(
    dataset: &Dataset<S>,
    args: &Args,
) -> BillAmendment {
    let bill = crate::fail::or_exit(dataset.get_bill(&args.bill), "Error reading the bill");
    let Some(mut bill) = bill else {
        eprintln!(
            "This dataset holds no bill `{}`, so nothing was written.\n\
             `words_to_data bills {}` lists the bills it holds.",
            args.bill, args.dataset
        );
        std::process::exit(1);
    };
    let Some(amendment) = bill.amendments.remove(&args.amendment) else {
        eprintln!(
            "Bill `{}` holds no amendment `{}`, so nothing was written.\n\
             `words_to_data show-bill {} {}` prints the id of each amendment.",
            args.bill, args.amendment, args.dataset, args.bill
        );
        std::process::exit(1);
    };
    amendment
}

/// Refuse the run unless `--from` and `--to` are two adjacent expressions of
/// one work, in that order.
///
/// A window that skips an expression is two windows. A change seen across it
/// happened in one of them, and a link over the pair would say it happened
/// over both, so no window a link names may hold another expression.
fn refuse_unless_adjacent<S: Storage>(dataset: &Dataset<S>, args: &Args) {
    let (from, to) = (&args.from, &args.to);
    let refuse = |why: String| -> ! {
        eprintln!(
            "{from} -> {to} is not a window of two adjacent expressions, so nothing was written.\n\
             {why}\n\
             `words_to_data expressions {}` lists every expression.",
            args.dataset
        );
        std::process::exit(1);
    };
    if from.work != to.work {
        refuse(format!("{from} and {to} are expressions of two works."));
    }
    let held = crate::fail::or_exit(
        dataset.expressions(&from.work),
        "Error reading the expressions",
    );
    let position = |id: &ExpressionId| held.iter().position(|info| info.id == *id);
    let (Some(earlier), Some(later)) = (position(from), position(to)) else {
        refuse(format!("This dataset does not hold both {from} and {to}."));
    };
    if later <= earlier {
        refuse(format!("{from} is not older than {to}."));
    }
    if later > earlier + 1 {
        let between: Vec<String> = held[earlier + 1..later]
            .iter()
            .map(|info| info.id.to_string())
            .collect();
        refuse(format!("It skips {}.", between.join(", ")));
    }
}

/// Refuse the run when a path it names did not change in the window.
///
/// A change is anything the diff reports at that path: its own words changed,
/// it was added, it was removed, or it moved. The check is at the path itself
/// and not beneath it, because a link points at the changed path
/// (`docs/adr/0013`). A section whose subsection changed did not change itself,
/// and `diff --path` shows the paths beneath it that did.
fn refuse_unchanged_paths<S: Storage>(dataset: &Dataset<S>, args: &Args) {
    let diff = crate::fail::or_exit(
        inspect::diff(dataset, &args.from, &args.to),
        "Error computing the diff",
    );
    let changed = |path: &str| {
        diff.changed_paths.iter().any(|p| p == path)
            || diff.added_paths.iter().any(|p| p == path)
            || diff.removed_paths.iter().any(|p| p == path)
            || diff
                .moved_paths
                .iter()
                .any(|moved| moved.from == path || moved.to == path)
    };
    let unchanged: Vec<&String> = args.paths.iter().filter(|p| !changed(p)).collect();
    if unchanged.is_empty() {
        return;
    }
    for path in &unchanged {
        // Said apart, because the fixes differ: a path that does not exist is
        // mistyped or in another window, and one that did not change is the
        // wrong provision.
        if exists_in_window(dataset, path, args) {
            eprintln!(
                "{path} did not change between {} and {}.",
                args.from, args.to
            );
        } else {
            eprintln!("{path} does not exist at {} or at {}.", args.from, args.to);
        }
    }
    eprintln!(
        "A link can only point at a change the diff produced, so nothing was written.\n\
         `words_to_data diff {} --from {} --to {} --path <section>` lists the paths that changed.",
        args.dataset, args.from, args.to
    );
    std::process::exit(1);
}

/// Whether either end of the window holds a provision at `path`.
fn exists_in_window<S: Storage>(dataset: &Dataset<S>, path: &str, args: &Args) -> bool {
    let holders = crate::fail::or_exit(dataset.find_nodes(path), "Error reading the path");
    holders
        .iter()
        .any(|(expression, _)| *expression == args.from || *expression == args.to)
}

/// The link the batch would write for this amendment and this path.
///
/// Built here rather than through `Link::from_annotation`, because that reads
/// its verification from the annotator's name, and only a `model:` name reads
/// as `MachineSuggested`. A recorder names itself `agent:claude`, and its claim
/// is still a machine's until someone settles it.
fn link_for(amendment: &BillAmendment, path: &str, args: &Args) -> Link {
    Link {
        subject: Target::Change {
            work: args.from.work.clone(),
            path: path.to_string(),
            from_date: args.from.at.clone(),
            to_date: args.to.at.clone(),
        },
        kind: LinkKind::new(LinkKind::AMENDED_BY),
        object: Target::External {
            reference: amendment_reference(&args.bill, &amendment.id),
            display: amendment.amending_text.clone(),
        },
        provenance: Provenance {
            source: args.source.clone(),
            method: None,
            verification: VerificationState::MachineSuggested,
            evidence: Evidence::from_reasoning(Some(args.reason.clone())),
            raw_score: None,
            timestamp: Some(time::OffsetDateTime::now_utc()),
            corroboration: None,
        },
        payload: Some(KindPayload {
            namespace: LinkKind::LEGISLATURE.to_string(),
            value: serde_json::json!({
                "operation": operation_of(amendment),
                "bill_id": args.bill,
                "amendment_id": amendment.id,
                "notes": null,
            }),
        }),
    }
}

/// The action the link records: the one the bill's markup states.
///
/// `amend` is the markup's umbrella word, and most instructions carry it beside
/// the action that says what they do, so it is set aside. When one action is
/// left, that is the action. When none or several are left, `amend` is the
/// honest word, because choosing one of several would claim a reading nobody
/// made.
fn operation_of(amendment: &BillAmendment) -> AmendingAction {
    let mut specific: Vec<AmendingAction> = Vec::new();
    for action in &amendment.action_types {
        if *action != AmendingAction::Amend && !specific.contains(action) {
            specific.push(*action);
        }
    }
    match specific.as_slice() {
        [one] => *one,
        _ => AmendingAction::Amend,
    }
}
