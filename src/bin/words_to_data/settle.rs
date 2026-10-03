//! `words_to_data settle` — say whether a link is right, and record it.
//!
//! Until this existed nothing could set a verdict on a link a dataset already
//! held. `match-amendments` (removed in #252) mentioned `AnnotationStatus` once,
//! to hard-code `Pending`, so every stored link read *machine suggested, unconfirmed* for
//! ever, and roughly fifty links in the real corpus were known to be wrong with
//! no way to say so (#227).
//!
//! **The link reviewed is never touched.** A review is its own link, with its
//! own reviewer, reason and time, so several reviewers may argue about one link
//! and every argument survives. That is what makes a bad review safe: a poor
//! suggestion is *visible* rather than destructive
//! (`docs/adr/0012-a-review-is-its-own-link-and-a-reader-reports-the-record.md`).
//!
//! **One judgement per run.** A `--from <file>` batch form is pure CLI surface
//! with no effect on what is recorded, so it waits until settling by hand hurts.
//!
//! **It takes either form the dataset comes in.** A database is changed where it
//! sits; a W2D file is read into memory and written out again (#195).

use clap::{Args as ClapArgs, ValueEnum};
use words_to_data::dataset::Dataset;
use words_to_data::inspect::{self, EvidenceWords};
use words_to_data::legislature::evidence_matching::{Recorded, RecordedSource};
use words_to_data::legislature::outdated::outdated_link;
use words_to_data::link::{Link, Named, Target};
use words_to_data::review::{self, Review, Verdict};
use words_to_data::storage::{LinkReader, Storage};

use crate::load::with_dataset;
use crate::ui;

#[derive(ClapArgs)]
pub struct Args {
    /// Path to a dataset (compact JSON or SQLite) holding the link to settle
    pub dataset: String,

    /// The link to settle, by any length of its id. `contradictions` prints the
    /// ids, and an ambiguous prefix is refused with the number it matched
    #[arg(long)]
    pub link: String,

    /// What the reviewer found. Not needed with `--explain`
    #[arg(long, value_enum, required_unless_present = "explain")]
    pub verdict: Option<Said>,

    /// Who is reviewing: `human:jesse`, `model:local`. Not needed with `--explain`
    #[arg(long, required_unless_present = "explain")]
    pub reviewer: Option<String>,

    /// Why the reviewer says so, in their own words. Not needed with `--explain`
    #[arg(long, required_unless_present = "explain")]
    pub reason: Option<String>,

    /// Show the link and the words at its two ends, and record nothing
    ///
    /// Judging a link needs its evidence, and a reviewer must be able to read
    /// that before deciding. Looking is not judging, so this writes nothing and
    /// needs no verdict
    #[arg(long)]
    pub explain: bool,

    /// Where to write a compact JSON dataset. Required for compact JSON, which
    /// is never written back over its input. Ignored for SQLite, which is
    /// changed in place.
    #[arg(long)]
    pub output: Option<String>,
}

/// The verdict as the command line spells it.
///
/// A separate word list from [`Verdict`] so that `clap`'s derive stays out of
/// the core. The core's enum is not a CLI surface, and a rename there must not
/// silently rename a flag a person has in a script.
#[derive(Clone, Copy, ValueEnum)]
pub enum Said {
    /// The link is right
    Confirmed,
    /// The link is wrong. Settled, and settled against it
    Refuted,
    /// The reviewer objects, and it is not settled
    Disputed,
}

impl From<Said> for Verdict {
    fn from(said: Said) -> Self {
        match said {
            Said::Confirmed => Self::Confirmed,
            Said::Refuted => Self::Refuted,
            Said::Disputed => Self::Disputed,
        }
    }
}

