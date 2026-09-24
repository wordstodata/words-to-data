//! Choosing which bills a command runs over.
//!
//! A run that covers every bill a dataset holds is a job of unknown size.
//! `match-amendments` asks a model about each amendment it finds, and a corpus
//! run showed four bills of five giving nothing while the run covered all of
//! them. Two commands need to make that choice — `score-amendments` and
//! `match-amendments` — and they must make it the same way, so it is made here,
//! beside [`crate::span::Span`], which does the same one level up for windows.

use clap::Args as ClapArgs;
use words_to_data::storage::LegislatureReader;

/// Which bills to work over: every bill the dataset holds, or a named few.
#[derive(ClapArgs)]
pub struct BillSelection {
    /// Bills to work on, e.g. `--bills 119-hr-1,119-hr-42` (default: every bill the dataset holds)
    ///
    /// The same comma-separated form `build-dataset` takes.
    #[arg(long, value_delimiter = ',')]
    pub bills: Vec<String>,
}

impl BillSelection {
    /// Was this run told which bills to cover?
    ///
    /// A command that writes its result beside the dataset must know: a file
    /// that speaks for the whole corpus must not be written from a part of it.
    pub fn narrows(&self) -> bool {
        !self.bills.is_empty()
    }

    /// The bill ids to work over, sorted.
    ///
    /// Refuses a name the dataset does not hold, and names it. A skipped name
    /// would leave a run that covered one bill of the two it was told, said
    /// nothing, and exited zero, which reads as having done the whole job.
    /// [`crate::span::Span::resolve`] reports what it could not cover for the
    /// same reason.
    pub fn resolve<R: LegislatureReader + ?Sized>(&self, reader: &R) -> Vec<String> {
        let mut held = crate::fail::or_exit(reader.list_bill_ids(), "Error listing bills");
        held.sort();

        if !self.narrows() {
            return held;
        }

        let missing: Vec<&str> = self
            .bills
            .iter()
            .filter(|named| !held.iter().any(|id| id == *named))
            .map(String::as_str)
            .collect();
        if !missing.is_empty() {
            crate::fail::refuse(&format!(
                "The dataset holds no bill named {}.\n\
                 The run stops, because it cannot cover a bill that is not there.\n\
                 Run `words_to_data bills <dataset>` to see which bills it holds.",
                missing.join(", ")
            ));
        }

        // Filtering the held ids, and not the named ones, keeps the sorted
        // order whatever order the names were given in, and drops a name given
        // twice.
        held.retain(|id| self.bills.iter().any(|named| named == id));
        held
    }
}
