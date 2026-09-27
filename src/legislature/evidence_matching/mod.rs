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
//! 3. **Resolve.** The changes under one section in that window, assigned to
//!    the amendments addressed there all together, by the words the bill
//!    quotes ([`resolve`]).
//!
//! # The window rule
//!
//! A window can hold a law's change only when it ends after the law's
//! enactment date. Public Law 119-21 was enacted on 2025-07-04, and the first
//! committed release point after it is dated 2025-07-18:
//!
//! ```
//! use words_to_data::legislature::evidence_matching::window_can_hold;
//!
//! assert!(window_can_hold("2025-07-04", "2025-07-18"));
//! // A window that ends on the day of enactment, or before it, holds the Code
//! // as it read before the law, so nothing is linked in it.
//! assert!(!window_can_hold("2025-07-04", "2025-07-04"));
//! ```
//!
//! **Nothing here is stored.** The answer for each amendment, linked or
//! stopped with the stage and the reason, is derived from the dataset every
//! time it is asked for (`docs/adr/0007-a-record-is-what-was-said-everything-else-is-derived.md`).
//! A command writes the links; the reason an amendment stopped stays
//! computable for the residue report (#251).

use std::collections::BTreeMap;

use serde::Serialize;

use crate::dataset::{Dataset, DatasetError, ExpressionId, WorkId};
use crate::diff::{FieldChangeEvent, TreeDiff};
use crate::document::{DocumentNode, NodeData};
use crate::legislature::redesignation::{SectionIndex, walk_down};
use crate::storage::{LegislatureReader, Storage};
use crate::uslm::UslmFacts;
use crate::uslm::amendment_address::{AmendmentAddress, addresses_in};

use evidence::Evidence;
use resolve::{Change, Contender, Resolution, resolve};

mod evidence;
mod resolve;

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
    /// Every later window in which something under the address changed too.
    ///
    /// Never a second link. The law landed in the first window after its
    /// enactment that shows it, and a change under the same address later is
    /// something else, or the same change seen twice. It is named so a
    /// reviewer can look (`docs/adr/0013`).
    pub later_windows: Vec<LaterWindow>,
}

/// A later window in which an address changed again.
#[derive(Debug, Clone, Serialize)]
pub struct LaterWindow {
    pub from: ExpressionId,
    pub to: ExpressionId,
    /// The changes under the address in that window, in document order.
    pub changes: Vec<String>,
}

/// Why an amendment was not linked.
#[derive(Debug, Clone, Serialize)]
pub struct Residue {
    /// The stage it stopped at.
    pub stage: Stage,
    /// Why, in words a reviewer can act on.
    pub reason: String,
    /// The older expression of the window the address changed in, when one
    /// did.
    pub from: Option<ExpressionId>,
    /// The newer expression of that window.
    pub to: Option<ExpressionId>,
    /// The changes under the address in that window, in document order: where
    /// a reviewer starts to look.
    pub changes: Vec<String>,
}