pub fn run(args: Args) {
    // Looking writes nothing, so it never chooses an output and never refuses
    // for want of one. A compact dataset can be read where it sits.
    if args.explain {
        let ds = crate::fail::or_exit(crate::load::open(&args.dataset), "Error opening dataset");
        with_dataset!(ds, d => explain(&d, &args));
        return;
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
            "settle",
        ))
    };

    match output {
        None => {
            let mut dataset =
                crate::fail::or_exit(Dataset::open_sqlite(&args.dataset), "Error opening dataset");
            settle(&mut dataset, &args);
            println!("\nWrote {}", args.dataset);
        }
        Some(output) => {
            let mut dataset = crate::fail::or_exit(
                crate::load::load_compact(&args.dataset),
                "Error loading dataset",
            );
            settle(&mut dataset, &args);
            crate::fail::or_exit(
                crate::load::save_compact(&dataset, output),
                "Error saving dataset",
            );
            println!("\nWrote {output}");
        }
    }
}

/// Show the link and the words at its two ends, and record nothing.
fn explain<S: Storage>(dataset: &Dataset<S>, args: &Args) {
    let reviewed = find_link(dataset, args);
    describe(&reviewed);
    print_evidence(dataset, &reviewed);

    // What anyone has already said about it. A reviewer about to judge a link
    // needs to know it has been judged, or they will publish over a verdict
    // without meaning to.
    let records = crate::fail::or_exit(
        dataset.reviews_of(&reviewed.id()),
        "Error reading the link's reviews",
    );
    match review::newest(&records) {
        None => println!("\n{}", ui::quiet("No reviews yet.")),
        Some(winning) => {
            println!(
                "\n{} review record(s). Readers report the newest: {} by {} on {}.",
                records.len(),
                winning.verdict,
                winning.reviewer,
                winning.at.date()
            );
        }
    }
}

/// The link a run names, or a refusal that says how to name one.
fn find_link<S: Storage>(dataset: &Dataset<S>, args: &Args) -> Link {
    match crate::fail::or_exit(
        dataset.link_by_id_prefix(&args.link),
        "Error reading the dataset's links",
    ) {
        Named::One(link) => link,
        Named::Unknown => {
            eprintln!(
                "No link in this dataset has an id starting with `{}`.\n\
                 A link id is a hash of what the link says, so it is the same in every \
                 build of the dataset. `words_to_data contradictions {}` prints the ids.",
                args.link, args.dataset
            );
            std::process::exit(1);
        }
        Named::Ambiguous(matched) => {
            eprintln!(
                "`{}` names {matched} links in this dataset. Type more of the id.",
                args.link
            );
            std::process::exit(1);
        }
    }
}

/// The link itself: its id, what it says, and who said it.
fn describe(reviewed: &Link) {
    println!(
        "{} {}",
        ui::heading("Link"),
        ui::figure(review::short_id(&reviewed.id()))
    );
    println!(
        "  {} said by {}",
        reviewed.kind.0,
        ui::quiet(&reviewed.provenance.source)
    );
    println!("  {}", reviewed.subject.name());
    println!("  -> {}", reviewed.object.name());
}

/// How the link was made: who made it, by which method at which version.
///
/// A reviewer weighs a link by how it was decided, so this is said before the
/// words at its ends.
fn print_how_made<S: Storage>(dataset: &Dataset<S>, reviewed: &Link) {
    let made = crate::fail::or_exit(
        inspect::how_made(dataset, reviewed),
        "Error reading how the link was made",
    );
    println!("\n{}", ui::heading("How it was made:"));
    println!("  Source: {}", made.source);
    println!(
        "  Method: {}",
        made.method.as_deref().unwrap_or("not recorded")
    );
    match (&made.recorded, &made.reasoning) {
        (Some(recorded), _) => print_recorded(recorded),
        // Evidence another method wrote: a model's reasoning, or an agent's
        // reason. It has no parts to show, so it is shown as it was written.
        (None, Some(text)) => {
            println!("  Reasoning:");
            print_wrapped(text, "    ");
        }
        (None, None) => println!("  Reasoning: not recorded"),
    }
    // A model's link names the model that answered, and the reply it answered
    // with, which the dataset keeps once under the hash of its text (#58).
    if let Some(model) = &made.model {
        println!("  Model: {model}");
    }
    if let Some(reply) = &made.reply {
        println!("  Reply: {reply}");
    }
    if made.causes > 1 {
        println!(
            "  Causes: one of {} amendments linked to this change",
            made.causes
        );
    }
    print_outdated(dataset, reviewed);
}

