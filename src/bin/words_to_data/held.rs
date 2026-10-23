//! The refusal a command gives for a bill the dataset does not hold (#337).
//!
//! `show-bill` said "Bill not found", `votes` said "No votes for bill" and
//! `residue --bill` listed "0 amendment(s)". Each command that takes one bill
//! now asks [`bill_not_held`] and refuses in the same words.

use words_to_data::inspect::{BillNotHeld, bill_not_held};

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
