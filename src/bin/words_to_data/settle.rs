//! `words_to_data settle` — say whether a link is right, and record it.
//!
//! Until this existed nothing could set a verdict on a link a dataset already
//! held. `match-amendments` mentions `AnnotationStatus` once, to hard-code
//! `Pending`, so every stored link read *machine suggested, unconfirmed* for
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
use words_to_data::dataset::{Dataset, Format};
use words_to_data::link::Named;
use words_to_data::review::{self, Review, Verdict};
use words_to_data::storage::{LinkReader, Storage};

#[derive(ClapArgs)]
pub struct Args {
    /// Path to a dataset (compact JSON or SQLite) holding the link to settle
    pub dataset: String,

    /// The link to settle, by any length of its id. `contradictions` prints the
    /// ids, and an ambiguous prefix is refused with the number it matched
    #[arg(long)]
    pub link: String,

    /// What the reviewer found
    #[arg(long, value_enum)]
    pub verdict: Said,

    /// Who is reviewing: `human:jesse`, `model:local`
    #[arg(long)]
    pub reviewer: String,

    /// Why the reviewer says so, in their own words
    #[arg(long)]
    pub reason: String,

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
                Dataset::load(&args.dataset, Format::Compact),
                "Error loading dataset",
            );
            settle(&mut dataset, &args);
            crate::fail::or_exit(
                dataset.save(output, Format::Compact),
                "Error saving dataset",
            );
            println!("\nWrote {output}");
        }
    }
}

/// Find the link, record the review, and say what the dataset now holds about
/// it.
fn settle<S: Storage>(dataset: &mut Dataset<S>, args: &Args) {
    let reviewed = match crate::fail::or_exit(
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
    };

    let reviewed_id = reviewed.id();
    println!("Link {}", review::short_id(&reviewed_id));
    println!(
        "  {} said by {}",
        reviewed.kind.0, reviewed.provenance.source
    );
    println!("  {}", reviewed.subject.name());
    println!("  -> {}", reviewed.object.name());

    // The run's clock. A reviewer cannot be asked for the time, and newest-wins
    // needs one, so a review with no timestamp is never built here and
    // `review::record` refuses one that arrives by another road.
    let review = Review {
        verdict: args.verdict.into(),
        reviewer: args.reviewer.clone(),
        reasoning: Some(args.reason.clone()),
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
    println!("  {}", args.reason);

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
