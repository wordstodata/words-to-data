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
    BillNotHeld, WindowNotHeld, absent_path, absent_scope, bill_not_held, path_as_held,
    window_not_held,
};
use words_to_data::query::{LinkQuery, Locator};
use words_to_data::storage::DocumentReader;
use words_to_data::storage::{LegislatureReader, LinkReader};

use crate::load::{OpenDataset, with_dataset};

/// Stop the run with exit 1 when the dataset at `path` holds no bill `bill_id`.
///
/// The notice goes to stderr, where each of these commands puts its other
/// refusals, so `--json` output is never a half answer.
pub fn refuse_unless_bill_held(dataset: &OpenDataset, path: &str, bill_id: &str) {
    with_dataset!(dataset, d => refuse_unless_bill_held_in(d, path, bill_id));
}

/// [`refuse_unless_bill_held`], for a caller that holds a reader rather than
/// an open dataset.
pub fn refuse_unless_bill_held_in<R: LegislatureReader + ?Sized>(
    reader: &R,
    path: &str,
    bill_id: &str,
) {
    let absent = crate::fail::or_exit(
        bill_not_held(reader, bill_id),
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

/// Stop the run with exit 1 when no expression in the dataset at `path` is
/// dated `date`. The notice is the one `search --at` gives.
pub fn refuse_unless_date_held_in<R: DocumentReader + ?Sized>(reader: &R, path: &str, date: &str) {
    let locator = Locator::new().in_window(date, date);
    let absent = crate::fail::or_exit(
        absent_scope(reader, &locator),
        "Error reading the dataset's expressions",
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

/// The full id of the one amendment of bill `bill_id` whose id starts with
/// `typed`, from the ids the bill holds. Stops the run with exit 1 when no id
/// starts with it, or when more than one does.
///
/// `annotations` and `amendment-addresses` print the first 12 characters of an
/// id, so a command takes any start of one, as `settle` takes a link's. When
/// no id of the bill starts with it, the notice names each other bill that
/// holds such an amendment: the id is often right and the bill wrong.
pub fn amendment_id_argument<'a, R: LegislatureReader + ?Sized>(
    reader: &R,
    held: impl IntoIterator<Item = &'a String>,
    path: &str,
    bill_id: &str,
    typed: &str,
) -> String {
    let matched: Vec<&String> = held
        .into_iter()
        .filter(|id| id.starts_with(typed))
        .collect();
    match matched.as_slice() {
        [one] => one.to_string(),
        [_, _, ..] => crate::fail::refuse(&format!(
            "`{typed}` names {} amendments of bill {bill_id}. Type more of the id.",
            matched.len()
        )),
        [] => {
            let mut notice = format!("Bill {bill_id} holds no amendment {typed}.");
            for other in bills_with_amendment(reader, bill_id, typed) {
                notice.push_str(&format!(
                    "\nBill {other} holds an amendment whose id starts with {typed}."
                ));
            }
            notice.push_str(&format!(
                "\n`words_to_data show-bill {path} {bill_id}` prints the id of each amendment."
            ));
            crate::fail::refuse(&notice)
        }
    }
}

/// Each bill other than `bill_id` with an amendment whose id starts with
/// `typed`.
fn bills_with_amendment<R: LegislatureReader + ?Sized>(
    reader: &R,
    bill_id: &str,
    typed: &str,
) -> Vec<String> {
    let bill_ids =
        crate::fail::or_exit(reader.list_bill_ids(), "Error reading the dataset's bills");
    bill_ids
        .into_iter()
        .filter(|other| other != bill_id)
        .filter(|other| {
            let bill = crate::fail::or_exit(reader.get_bill(other), "Error reading a bill");
            bill.is_some_and(|bill| bill.amendments.keys().any(|id| id.starts_with(typed)))
        })
        .collect()
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

/// Stop the run with exit 1 when no expression the `locator` reads holds its
/// path, or a path beneath it.
///
/// For a command that found nothing at that path: a link or a citation can
/// name a path that no expression holds, and what it found is a true answer,
/// so the caller asks only when it found nothing. "0 annotation(s)" over a
/// guessed path, exit 0, read as an answer about a held path (#305, #425).
///
/// `scope` says which expressions were read, as words that follow "No
/// expression": "in the dataset", or "of <work> from <date> to <date>".
pub fn refuse_unless_path_held_in<R: DocumentReader + ?Sized>(
    reader: &R,
    locator: &Locator,
    exact: bool,
    scope: &str,
) {
    let absent = crate::fail::or_exit(absent_path(reader, locator), "Error checking the path");
    if let Some(absent) = absent {
        crate::fail::refuse(&crate::annotations::absent_notice(&absent, exact, scope));
    }
}

/// The words that say which expressions a window reads, for
/// [`refuse_unless_path_held_in`]: "in the dataset" with no window.
pub fn scope_of(from: Option<&ExpressionId>, to: Option<&ExpressionId>) -> String {
    match (from, to) {
        (Some(from), Some(to)) => format!("of {} from {} to {}", from.work, from.at, to.at),
        _ => "in the dataset".to_string(),
    }
}

/// An id argument (a link's or an amendment's, or the start of one) in the
/// spelling the dataset holds: an id is lower-case hex, and `8348E252` names
/// the same id as `8348e252` (#424). A `clap` value parser.
pub fn id_argument(typed: &str) -> Result<String, std::convert::Infallible> {
    Ok(typed.to_lowercase())
}
