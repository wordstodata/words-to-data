//! Each amendment of a public law, linked to the change it made from evidence
//! the dataset already holds, with no model call (#250).
//!
//! Stages 3 and 4 of
//! `docs/adr/0013-matching-is-evidence-first-and-the-batch-calls-no-model.md`:
//!
//! 1. **Address.** The markup resolver ([`crate::uslm::amendment_address`])
//!    says which section, and which provision below it, an instruction acts
//!    on. No address, and the amendment stops here.
//! 2. **Window.** The first window of the Code after the law's enactment date
//!    in which something under the address changed. The enactment date is the
//!    date of the law's stored expression, such as
//!    `publiclawdocument_119-21@2025-07-04`.
//! 3. **Resolve.** The change under the address in that window.
//!
//! **Nothing here is stored.** The answer for each amendment, linked or
//! stopped with the stage and the reason, is derived from the dataset every
//! time it is asked for (`docs/adr/0007-a-record-is-what-was-said-everything-else-is-derived.md`).
//! A command writes the links; the reason an amendment stopped stays
//! computable for the residue report (#251).

use std::collections::BTreeMap;

use serde::Serialize;

use crate::dataset::{Dataset, DatasetError, ExpressionId, WorkId};
use crate::diff::TreeDiff;
use crate::document::DocumentNode;
use crate::legislature::redesignation::{SectionIndex, walk_down};
use crate::storage::{LegislatureReader, Storage};
use crate::uslm::amendment_address::{AmendmentAddress, addresses_in};

/// What the matcher found for one amendment.
#[derive(Debug, Clone, Serialize)]
pub struct AmendmentMatch {
    /// The bill, as the dataset names it: `119-hr-1`.
    pub bill_id: String,
    /// The amendment, by the content hash its bill's node carries.
    pub amendment_id: String,
    /// Where the bill's markup says the amendment acts.
    pub address: AmendmentAddress,
    /// Linked, or stopped with a reason.
    pub outcome: Outcome,
}

/// Linked to changes, or residue.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Linked(Linked),
    Residue(Residue),
}

/// The changes one amendment made, in the window it made them in.
#[derive(Debug, Clone, Serialize)]
pub struct Linked {
    /// The older expression of the window.
    pub from: ExpressionId,
    /// The newer expression of the window.
    pub to: ExpressionId,
    /// Each changed path the amendment caused, in document order.
    pub paths: Vec<String>,
}

/// Why an amendment was not linked.
#[derive(Debug, Clone, Serialize)]
pub struct Residue {
    /// The stage it stopped at.
    pub stage: Stage,
    /// Why, in words a reviewer can act on.
    pub reason: String,
}

/// The stage of the method at which an amendment stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    /// The bill's markup gives no address.
    Address,
    /// No window holds a change under the address.
    Window,
}

/// What the matcher finds for every amendment of every public law the dataset
/// holds, in the order each bill states them.
pub fn match_by_evidence<S: Storage + LegislatureReader>(
    dataset: &Dataset<S>,
) -> Result<Vec<AmendmentMatch>, DatasetError> {
    let stated = stated_amendments(dataset)?;
    let mut outcomes: Vec<Option<Outcome>> = vec![None; stated.len()];

    // Stage 1: the address. Each addressed amendment is sent on to the work of
    // the Code its section sits in.
    let mut by_work: BTreeMap<WorkId, Vec<usize>> = BTreeMap::new();
    for (at, amendment) in stated.iter().enumerate() {
        match addressed_work(&amendment.address) {
            Ok(work) => by_work.entry(work).or_default().push(at),
            Err(reason) => outcomes[at] = Some(residue(Stage::Address, reason)),
        }
    }

    // One work at a time, so only one title of the Code is in memory at once.
    for (work, members) in &by_work {
        match_in_work(dataset, work, &stated, members, &mut outcomes)?;
    }

    Ok(stated
        .into_iter()
        .zip(outcomes)
        .map(|(amendment, outcome)| AmendmentMatch {
            bill_id: amendment.bill_id,
            amendment_id: amendment.address.amendment_id.clone(),
            address: amendment.address,
            outcome: outcome.expect("every amendment is answered for"),
        })
        .collect())
}

/// One amendment as its public law states it.
struct Stated {
    bill_id: String,
    /// The date of the law's stored expression.
    enacted: String,
    address: AmendmentAddress,
}

/// Every amendment of every public law the dataset holds as a document.
fn stated_amendments<S: Storage + LegislatureReader>(
    dataset: &Dataset<S>,
) -> Result<Vec<Stated>, DatasetError> {
    let mut stated = Vec::new();
    for bill_id in dataset.list_bill_ids()? {
        // Only a public law is matched: a bill that has not been enacted
        // changes nothing (`docs/adr/0013`). A bill with no public law document
        // in the dataset is not one this dataset can match.
        let Some(law) = dataset.bill_document(&bill_id)? else {
            continue;
        };
        for address in addresses_in(&law.root) {
            stated.push(Stated {
                bill_id: bill_id.clone(),
                enacted: law.id.at.clone(),
                address,
            });
        }
    }
    Ok(stated)
}

/// The work of the Code an address acts in, or why there is none.
fn addressed_work(address: &AmendmentAddress) -> Result<WorkId, String> {
    let Some(section) = &address.section else {
        return Err(address
            .unresolved
            .as_ref()
            .map_or("the markup gives no address".to_string(), ToString::to_string));
    };
    work_of(section).ok_or_else(|| format!("{section} is not a section of the US Code"))
}

