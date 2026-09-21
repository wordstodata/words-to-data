//! `words_to_data add-release-points` — put more US Code release points into a
//! dataset that is already there (#180).
//!
//! Until this existed a dataset could not grow: `build-dataset` created, and one
//! more printing of one title meant building everything again, which bought
//! every model call a second time. A release point is not a stored thing — an
//! expression is `(work, date)` — so growth is more keys and no format change
//! (`docs/adr/0003-storage-is-keyed-by-work.md`).
//!
//! **Where the result goes is not the same for the two forms.** A SQLite dataset
//! grows in place, under a transaction. A compact JSON dataset is written whole,
//! so it must be told where to write and is never written back over its input
//! (#186).
//!
//! **It adds release points and runs no step over them.** A window is made here
//! and resolved by `redesignations` and `match-amendments`, which is why the run
//! ends by naming the windows it made and what each one holds.

use std::collections::BTreeMap;

use clap::Args as ClapArgs;
use words_to_data::dataset::{Dataset, ExpressionId, ExpressionPair, Format, adjacent_expressions};
use words_to_data::storage::{DocumentReader, LinkReader, Storage};

use crate::release_points::{self, DEFAULT_MIRROR_INDEX, Missing, ReleaseSource};

#[derive(ClapArgs)]
pub struct Args {
    /// Path to the dataset to add the release points to (compact JSON or SQLite)
    pub dataset: String,

    /// Release-point dates to add (YYYY-MM-DD), comma-separated
    #[arg(long, value_delimiter = ',', required = true)]
    pub uslm_dates: Vec<String>,

    /// Mirror manifest URL
    #[arg(long, default_value = DEFAULT_MIRROR_INDEX)]
    pub mirror_index: String,

    /// Cache directory for downloaded release points
    /// (default: the shared `<user cache dir>/words_to_data`)
    #[arg(long)]
    pub cache_dir: Option<String>,

    /// Read only the cache, and never reach the network
    ///
    /// A run that needs a release point it has not got fails by name instead of
    /// downloading hundreds of megabytes.
    #[arg(long)]
    pub offline: bool,

    /// Where to write a compact JSON dataset. Required for compact JSON, which
    /// is never written back over its input. Ignored for SQLite, which grows in
    /// place.
    #[arg(long)]
    pub output: Option<String>,
}

pub fn run(args: Args) {
    // Where the result goes: `None` is SQLite, which grows in place. Compact
    // JSON must be told, and it is asked before anything is downloaded.
    let output = if crate::load::is_sqlite(&args.dataset) {
        None
    } else {
        Some(crate::load::output_or_refuse(
            &args.dataset,
            args.output.as_deref(),
            "add-release-points",
        ))
    };

    let source = if args.offline {
        ReleaseSource::cached_only(args.cache_dir.as_deref())
    } else {
        crate::fail::or_exit(
            ReleaseSource::from_mirror(&args.mirror_index, args.cache_dir.as_deref()),
            "Error fetching mirror manifest",
        )
    };

    match output {
        None => {
            let mut dataset =
                crate::fail::or_exit(Dataset::open_sqlite(&args.dataset), "Error opening dataset");
            grow(&mut dataset, &source, &args.uslm_dates, &args.dataset);
            println!("\nWrote {}", args.dataset);
        }
        Some(output) => {
            let mut dataset = crate::fail::or_exit(
                Dataset::load(&args.dataset, Format::Compact),
                "Error loading dataset",
            );
            grow(&mut dataset, &source, &args.uslm_dates, output);
            crate::fail::or_exit(
                dataset.save(output, Format::Compact),
                "Error saving dataset",
            );
            println!("\nWrote {output}");
        }
    }
}

/// Add the release points, and report the windows the dataset gained.
///
/// `written` is the file the result lands in, so the steps this run names can be
/// run as they are printed.
fn grow<S: Storage>(
    dataset: &mut Dataset<S>,
    source: &ReleaseSource,
    dates: &[String],
    written: &str,
) {
    let before = windows_of(dataset);

    let added = crate::fail::or_exit(
        release_points::add_all(dataset, source, dates, Missing::Stop),
        "Error adding release points",
    );
    println!(
        "\nAdded {} release point(s): {}",
        added.len(),
        added.join(", ")
    );

    report_windows(dataset, &before, written);
}

/// Name the windows this run made, and say what each one holds.
///
/// A window is made here and resolved elsewhere: `redesignations` and
/// `match-amendments` are their own commands over a named span (decision 9 of
/// #179). A run that added a release point and said nothing more would read as
/// a finished job.
///
/// What a window holds is read from its links, and **nothing is stored**. A
/// dataset keeps no record of which method at which version ran over which
/// window; that record is the schema break in #182. So "no link" is reported as
/// what it is — no step has run, or one ran and found nothing — rather than as
/// the stronger fact this build cannot know.
fn report_windows<S: Storage>(dataset: &Dataset<S>, before: &[ExpressionPair], written: &str) {
    let new: Vec<ExpressionPair> = windows_of(dataset)
        .into_iter()
        .filter(|window| !before.contains(window))
        .collect();

    if new.is_empty() {
        println!(
            "\nNo window is new. Each release point added is the only printing of \
             its work, or the dataset already held it."
        );
        return;
    }

    println!("\n{} window(s) are new:", new.len());
    for (from, to) in &new {
        println!(
            "  {}  {} -> {}  {}",
            from.work,
            from.at,
            to.at,
            held_in(dataset, from, to)
        );
    }

    println!(
        "\nA window holding no link has had no step run over it, or had one that \
         found nothing: a dataset does not record which. To run the steps over a \
         new window:"
    );
    if crate::load::is_sqlite(written) {
        println!("  words_to_data convert-dataset {written}    (both steps need compact JSON)");
    }
    for (from, to) in spans_of(&new) {
        println!(
            "  words_to_data redesignations {written} --bill-id <bill> \
             --between {from} {to}"
        );
        println!("  words_to_data match-amendments {written} --between {from} {to}");
    }
}

/// What a window holds, counted by link kind.
///
/// By kind rather than as a total, because a window with amendment links and no
/// redesignation links has had one step run over it and not the other, and a
/// total hides that (`docs/adr/0002-links-live-in-the-core.md`).
fn held_in<S: Storage>(dataset: &Dataset<S>, from: &ExpressionId, to: &ExpressionId) -> String {
    let links = crate::fail::or_exit(
        dataset.links_for_pair(from, to),
        "Error reading a window's links",
    );
    if links.is_empty() {
        return "no link".to_string();
    }

    let mut by_kind: BTreeMap<String, usize> = BTreeMap::new();
    for link in links {
        *by_kind.entry(link.kind.0).or_default() += 1;
    }
    by_kind
        .into_iter()
        .map(|(kind, count)| format!("{count} {kind}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The distinct date pairs among the windows.
///
/// One `--between` covers every work published on both dates, so a span is
/// named once however many works it spans.
fn spans_of(windows: &[ExpressionPair]) -> Vec<(String, String)> {
    let mut spans: Vec<(String, String)> = windows
        .iter()
        .map(|(from, to)| (from.at.clone(), to.at.clone()))
        .collect();
    spans.sort();
    spans.dedup();
    spans
}

/// Every window the dataset holds: each work's expressions, in neighbouring pairs.
fn windows_of<S: Storage>(dataset: &Dataset<S>) -> Vec<ExpressionPair> {
    crate::fail::or_exit(
        adjacent_expressions(dataset as &dyn DocumentReader),
        "Error listing the dataset's windows",
    )
}
