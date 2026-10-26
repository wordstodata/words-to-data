//! How a command reads a bill, a work, a window or a path it was given.
//!
//! `show-bill` said "Bill not found", `votes` said "No votes for bill" and
//! `residue --bill` listed "0 amendment(s)" for a bill not held (#337).
//! `annotations` then gave "0 annotation(s)" for a bill, a date or a window not
//! held, and a path typed with a trailing `/` matched nothing (#392, #395).
//! Each command that takes one of these reads it here, so every command
//! refuses in the same words and reads a path in the same way.

use words_to_data::dataset::ExpressionId;
use words_to_data::inspect::{
    BillNotHeld, WindowNotHeld, absent_scope, bill_not_held, path_as_held, window_not_held,
};
use words_to_data::query::{LinkQuery, Locator};
use words_to_data::storage::DocumentReader;
use words_to_data::storage::LinkReader;

use crate::load::{OpenDataset, with_dataset};

/// Stop the run with exit 1 when the dataset at `path` holds no bill `bill_id`.
///
/// The notice goes to stderr, where each of these commands puts its other
/// refusals, so `--json` output is never a half answer.
pub fn refuse_unless_bill_held(dataset: &OpenDataset, path: &str, bill_id: &str) {
    let absent = crate::fail::or_exit(
        with_dataset!(dataset, d => bill_not_held(d, bill_id)),
        "Error reading the dataset's bills",
    );
    if let Some(absent) = absent {
        crate::fail::refuse(&bill_notice(&absent, path));
    }
}

/// Stop the run with exit 1 when the dataset holds no bill `bill_id` and no
/// link names it.
///
/// For a command that reads links. A dataset can hold the links of a bill and
/// not the bill: one made from a run's annotations alone holds no bill record.
/// Its links are a true answer, so they are not refused.
pub fn refuse_unless_bill_held_or_linked(dataset: &OpenDataset, path: &str, bill_id: &str) {
    let query = LinkQuery::new()
        .with_object_prefix(words_to_data::link::bill_reference_prefix(bill_id))
        .with_limit(1);
    let linked = crate::fail::or_exit(
        with_dataset!(dataset, d => d.links_matching(&query)),
        "Error reading the dataset's links",
    );
    if linked.total == 0 {
        refuse_unless_bill_held(dataset, path, bill_id);
    }
}

/// What the dataset does not hold, and where to go on from.
fn bill_notice(absent: &BillNotHeld, path: &str) -> String {
    let mut notice = format!("This dataset holds no bill {}.", absent.bill_id);
    if let Some(number) = absent.bill_id.rsplit('-').next()
        && !absent.same_number.is_empty()
    {
        notice.push_str(&format!(
            "\nHeld bills with the number {number}: {}",
            absent.same_number.join(", ")
        ));
    }
    notice.push_str(&format!(
        "\n`words_to_data bills {path}` lists the {} bill(s) it holds.",
        absent.bills_held
    ));
    notice
}

/// Stop the run with exit 1 when the dataset at `path` holds no expression of
/// `work`. The notice is the one `search --work` gives.
pub fn refuse_unless_work_held(dataset: &OpenDataset, path: &str, work: &str) {
    let locator = Locator::new().in_work(work);
    let absent = crate::fail::or_exit(
        with_dataset!(dataset, d => absent_scope(d, &locator)),
        "Error reading the dataset's works",
    );
    if let Some(absent) = absent {
        crate::fail::refuse(&crate::search::scope_notice(&absent, path, false));
    }
}

/// Stop the run with exit 1 unless `--from` and `--to` are two held
/// expressions of one work, the older one first.
pub fn refuse_unless_window_held(dataset: &OpenDataset, from: &ExpressionId, to: &ExpressionId) {
    with_dataset!(dataset, d => refuse_unless_window_held_in(d, from, to));
}

/// [`refuse_unless_window_held`], for a caller that holds a reader rather than
/// an open dataset.
pub fn refuse_unless_window_held_in<R: DocumentReader + ?Sized>(
    reader: &R,
    from: &ExpressionId,
    to: &ExpressionId,
) {
    let fault = crate::fail::or_exit(
        window_not_held(reader, from, to),
        "Error reading the dataset's expressions",
    );
    if let Some(fault) = fault {
        crate::fail::refuse(&window_notice(&fault));
    }
}

/// What is wrong with the window, and what to give instead.
fn window_notice(fault: &WindowNotHeld) -> String {
    match fault {
        WindowNotHeld::TwoWorks { from, to } => format!(
            "--from {from} and --to {to} are expressions of two works. \
             A window is two expressions of one work."
        ),
        WindowNotHeld::Expression {
            expression,
            work,
            held_dates,
        } if held_dates.is_empty() => format!(
            "This dataset holds no expression {expression}.\n\
             It holds no expression of {work}. `words_to_data expressions` lists the works it holds."
        ),
        WindowNotHeld::Expression {
            expression,
            work,
            held_dates,
        } => format!(
            "This dataset holds no expression {expression}.\nHeld dates of {work}: {}",
            held_dates.join(", ")
        ),
        WindowNotHeld::NotOlder { from, to } => format!(
            "--from {from} is not older than --to {to}. Give the older expression as --from."
        ),
    }
}

/// A path argument in the spelling the dataset holds: no trailing `/`, and a
/// section number's hyphen read as the dash the dataset prints
/// ([`path_as_held`]).
///
/// Says so on stderr when it reads a path in another spelling than the one
/// typed, so a reader of the answer knows which path it is about.
pub fn path_argument<R: DocumentReader + ?Sized>(reader: &R, typed: &str) -> String {
    let held = crate::fail::or_exit(path_as_held(reader, typed), "Error checking the path");
    if held != typed {
        eprintln!("Reading {typed} as {held}, the spelling this dataset holds.");
    }
    held
}