/// The stage of the method at which an amendment stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    /// The bill's markup gives no address.
    Address,
    /// No window holds a change under the address.
    Window,
    /// Changes were found under the address, and none could be given to this
    /// amendment.
    Resolve,
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
    /// The words the bill quotes for it.
    evidence: Evidence,
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
            let evidence = law
                .root
                .find(&address.path)
                .and_then(|node| UslmFacts::of(&node.data))
                .and_then(|facts| facts.amendment)
                .map(|facts| Evidence::of(&facts))
                .unwrap_or_default();
            stated.push(Stated {
                bill_id: bill_id.clone(),
                enacted: law.id.at.clone(),
                address,
                evidence,
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
    /// Every change in the window, in document order.
    changes: Vec<Change>,
}

/// Where one amendment's address changed in one window.
struct Placed {
    window: usize,
    /// The section's own path, which groups the amendments resolved together.
    section: String,
    /// The changes under the address, by their place in the window's list.
    candidates: Vec<usize>,
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
    let mut placed: BTreeMap<usize, Vec<Placed>> = BTreeMap::new();
    for (window, view) in windows.iter().enumerate() {
        let later = SectionIndex::of(&view.later);
        let earlier = SectionIndex::of(&view.earlier);
        for &at in members {
            let amendment = &stated[at];
            if !window_can_hold(&amendment.enacted, &view.to.at) {
                continue;
            }
            let Some((section, under)) = address_path(&later, &earlier, &amendment.address)
            else {
                continue;
            };
            let candidates: Vec<usize> = (0..view.changes.len())
                .filter(|&change| is_at_or_below(&view.changes[change].path, &under))
                .collect();
            if !candidates.is_empty() {
                placed.entry(at).or_default().push(Placed {
                    window,
                    section,
                    candidates,
                });
            }
        }
    }

    // Stage 3: resolve, in the first window that holds a change, with every
    // amendment addressed to the same section in the same window.
    let mut groups: BTreeMap<(usize, &str), Vec<usize>> = BTreeMap::new();
    for &at in members {
        match placed.get(&at).and_then(|found| found.first()) {
            Some(first) => groups
                .entry((first.window, first.section.as_str()))
                .or_default()
                .push(at),
            None => {
                let amendment = &stated[at];
                outcomes[at] = Some(residue(
                    Stage::Window,
                    format!(
                        "nothing under {} changed in any window after {}",
                        amendment.address.section.as_deref().unwrap_or_default(),
                        amendment.enacted
                    ),
                ));
            }
        }
    }
    for ((window, _), group) in groups {
        let view = &windows[window];
        let contenders: Vec<Contender> = group
            .iter()
            .map(|at| Contender {
                evidence: &stated[*at].evidence,
                candidates: placed[at][0].candidates.clone(),
            })
            .collect();
        for (at, resolution) in group.iter().zip(resolve(&view.changes, &contenders)) {
            let changes_under_address = || {
                placed[at][0]
                    .candidates
                    .iter()
                    .map(|change| view.changes[*change].path.clone())
                    .collect()
            };
            outcomes[*at] = Some(match resolution {
                Resolution::Caused(caused) => Outcome::Linked(Linked {
                    from: view.from.clone(),
                    to: view.to.clone(),
                    paths: caused
                        .iter()
                        .map(|(change, _)| view.changes[*change].path.clone())
                        .collect(),
                    later_windows: placed[at][1..]
                        .iter()
                        .map(|later| {
                            let view = &windows[later.window];
                            LaterWindow {
                                from: view.from.clone(),
                                to: view.to.clone(),
                                changes: later
                                    .candidates
                                    .iter()
                                    .map(|change| view.changes[*change].path.clone())
                                    .collect(),
                            }
                        })
                        .collect(),
                }),
                Resolution::Stopped(reason) => Outcome::Residue(Residue {
                    stage: Stage::Resolve,
                    reason,
                    from: Some(view.from.clone()),
                    to: Some(view.to.clone()),
                    changes: changes_under_address(),
                }),
            });
        }
    }
    Ok(())
}

/// Residue that stopped before any window was found.
fn residue(stage: Stage, reason: String) -> Outcome {
    Outcome::Residue(Residue {
        stage,
        reason,
        from: None,
        to: None,
        changes: Vec::new(),
    })
}

/// Whether a window that ends on `window_end` can hold a change made by a law
/// enacted on `enacted`. Both are ISO dates.
///
/// A window that ends on or before the enactment date holds the Code as it
/// read before the law existed, so the law cannot have changed anything in it.
pub fn window_can_hold(enacted: &str, window_end: &str) -> bool {
    window_end > enacted
}

/// Every window of a work that can hold a change made on `enacted`, oldest
/// first, each read and diffed once.
fn windows_after<S: Storage + LegislatureReader>(
    dataset: &Dataset<S>,
    work: &WorkId,
    enacted: &str,
) -> Result<Vec<WindowView>, DatasetError> {
    let held = dataset.expressions(work)?;
    let mut views = Vec::new();
    for pair in held.windows(2) {
        let (from, to) = (pair[0].id.clone(), pair[1].id.clone());
        if !window_can_hold(enacted, &to.at) {
            continue;
        }
        let (Some(earlier), Some(later)) =
            (dataset.get_expression(&from)?, dataset.get_expression(&to)?)
        else {
            continue;
        };
        let diff = dataset.compute_diff(&from, &to)?;
        let mut changes = Vec::new();
        collect_changes(&diff, &mut changes);
        views.push(WindowView {
            from,
            to,
            earlier: earlier.root,
            later: later.root,
            changes,
        });
    }
    Ok(views)
}

/// The paths an address names — its section's, and the provision's below
/// it — in the later document of a window or, when the later one does not
/// hold them, in the earlier one.
fn address_path(
    later: &SectionIndex,
    earlier: &SectionIndex,
    address: &AmendmentAddress,
) -> Option<(String, String)> {
    let section = address.section.as_deref()?;
    [later, earlier].into_iter().find_map(|index| {
        let [section] = index.get(section) else {
            return None;
        };
        let under = walk_down(section, &address.container).ok()?;
        Some((section.data.path.to_string(), under.data.path.to_string()))
    })
}

/// Every change in a diff, with its words before and after: a provision whose
/// own words changed, or one that was added, removed or renumbered.
fn collect_changes(diff: &TreeDiff, changes: &mut Vec<Change>) {
    if !diff.changes.is_empty() {
        let joined = |value: fn(&FieldChangeEvent) -> &str| {
            diff.changes
                .iter()
                .map(value)
                .collect::<Vec<&str>>()
                .join(" ")
        };
        changes.push(Change {
            path: diff.root_path.clone(),
            before: joined(|field| &field.old_value),
            after: joined(|field| &field.new_value),
        });
    }
    changes.extend(diff.added.iter().map(|node| Change {
        path: node.path.to_string(),
        before: String::new(),
        after: own_text(node),
    }));
    changes.extend(diff.removed.iter().map(|node| Change {
        path: node.path.to_string(),
        before: own_text(node),
        after: String::new(),
    }));
    changes.extend(diff.moved.iter().map(|moved| Change {
        path: moved.to.path.to_string(),
        before: own_text(&moved.from),
        after: own_text(&moved.to),
    }));
    for child in &diff.child_diffs {
        collect_changes(child, changes);
    }
}

/// One provision's own words, its five text fields joined by a space.
fn own_text(node: &NodeData) -> String {
    [
        node.heading.as_deref(),
        node.chapeau.as_deref(),
        node.content.as_deref(),
        node.proviso.as_deref(),
        node.continuation.as_deref(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<&str>>()
    .join(" ")
}

fn is_at_or_below(path: &str, under: &str) -> bool {
    path == under
        || path
            .strip_prefix(under)
            .is_some_and(|rest| rest.starts_with('/'))
}
