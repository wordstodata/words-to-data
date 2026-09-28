//! The evidence a link of this method records, read back into its parts.
//!
//! [`super::AmendmentMatch::links`] writes how it decided a link as one text:
//! the address and the evidence that gave it, the window, and how the change
//! was chosen. A reviewer, and a playbook that reviews the weaker kinds of
//! decision first, reads those parts one at a time.
//!
//! The text is written in `mod.rs` and `resolve.rs`, and read here. A change
//! to a sentence there must be made here too:
//! `tests/link_decision_tests.rs` reads back every link the method writes over
//! the committed corpus, and fails on one this does not read.

use serde::Serialize;

/// The parts of one link's recorded evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Recorded {
    /// Where the amendment acts: `/us/usc/t26/s6041(a)`.
    pub address: String,
    /// Which evidence gave the address.
    pub address_source: RecordedSource,
    /// What the evidence says about that source, as recorded. For the OLRC,
    /// the rows of the table, and why the markup gave no section.
    pub address_source_said: String,
    /// The window the change was found in: `uscode/title_26@2025-07-18 to
    /// 2025-07-30`.
    pub window: String,
    /// What the evidence says about the window, as recorded: why it is the
    /// one, and every later window the address changed in too.
    pub window_said: String,
    /// How many changes under the address the window holds, the link's own
    /// among them. One means there was nothing to choose between. `None` for
    /// a link written before the method recorded it.
    pub changes_under_address: Option<usize>,
    /// How the change was chosen from the changes under the address.
    pub chosen: Chosen,
    /// What the evidence says about how the change was chosen, as recorded:
    /// for quoted words, the words.
    pub chosen_said: String,
}

/// How the matcher chose a link's change, in the order of trust its resolve
/// stage applies (`resolve.rs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Chosen {
    /// The change is a renumbering, and the dataset's
    /// `legislature.redesignated_as` link names this amendment.
    Renumbering,
    /// The change shows words the bill quotes.
    QuotedWords,
    /// The change sits inside a provision the quoted words placed.
    InsidePlacedProvision,
    /// No quoted words placed the change, and no other amendment addressed
    /// there takes it.
    Elimination,
}

/// Which evidence gave a recorded address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordedSource {
    /// The bill's markup names the section.
    Markup,
    /// The OLRC's classification table gave the section, because the markup
    /// named none.
    Olrc,
}

impl Recorded {
    /// The parts of `reasoning`, or `None` when it is not the text this
    /// method writes.
    pub fn read(reasoning: &str) -> Option<Self> {
        let (address_part, rest) = reasoning
            .strip_prefix("Address: ")?
            .split_once(". Window: ")?;
        let (address, source_said) = address_part.split_once(", read from ")?;
        let address_source = if source_said.starts_with("the OLRC") {
            RecordedSource::Olrc
        } else {
            RecordedSource::Markup
        };
        let (window_said, change) = rest.split_once(" Change: ")?;
        let (window, window_said) = window_said.split_once(", ")?;
        let (window_said, changes_under_address) = changes_under_address(window_said);
        let chosen_said = match change.rsplit_once(" OLRC: ") {
            Some((said, _olrc)) => said,
            None => change,
        };
        Some(Self {
            address: address.to_string(),
            address_source,
            address_source_said: format!("read from {source_said}"),
            window: window.to_string(),
            window_said,
            changes_under_address,
            chosen: Chosen::of(chosen_said)?,
            chosen_said: chosen_said.to_string(),
        })
    }
}

impl RecordedSource {
    /// The source, as a reader names it.
    pub fn name(self) -> &'static str {
        match self {
            Self::Markup => "the bill's markup",
            Self::Olrc => "the OLRC classification table",
        }
    }
}

impl Chosen {
    /// The kind a recorded sentence names. The sentences are the ones
    /// `resolve.rs` writes for each way a change is found.
    fn of(said: &str) -> Option<Self> {
        let kinds = [
            ("the change is a renumbering", Self::Renumbering),
            (
                "the change shows the words the bill quotes",
                Self::QuotedWords,
            ),
            ("the change sits inside", Self::InsidePlacedProvision),
            (
                "the change is under the address, and no other",
                Self::Elimination,
            ),
        ];
        kinds
            .into_iter()
            .find(|(start, _)| said.starts_with(start))
            .map(|(_, kind)| kind)
    }

    /// The kind, as a reader names it.
    pub fn name(self) -> &'static str {
        match self {
            Self::Renumbering => "as a renumbering the dataset records",
            Self::QuotedWords => "by quoted words",
            Self::InsidePlacedProvision => "inside a provision the quoted words placed",
            Self::Elimination => "by elimination",
        }
    }
}

/// The window's sentences with the count of changes under the address taken
/// out, and the count, when the evidence records one.
fn changes_under_address(window_said: &str) -> (String, Option<usize>) {
    let counted = window_said
        .split_once(" The address holds ")
        .and_then(|(before, rest)| {
            let (count, after) = rest.split_once(" change(s) in this window.")?;
            Some((format!("{before}{after}"), count.parse().ok()?))
        });
    match counted {
        Some((said, count)) => (said, Some(count)),
        None => (window_said.to_string(), None),
    }
}