/// The work of the Code a section identifier sits in: `/us/usc/t7/s2028` is in
/// `uscode/title_7`.
fn work_of(section: &str) -> Option<WorkId> {
    let title = section.strip_prefix("/us/usc/t")?.split('/').next()?;
    Some(WorkId::new(format!("uscode/title_{title}")))
}

/// One window of a work, read once for every amendment that acts in it.
struct WindowView {
    from: ExpressionId,
    to: ExpressionId,
    earlier: DocumentNode,
    later: DocumentNode,
    /// Every path that changed in the window, in document order.
    changed: Vec<String>,
}

/// Where one amendment's address found changes: a window, and the changed
/// paths under the address in it.
struct Found {
    window: usize,
    paths: Vec<String>,
}

/// Match every amendment that acts in one work.
fn match_in_work<S: Storage + LegislatureReader>(
    dataset: &Dataset<S>,
    work: &WorkId,
    stated: &[Stated],
    members: &[usize],
    outcomes: &mut [Option<Outcome>],
) -> Result<(), DatasetError> {
    let earliest_enactment = members
        .iter()
        .map(|&at| stated[at].enacted.as_str())
        .min()
        .unwrap_or_default();
    let windows = windows_after(dataset, work, earliest_enactment)?;

    // Stage 2: the window. Every window the amendment's address changed in,
    // oldest first.
    let mut found: Vec<Vec<Found>> = members.iter().map(|_| Vec::new()).collect();
    for (window, view) in windows.iter().enumerate() {
        let later = SectionIndex::of(&view.later);
        let earlier = SectionIndex::of(&view.earlier);
        for (member, &at) in members.iter().enumerate() {
            let amendment = &stated[at];
            if view.to.at <= amendment.enacted {
                continue;
            }
            let Some(under) = address_path(&later, &earlier, &amendment.address) else {
                continue;
            };
            let paths: Vec<String> = view
                .changed
                .iter()
                .filter(|path| is_at_or_below(path, &under))
                .cloned()
                .collect();
            if !paths.is_empty() {
                found[member].push(Found { window, paths });
            }
        }
    }

    // Stage 3: resolve, in the first window that holds a change.
    for (member, &at) in members.iter().enumerate() {
        let amendment = &stated[at];
        outcomes[at] = Some(match found[member].first() {
            Some(first) => {
                let view = &windows[first.window];
                Outcome::Linked(Linked {
                    from: view.from.clone(),
                    to: view.to.clone(),
                    paths: first.paths.clone(),
                })
            }
            None => residue(
                Stage::Window,
                format!(
                    "nothing under {} changed in any window after {}",
                    amendment.address.section.as_deref().unwrap_or_default(),
                    amendment.enacted
                ),
            ),
        });
    }
    Ok(())
}

fn residue(stage: Stage, reason: String) -> Outcome {
    Outcome::Residue(Residue { stage, reason })
}

/// Every window of a work whose later expression is dated after `enacted`,
/// oldest first, each read and diffed once.
///
/// A window that ends on or before a law's enactment date holds the Code as it
/// read before the law existed, so the law cannot have changed anything in it.
fn windows_after<S: Storage + LegislatureReader>(
    dataset: &Dataset<S>,
    work: &WorkId,
    enacted: &str,
) -> Result<Vec<WindowView>, DatasetError> {
    let held = dataset.expressions(work)?;
    let mut views = Vec::new();
    for pair in held.windows(2) {
        let (from, to) = (pair[0].id.clone(), pair[1].id.clone());
        if to.at.as_str() <= enacted {
            continue;
        }
        let (Some(earlier), Some(later)) =
            (dataset.get_expression(&from)?, dataset.get_expression(&to)?)
        else {
            continue;
        };
        let diff = dataset.compute_diff(&from, &to)?;
        let mut changed = Vec::new();
        collect_changed_paths(&diff, &mut changed);
        views.push(WindowView {
            from,
            to,
            earlier: earlier.root,
            later: later.root,
            changed,
        });
    }
    Ok(views)
}

/// The structural path an address names, in the later document of a window
/// or, when the later one does not hold it, in the earlier one.
fn address_path(
    later: &SectionIndex,
    earlier: &SectionIndex,
    address: &AmendmentAddress,
) -> Option<String> {
    let section = address.section.as_deref()?;
    [later, earlier].into_iter().find_map(|index| {
        let [section] = index.get(section) else {
            return None;
        };
        walk_down(section, &address.container)
            .ok()
            .map(|node| node.data.path.to_string())
    })
}

/// Every path that changed in a diff: its own words changed, or it was added,
/// removed or renumbered.
fn collect_changed_paths(diff: &TreeDiff, paths: &mut Vec<String>) {
    if !diff.changes.is_empty() {
        paths.push(diff.root_path.clone());
    }
    paths.extend(diff.added.iter().map(|node| node.path.to_string()));
    paths.extend(diff.removed.iter().map(|node| node.path.to_string()));
    paths.extend(diff.moved.iter().map(|moved| moved.to.path.to_string()));
    for child in &diff.child_diffs {
        collect_changed_paths(child, paths);
    }
}

fn is_at_or_below(path: &str, under: &str) -> bool {
    path == under
        || path
            .strip_prefix(under)
            .is_some_and(|rest| rest.starts_with('/'))
}
