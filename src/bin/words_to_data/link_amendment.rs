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
//! **It names its method.** `--method name@version` goes on the link's
//! provenance, and a run of that method over the window is recorded, because
//! "this reasoning was applied to this window" is what an agent after it can
//! act on (#179, decisions 10 and 11).
//!
//! **It can record that there is no link.** With `--no-link <category>` in place
//! of a window and paths, it records a [`words_to_data::review::NoLink`]: a
//! review whose subject is the amendment itself (ADR 0012, addendum of
//! 2026-09-27). `residue` then reports the amendment as reviewed and not as
//! work. One command for both outcomes, because they share every other
//! argument and every refusal, and an agent working the residue reaches one or
//! the other on each row.
//!
//! **It takes either form the dataset comes in.** A database is changed where it
//! sits; a W2D file is read into memory and written out again (#195).

use clap::{Args as ClapArgs, ValueEnum};
use words_to_data::dataset::{Dataset, ExpressionId};
use words_to_data::inspect;
use words_to_data::legislature::{AmendingAction, BillAmendment};
use words_to_data::link::{
    Evidence, KindPayload, Link, LinkKind, Provenance, Target, VerificationState,
    amendment_reference,
};
use words_to_data::method::Method;
use words_to_data::review::{NoLink, NoLinkCategory, short_id};
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
    #[arg(long, required_unless_present = "no_link")]
    pub from: Option<ExpressionId>,

    /// The next expression of the same work
    #[arg(long, required_unless_present = "no_link")]
    pub to: Option<ExpressionId>,

    /// A path that changed in the window. Give it once for each path
    #[arg(long = "path", required_unless_present = "no_link")]
    pub paths: Vec<String>,

    /// Record that the amendment has no correct link, and why, in place of a
    /// link. It names no window and no path
    #[arg(long, value_enum, conflicts_with_all = ["from", "to", "paths"])]
    pub no_link: Option<NoLinkSaid>,

    /// Who is recording the link: `agent:claude`, `human:jesse`
    #[arg(long)]
    pub source: String,

    /// The reasoning the recorder applied, and its version, as `name@version`:
    /// `resolve-residue@1`. Raise the version when the reasoning changes
    #[arg(long)]
    pub method: Method,

    /// Why the amendment caused these changes, in the recorder's own words
    #[arg(long)]
    pub reason: String,

    /// Where to write a compact JSON dataset. Required for compact JSON, which
    /// is never written back over its input. Ignored for SQLite, which is
    /// changed in place.
    #[arg(long)]
    pub output: Option<String>,
}

impl Args {
    /// The window a link is recorded over.
    ///
    /// `clap` requires both ends unless `--no-link` was given, and a
    /// `--no-link` run never asks for a window, so they cannot be absent.
    fn window(&self) -> (&ExpressionId, &ExpressionId) {
        (
            self.from
                .as_ref()
                .expect("clap requires --from without --no-link"),
            self.to
                .as_ref()
                .expect("clap requires --to without --no-link"),
        )
    }
}

/// Why an amendment has no correct link, as the command line spells it.
///
/// A separate word list from [`NoLinkCategory`], as `settle` keeps one from
/// its verdict, so that `clap`'s derive stays out of the core.
#[derive(Clone, Copy, ValueEnum)]
#[value(rename_all = "snake_case")]
pub enum NoLinkSaid {
    /// The change is in material the dataset does not hold
    NotHeld,
    /// The change takes effect after the newest release point held
    NotYetInCorpus,
    /// The text did not change
    NoChange,
    /// Another reason, which --reason gives
    Other,
}

impl From<NoLinkSaid> for NoLinkCategory {
    fn from(said: NoLinkSaid) -> Self {
        match said {
            NoLinkSaid::NotHeld => Self::NotHeld,
            NoLinkSaid::NotYetInCorpus => Self::NotYetInCorpus,
            NoLinkSaid::NoChange => Self::NoChange,
            NoLinkSaid::Other => Self::Other,
        }
    }
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
                crate::load::load_compact(&args.dataset),
                "Error loading dataset",
            );
            record(&mut dataset, &args);
            crate::fail::or_exit(
                crate::load::save_compact(&dataset, output),
                "Error saving dataset",
            );
            println!("\nWrote {output}");
        }
    }
}

