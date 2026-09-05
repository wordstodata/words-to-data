//! Choosing which expression pairs a command runs over.
//!
//! A diff is between two expressions of one work, so a job that used to be one
//! call over a global tree is now one call per document. Two commands need to
//! make that choice — `score-amendments` and `match-amendments` — and they must
//! make it the same way, so it is made here.

use clap::Args as ClapArgs;
use words_to_data::dataset::{ExpressionId, ExpressionPair, works_between};
use words_to_data::storage::DocumentReader;

/// Check a `--between` date before any work starts.
///
/// `--from` and `--to` get this for free, because `ExpressionId` parses its
/// date. Without the same check here a typo is not an error: no work is held
/// on `not-a-date`, so every work is skipped, the run reports covering nothing,
/// and it exits zero. A pipeline would carry straight on past a job that never
/// happened.
fn publication_date(text: &str) -> Result<String, String> {
    words_to_data::date::date_str_to_date(text)
        .map(|_| text.to_string())
        .map_err(|_| format!("`{text}` is not a date. Write it as YYYY-MM-DD, such as 2025-07-18."))
}

/// Which expressions to work over: one named pair, or every work spanning two
/// dates.
#[derive(ClapArgs)]
pub struct Span {
    /// Every work published on both dates, e.g. `--between 2025-07-18 2025-07-30`
    #[arg(
        long,
        num_args = 2,
        value_names = ["FROM", "TO"],
        value_parser = publication_date,
        conflicts_with_all = ["from", "to"],
        required_unless_present = "from"
    )]
    pub between: Option<Vec<String>>,

    /// Older expression, e.g. `uscode/title_26@2025-07-18`
    #[arg(long, requires = "to")]
    pub from: Option<ExpressionId>,

    /// Newer expression of the same work
    #[arg(long, requires = "from")]
    pub to: Option<ExpressionId>,
}

impl Span {
    /// The pairs to run over, and a note of anything this span cannot cover.
    ///
    /// Prints what it skipped rather than returning a shorter list quietly: a
    /// corpus run that covered forty of fifty-eight works and said nothing
    /// would read as having done the whole job.
    pub fn resolve<R: DocumentReader + ?Sized>(&self, reader: &R) -> Vec<ExpressionPair> {
        match (&self.between, &self.from, &self.to) {
            (Some(dates), _, _) => {
                let (from, to) = (&dates[0], &dates[1]);
                let between =
                    crate::fail::or_exit(works_between(reader, from, to), "Error listing works");

                if !between.skipped.is_empty() {
                    eprintln!(
                        "Skipping {} work(s) not held at both {from} and {to}: {}",
                        between.skipped.len(),
                        preview(&between.skipped)
                    );
                }
                if between.pairs.is_empty() {
                    eprintln!("No work is held at both {from} and {to}; nothing to do.");
                }
                between.pairs
            }
            (None, Some(from), Some(to)) => vec![(from.clone(), to.clone())],
            // clap's `required_unless_present` and `requires` between them make
            // the remaining combinations unreachable.
            _ => unreachable!("clap requires --between or --from/--to"),
        }
    }
}

/// The first few names, and how many more there are.
fn preview<T: std::fmt::Display>(items: &[T]) -> String {
    const SHOWN: usize = 5;
    let shown: Vec<String> = items.iter().take(SHOWN).map(T::to_string).collect();
    match items.len().checked_sub(SHOWN) {
        Some(rest) if rest > 0 => format!("{}, and {rest} more", shown.join(", ")),
        _ => shown.join(", "),
    }
}