/// Whether the current version of the link's batch method no longer makes it
/// (#185). Said only when it is so: most links are current.
fn print_outdated<S: Storage>(dataset: &Dataset<S>, reviewed: &Link) {
    let outdated = crate::fail::or_exit(
        outdated_link(dataset, reviewed),
        "Error reading whether the link is outdated",
    );
    if let Some(outdated) = outdated {
        println!(
            "  Outdated: made by @{}. This bill's links in this window were re-made by @{}, \
             and this one was not",
            outdated.made_by.version, outdated.remade_by.version
        );
    }
}

/// The parts of the evidence `link-by-evidence` records: the address and its
/// source, the window, and how the change was chosen.
fn print_recorded(recorded: &Recorded) {
    println!(
        "  Address: {}, from {}",
        recorded.address,
        recorded.address_source.name()
    );
    if recorded.address_source == RecordedSource::Olrc {
        print_wrapped(&recorded.address_source_said, "    ");
    }
    println!("  Window: {}", recorded.window);
    print_wrapped(&recorded.window_said, "    ");
    if let Some(count) = recorded.changes_under_address {
        println!("  Changes under the address: {count}");
    }
    println!("  Chosen: {}", recorded.chosen.name());
    print_wrapped(&recorded.chosen_said, "    ");
}

/// `text` in lines of fourteen words, each after `indent`.
fn print_wrapped(text: &str, indent: &str) {
    for line in text.split_whitespace().collect::<Vec<_>>().chunks(14) {
        println!("{indent}{}", line.join(" "));
    }
}

/// What a verdict rests on: how the link was made, the words at its ends,
/// and the OLRC's classification of the section they sit in.
fn print_evidence<S: Storage>(dataset: &Dataset<S>, reviewed: &Link) {
    print_how_made(dataset, reviewed);
    print_words_at_the_ends(dataset, reviewed);
    print_olrc(dataset, reviewed);
}

/// The OLRC's classifications of the section the link's subject belongs to.
///
/// The authority's own statement of which law changed the section, which is
/// the strongest corroboration a reviewer has (#259). Said only when there is
/// one: most links name a section the table does not list, and a line saying
/// so would be read as a statement that nothing changed.
fn print_olrc<S: Storage>(dataset: &Dataset<S>, reviewed: &Link) {
    let Some(path) = reviewed.subject.path() else {
        return;
    };
    let rows = crate::fail::or_exit(
        inspect::olrc_classifications(dataset, path),
        "Error reading the OLRC's classifications",
    );
    if let Some(first) = rows.first() {
        crate::path::print_classifications(&first.section, &rows);
    }
}

/// The words at the link's ends: both, or the one the window holds.
fn print_words_at_the_ends<S: Storage>(dataset: &Dataset<S>, reviewed: &Link) {
    let evidence = crate::fail::or_exit(
        inspect::link_evidence(dataset, reviewed),
        "Error reading the words at the link's ends",
    );
    let Some(evidence) = evidence else {
        println!(
            "\n  This dataset holds the provision at neither end, so there are no words to show."
        );
        return;
    };
    // What the bill instructed, where the object carries it. Said before the
    // diff, because a reviewer reads the instruction and then checks whether the
    // words moved the way it said.
    if let Some(instructed) = &evidence.object_text {
        println!("\n{}", ui::heading("What the object says:"));
        print_wrapped(instructed, "  ");
    }
    match &evidence.words {
        EvidenceWords::BothEnds { from, to, changes } => print_changes(from, to, changes),
        // A provision at one end only has nothing to compare against, so its
        // words are the evidence (#259).
        EvidenceWords::Added {
            expression,
            path,
            words,
        } => {
            println!("\nThe provision is new in the window. It is at {expression} {path}.");
            let rest = every_word(path, reviewed);
            crate::path::print_words("added", path, words, "  ", &rest);
        }
        EvidenceWords::Removed {
            expression,
            path,
            words,
        } => {
            println!("\nThe provision is gone in the window. It was at {expression} {path}.");
            let rest = every_word(path, reviewed);
            crate::path::print_words("removed", path, words, "  ", &rest);
        }
    }
}

