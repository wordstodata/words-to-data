//! Every amendment of a public law that no `legislature.amended_by` link names,
//! with the stage and the reason it stopped (#251).
//!
//! Stage 5 of
//! `docs/adr/0013-matching-is-evidence-first-and-the-batch-calls-no-model.md`.
//! This is the list an agent works from.
//!
//! **An amendment is unlinked when no link names it, from any source.** A link
//! names an amendment by its object reference,
//! `legislature.amendment:<bill>:<amendment id>`
//! ([`crate::link::amendment_reference`]). The batch, the older model route and
//! the agent door all write that reference, so one link from any of them takes
//! the amendment off the list.
//!
//! **What it knows comes from the matcher.** Each unlinked amendment is
//! described by the answer [`crate::legislature::evidence_matching`] gives for
//! it: the stage, the reason, the window and the changes under the address.
//! Nothing here matches anything again.
//!
//! **Nothing is stored.** The list is derived every time it is asked for,
//! because a stored list goes false the moment someone links an amendment
//! (`docs/adr/0007-a-record-is-what-was-said-everything-else-is-derived.md`).

use std::collections::BTreeSet;

use serde::Serialize;

use crate::dataset::{Dataset, DatasetError};
use crate::legislature::evidence_matching::{Outcome, Residue, match_by_evidence};
use crate::link::{LinkKind, amendment_reference, bill_reference_prefix};
use crate::query::LinkQuery;
use crate::storage::{LegislatureReader, LinkReader, Storage};
use crate::uslm::amendment_address::AmendmentAddress;

/// One amendment no link names, and what is known about it.
#[derive(Debug, Clone, Serialize)]
pub struct Unlinked {
    /// The bill, as the dataset names it: `119-hr-1`.
    pub bill_id: String,
    /// The public law the bill became: `119-21`.
    pub public_law: String,
    /// The amendment, by the content hash its bill's node carries.
    pub amendment_id: String,
    /// The amendment's words, as the bill states them.
    pub amending_text: String,
    /// Where the bill's markup says the amendment acts.
    pub address: AmendmentAddress,
    /// Whether it is work for an agent.
    pub category: Category,
    /// The stage it stopped at, why, and where to look.
    #[serde(flatten)]
    pub residue: Residue,
}

/// Whether an unlinked amendment is work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    /// Something is left to resolve.
    Work,
    /// The address is in the Code the dataset holds and nothing under it
    /// changed after the law's enactment ([`Residue::quiet`]).
    Quiet,
}

impl Category {
    fn of(residue: &Residue) -> Self {
        if residue.quiet {
            Self::Quiet
        } else {
            Self::Work
        }
    }
}

/// Every amendment of the public law `bill` became, or of every public law the
/// dataset holds when `bill` is `None`, that no `amended_by` link names.
///
/// In the order each bill states its amendments.
pub fn unlinked_amendments<S: Storage + LegislatureReader>(
    dataset: &Dataset<S>,
    bill: Option<&str>,
) -> Result<Vec<Unlinked>, DatasetError> {
    let linked = linked_amendments(dataset, bill)?;
    let found = match_by_evidence(dataset)?;
    let mut unlinked = Vec::new();
    for amendment in found.matches {
        if bill.is_some_and(|bill| bill != amendment.bill_id)
            || linked.contains(&amendment_reference(
                &amendment.bill_id,
                &amendment.amendment_id,
            ))
        {
            continue;
        }
        if let Outcome::Residue(residue) = amendment.outcome {
            unlinked.push(Unlinked {
                bill_id: amendment.bill_id,
                public_law: amendment.public_law,
                amendment_id: amendment.amendment_id,
                amending_text: amendment.amending_text,
                address: amendment.address,
                category: Category::of(&residue),
                residue,
            });
        }
    }
    Ok(unlinked)
}

/// The object reference of every amendment an `amended_by` link names.
fn linked_amendments<S: Storage>(
    dataset: &Dataset<S>,
    bill: Option<&str>,
) -> Result<BTreeSet<String>, DatasetError> {
    let mut query = LinkQuery::new().of_kind(LinkKind::AMENDED_BY);
    if let Some(bill) = bill {
        query = query.with_object_prefix(bill_reference_prefix(bill));
    }
    Ok(dataset
        .links_matching(&query)?
        .rows
        .into_iter()
        .map(|link| link.object.name())
        .collect())
}