/// Write what the run concluded, and say what was written.
///
/// Every check runs before the first write, so a refusal writes nothing.
fn record<S: Storage + LegislatureReader>(dataset: &mut Dataset<S>, args: &Args) {
    let amendment = find_amendment(dataset, args);
    match args.no_link {
        Some(category) => record_no_link(dataset, &amendment, category.into(), args),
        None => record_links(dataset, &amendment, args),
    }
}

/// Write the reviewer's conclusion that the amendment has no correct link.
///
/// It records no method run. A run says a method was applied to a window, and
/// this conclusion names none.
fn record_no_link<S: Storage>(
    dataset: &mut Dataset<S>,
    amendment: &BillAmendment,
    category: NoLinkCategory,
    args: &Args,
) {
    let conclusion = NoLink {
        bill_id: args.bill.clone(),
        amendment_id: amendment.id.clone(),
        category,
        reviewer: args.source.clone(),
        method: Some(args.method.clone()),
        reasoning: args.reason.clone(),
        at: time::OffsetDateTime::now_utc(),
    };
    let record = conclusion.record(&amendment.amending_text);
    let id = record.id();
    crate::fail::or_exit(dataset.add_link(record), "Error adding the record");
    println!(
        "Recorded no link ({category}) for amendment {}: record {}",
        short_id(&amendment.id),
        short_id(&id)
    );
}

/// Write one link for each path, and a run of the method over the window.
fn record_links<S: Storage>(dataset: &mut Dataset<S>, amendment: &BillAmendment, args: &Args) {
    let (from, to) = args.window();
    refuse_unless_adjacent(dataset, args);
    refuse_unchanged_paths(dataset, args);
    for path in &args.paths {
        let link = link_for(amendment, path, args);
        let id = link.id();
        crate::fail::or_exit(dataset.add_link(link), "Error adding link");
        println!(
            "Recorded link {}  {path}",
            words_to_data::review::short_id(&id)
        );
    }
    // "This reasoning was applied to this window" (#179, decision 11). A run is
    // identified by what it says, so recording the same finding again adds no
    // second run.
    crate::fail::or_exit(
        dataset.record_method_run(args.method.clone(), from, to),
        "Error recording what ran",
    );
    println!(
        "Recorded a run of {} over {} -> {}",
        args.method, from, to.at
    );
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
    let (from, to) = args.window();
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
    let (from, to) = args.window();
    let diff = crate::fail::or_exit(inspect::diff(dataset, from, to), "Error computing the diff");
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
            eprintln!("{path} did not change between {} and {}.", from, to);
        } else {
            eprintln!("{path} does not exist at {} or at {}.", from, to);
        }
    }
    eprintln!(
        "A link can only point at a change the diff produced, so nothing was written.\n\
         `words_to_data diff {} --from {} --to {} --path <section>` lists the paths that changed.",
        args.dataset, from, to
    );
    std::process::exit(1);
}

/// Whether either end of the window holds a provision at `path`.
fn exists_in_window<S: Storage>(dataset: &Dataset<S>, path: &str, args: &Args) -> bool {
    let (from, to) = args.window();
    let holders = crate::fail::or_exit(dataset.find_nodes(path), "Error reading the path");
    holders
        .iter()
        .any(|(expression, _)| *expression == *from || *expression == *to)
}

/// The link the batch would write for this amendment and this path.
///
/// Built here rather than through `Link::from_annotation`, because that reads
/// its verification from the annotator's name, and only a `model:` name reads
/// as `MachineSuggested`. A recorder names itself `agent:claude`, and its claim
/// is still a machine's until someone settles it.
fn link_for(amendment: &BillAmendment, path: &str, args: &Args) -> Link {
    let (from, to) = args.window();
    Link {
        subject: Target::Change {
            work: from.work.clone(),
            path: path.to_string(),
            from_date: from.at.clone(),
            to_date: to.at.clone(),
        },
        kind: LinkKind::new(LinkKind::AMENDED_BY),
        object: Target::External {
            reference: amendment_reference(&args.bill, &amendment.id),
            display: amendment.amending_text.clone(),
        },
        provenance: Provenance {
            source: args.source.clone(),
            method: Some(args.method.clone()),
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