/// Where a reviewer reads every word a screen left out: `path` over the
/// link's own window, which carries them all in `--json`.
///
/// Only a link whose subject is a change has words at one end, so the window is
/// always there to name.
fn every_word(path: &str, reviewed: &Link) -> String {
    let window = match &reviewed.subject {
        Target::Change {
            work,
            from_date,
            to_date,
            ..
        } => format!(" --from {work}@{from_date} --to {work}@{to_date}"),
        _ => String::new(),
    };
    format!("`words_to_data path <dataset> {path}{window} --json` carries every field in full.")
}

/// How the fields differ between a link's two ends.
///
/// An empty change list is said in words rather than left blank, and it is said
/// **narrowly**. It means no field differs on this provision itself, which is not
/// the same as nothing having changed: only the node at the subject's path is
/// compared, so an amendment that rewrote a child leaves this list empty. A
/// reader who saw nothing printed would read it as missing data, and one who saw
/// "untouched" would read it as a guarantee about the subtree that was never
/// checked.
fn print_changes(from: &str, to: &str, changes: &[inspect::PathFieldChange]) {
    println!("\n{}", ui::heading("The words at its two ends:"));
    println!("  {from}");
    println!("  {to}");
    if changes.is_empty() {
        println!("  No field differs on this provision itself.");
        println!("  Only this provision is compared, so a change beneath it is not read here.");
        return;
    }
    for change in changes {
        println!(
            "  {}: {:?} -> {:?}",
            change.field, change.old_value, change.new_value
        );
    }
}

/// Find the link, record the review, and say what the dataset now holds about
/// it.
fn settle<S: Storage>(dataset: &mut Dataset<S>, args: &Args) {
    let reviewed = find_link(dataset, args);
    let reviewed_id = reviewed.id();
    describe(&reviewed);
    print_evidence(dataset, &reviewed);

    // The run's clock. A reviewer cannot be asked for the time, and newest-wins
    // needs one, so a review with no timestamp is never built here and
    // `review::record` refuses one that arrives by another road.
    //
    // `clap` requires all three unless `--explain` was given, and `--explain`
    // never reaches here, so these cannot be absent.
    let review = Review {
        verdict: args
            .verdict
            .expect("clap requires a verdict without --explain")
            .into(),
        reviewer: args
            .reviewer
            .clone()
            .expect("clap requires a reviewer without --explain"),
        reasoning: Some(
            args.reason
                .clone()
                .expect("clap requires a reason without --explain"),
        ),
        at: time::OffsetDateTime::now_utc(),
    };
    crate::fail::or_exit(
        review::record(dataset, review.about(&reviewed)),
        "Error recording the review",
    );

    println!(
        "\nRecorded a review: {} by {} on {}.",
        review.verdict,
        review.reviewer,
        review.at.date()
    );
    println!("  {}", review.reasoning.as_deref().unwrap_or_default());

    // What the dataset now holds about this link, and which record a reader will
    // report. Nothing was replaced: an earlier verdict, from this reviewer or
    // any other, is still there. Saying so is the point — a reviewer who thought
    // they had overwritten a record would be wrong about the file they hold.
    let records = crate::fail::or_exit(
        dataset.reviews_of(&reviewed_id),
        "Error reading the link's reviews",
    );
    println!(
        "\nThis link now holds {} review record(s), and every one of them is kept.",
        records.len()
    );
    if let Some(winning) = review::newest(&records) {
        println!(
            "Readers report the newest: {} by {} on {}.",
            winning.verdict,
            winning.reviewer,
            winning.at.date()
        );
    }
    if records.len() > 1 {
        println!("A verdict is corrected by publishing over it, never by removing it.");
        println!("  See docs/adr/0005-evidence-is-stored-once-and-never-deleted.md");
    }
}
