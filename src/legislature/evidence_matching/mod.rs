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

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::dataset::{Dataset, DatasetError, ExpressionId, WorkId};
use crate::diff::{FieldChangeEvent, TreeDiff};
use crate::document::{DocumentNode, NodeData};
use crate::legislature::AmendingAction;
use crate::legislature::redesignation::{SectionIndex, walk_down};
use crate::link::{
    Evidence, KindPayload, Link, LinkKind, Provenance, Target, VerificationState,
    amendment_reference,
};
use crate::method::Method;
use crate::storage::{LegislatureReader, LinkReader, Storage};
use crate::uslm::UslmFacts;
use crate::uslm::amendment_address::{AmendmentAddress, addresses_in};

use quoted_words::QuotedWords;
use resolve::{Change, Contender, Resolution, resolve};

mod olrc;
mod quoted_words;

pub(crate) use olrc::is_a_note;
pub use olrc::{OlrcClassification, olrc_classification};
mod resolve;

/// What the matcher found for one amendment.
#[derive(Debug, Clone, Serialize)]
pub struct AmendmentMatch {
    /// The bill, as the dataset names it: `119-hr-1`.
    pub bill_id: String,
    /// The public law the bill became, by its number: `119-21`.
    pub public_law: String,
    /// The amendment, by the content hash its bill's node carries.
    pub amendment_id: String,
    /// The amendment's words, as the bill states them.
    pub amending_text: String,
    /// The action the bill's markup states for it.
    pub operation: AmendingAction,
    /// The law's enactment date: the date of its stored expression.
    pub enacted: String,
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
    /// Each change the amendment caused, in document order.
    pub changes: Vec<CausedChange>,
    /// Every later window in which something under the address changed too.
    ///
    /// Never a second link. The law landed in the first window after its
    /// enactment that shows it, and a change under the same address later is
    /// something else, or the same change seen twice. It is named so a
    /// reviewer can look (`docs/adr/0013`).
    pub later_windows: Vec<LaterWindow>,
    /// What the OLRC's classification says about the section. Evidence for a
    /// reviewer, and never a reason for the link.
    pub olrc: OlrcClassification,
}

impl Linked {
    /// The changed paths, in document order.
    pub fn paths(&self) -> Vec<String> {
        self.changes
            .iter()
            .map(|change| change.path.clone())
            .collect()
    }
}

/// One change an amendment caused, and why it was given to that amendment.
#[derive(Debug, Clone, Serialize)]
pub struct CausedChange {
    /// The changed path.
    pub path: String,
    /// How the change was told apart from the others under the address, in
    /// words a reviewer can check.
    pub why: String,
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
    /// The address is in the Code the dataset holds, and nothing under it
    /// changed in any window held after the law's enactment.
    ///
    /// Not work. A corpus that does not reach the date an amendment takes
    /// effect is the ordinary state of a growing dataset (#211), and nothing is
    /// there to resolve until a later release point is added.
    pub quiet: bool,
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

/// The reasoning this module applies, at the version it is at now.
///
/// A different reasoning from the model method `match-amendments` ran before it
/// was removed (#252), so a new name and not a new version of that one (#179,
/// decision 10). Raise the
/// version when this method's answers change — a new rule for telling changes
/// apart, a different window rule. Tidying the code that gives the same answers
/// is not such a change (`crate::method::Method`).
pub fn evidence_method() -> Method {
    Method::new("address, window and quoted words", 1)
}

/// Who a link this method writes says made it.
const SOURCE: &str = "rule:evidence_matching";

impl AmendmentMatch {
    /// One `legislature.amended_by` link for each change this amendment
    /// caused, or none when it is residue.
    ///
    /// The shape `match-amendments` wrote before its removal (#252), so every reader of those links
    /// reads these unchanged: the subject is the change, the object is the
    /// amendment, and the payload is the legislature's. The address, the
    /// window and the words that placed the change are the link's evidence.
    ///
    /// `MachineSuggested`: the bill asserted the amendment, and the reading of
    /// which change it made is ours (`docs/adr/0010`). No clock reading,
    /// because the same dataset gives the same answer on any day.
    pub fn links(&self) -> Vec<Link> {
        let Outcome::Linked(linked) = &self.outcome else {
            return Vec::new();
        };
        linked
            .changes
            .iter()
            .map(|change| Link {
                subject: Target::Change {
                    work: linked.from.work.clone(),
                    path: change.path.clone(),
                    from_date: linked.from.at.clone(),
                    to_date: linked.to.at.clone(),
                },
                kind: LinkKind::new(LinkKind::AMENDED_BY),
                object: Target::External {
                    reference: amendment_reference(&self.bill_id, &self.amendment_id),
                    display: self.amending_text.clone(),
                },
                provenance: Provenance {
                    source: SOURCE.to_string(),
                    method: Some(evidence_method()),
                    verification: VerificationState::MachineSuggested,
                    evidence: Evidence::from_reasoning(Some(self.reasoning(linked, change))),
                    raw_score: None,
                    timestamp: None,
                    corroboration: None,
                },
                payload: Some(KindPayload {
                    namespace: LinkKind::LEGISLATURE.to_string(),
                    value: serde_json::json!({
                        "operation": self.operation,
                        "bill_id": self.bill_id,
                        "amendment_id": self.amendment_id,
                        "notes": null,
                    }),
                }),
            })
            .collect()
    }

    /// The link's evidence: where, when, and by which words.
    fn reasoning(&self, linked: &Linked, change: &CausedChange) -> String {
        let mut reasoning = format!(
            "Address: {}, read from the bill's markup. \
             Window: {} to {}, the first window after the law's enactment on {} \
             in which something under the address changed. ",
            address_text(&self.address),
            linked.from,
            linked.to.at,
            self.enacted
        );
        for later in &linked.later_windows {
            reasoning.push_str(&format!(
                "The address changed again from {} to {}, and that window is not linked. ",
                later.from, later.to.at
            ));
        }
        reasoning.push_str("Change: ");
        reasoning.push_str(&change.why);
        reasoning.push(' ');
        reasoning.push_str(&linked.olrc.sentence(&self.public_law));
        reasoning
    }
}

/// What one run of the matcher found.
#[derive(Debug, Clone, Serialize)]
pub struct EvidenceMatching {
    /// One answer for every amendment of every public law the dataset holds,
    /// in the order each bill states them.
    pub matches: Vec<AmendmentMatch>,
    /// Every window the method read, in the order it read them.
    ///
    /// A window it read and linked nothing in has still been worked on, so a
    /// run is recorded over each of these and not only over the windows that
    /// carry a link (#179, decision 11).
    pub windows: Vec<(ExpressionId, ExpressionId)>,
}

/// What the matcher finds for every amendment of every public law the dataset
/// holds.
pub fn match_by_evidence<S: Storage + LegislatureReader>(
    dataset: &Dataset<S>,
) -> Result<EvidenceMatching, DatasetError> {
    let stated = stated_amendments(dataset)?;
    let classifications = dataset.links_by_kind(LinkKind::CLASSIFIED_FROM)?;
    let mut outcomes: Vec<Option<Outcome>> = vec![None; stated.len()];
    let mut windows = Vec::new();

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
        windows.extend(match_in_work(
            dataset,
            work,
            &stated,
            members,
            &classifications,
            &mut outcomes,
        )?);
    }

    let matches = stated
        .into_iter()
        .zip(outcomes)
        .map(|(amendment, outcome)| AmendmentMatch {
            bill_id: amendment.bill_id,
            public_law: amendment.public_law,
            amendment_id: amendment.address.amendment_id.clone(),
            amending_text: amendment.amending_text,
            operation: amendment.operation,
            enacted: amendment.enacted,
            address: amendment.address,
            outcome: outcome.expect("every amendment is answered for"),
        })
        .collect();
    Ok(EvidenceMatching { matches, windows })
}

/// One amendment as its public law states it.
struct Stated {
    bill_id: String,
    /// The law's number: `119-21`.
    public_law: String,
    /// The date of the law's stored expression.
    enacted: String,
    amending_text: String,
    operation: AmendingAction,
    address: AmendmentAddress,
    /// The words the bill quotes for it.
    evidence: QuotedWords,
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
        let Some(bill) = dataset.get_bill(&bill_id)? else {
            continue;
        };
        // The law's number is the name of its work: `publiclawdocument_119-21`.
        let public_law = law
            .id
            .work
            .as_str()
            .split_once('_')
            .map_or(law.id.work.as_str(), |(_, number)| number)
            .to_string();
        for address in addresses_in(&law.root) {
            let evidence = law
                .root
                .find(&address.path)
                .and_then(|node| UslmFacts::of(&node.data))
                .and_then(|facts| facts.amendment)
                .map(|facts| QuotedWords::of(&facts))
                .unwrap_or_default();
            let amendment = bill.amendments.get(&address.amendment_id);
            stated.push(Stated {
                bill_id: bill_id.clone(),
                public_law: public_law.clone(),
                enacted: law.id.at.clone(),
                amending_text: amendment
                    .map(|amendment| amendment.amending_text.clone())
                    .unwrap_or_else(|| address.text.clone()),
                operation: amendment.map_or(AmendingAction::Amend, |amendment| {
                    stated_operation(&amendment.action_types)
                }),
                address,
                evidence,
            });
        }
    }
    Ok(stated)
}

/// The action a link records: the one the bill's markup states.
///
/// `amend` is the markup's umbrella word, and most instructions carry it beside
/// the action that says what they do, so it is set aside. When one action is
/// left, that is the action. When none or several are left, `amend` is the
/// honest word, because choosing one of several would claim a reading nobody
/// made.
fn stated_operation(actions: &[AmendingAction]) -> AmendingAction {
    let mut specific: Vec<AmendingAction> = Vec::new();
    for action in actions {
        if *action != AmendingAction::Amend && !specific.contains(action) {
            specific.push(*action);
        }
    }
    match specific.as_slice() {
        [one] => *one,
        _ => AmendingAction::Amend,
    }
}

/// The work of the Code an address acts in, or why there is none.
fn addressed_work(address: &AmendmentAddress) -> Result<WorkId, String> {
    let Some(section) = &address.section else {
        return Err(address.unresolved.as_ref().map_or(
            "the markup gives no address".to_string(),
            ToString::to_string,
        ));
    };
    work_of(section).ok_or_else(|| format!("{section} is not a section of the US Code"))
}

/// An address as one string: the section, then each step below it,
/// `/us/usc/t26/s11026(a)`.
fn address_text(address: &AmendmentAddress) -> String {
    let below: String = address
        .container
        .iter()
        .map(|step| format!("({})", step.number))
        .collect();
    format!("{}{below}", address.section.as_deref().unwrap_or_default())
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

/// Match every amendment that acts in one work, and say which windows were
/// read.
fn match_in_work<S: Storage + LegislatureReader>(
    dataset: &Dataset<S>,
    work: &WorkId,
    stated: &[Stated],
    members: &[usize],
    classifications: &[Link],
    outcomes: &mut [Option<Outcome>],
) -> Result<Vec<(ExpressionId, ExpressionId)>, DatasetError> {
    let earliest_enactment = members
        .iter()
        .map(|&at| stated[at].enacted.as_str())
        .min()
        .unwrap_or_default();
    // A title the dataset does not hold is a limit of the dataset. Saying that
    // nothing changed there would state a fact about the law nobody checked.
    if dataset.expressions(work)?.is_empty() {
        for &at in members {
            outcomes[at] = Some(residue(
                Stage::Window,
                format!("the dataset holds no expression of {work}"),
            ));
        }
        return Ok(Vec::new());
    }
    let windows = windows_after(dataset, work, earliest_enactment)?;

    // Stage 2: the window. Every window the amendment's address changed in,
    // oldest first.
    let mut placed: BTreeMap<usize, Vec<Placed>> = BTreeMap::new();
    // The amendments a window after enactment was read for, and those whose
    // address that window's Code holds. One looked for and never found is not
    // quiet: nothing changed there because nothing is there.
    let mut looked_for: BTreeSet<usize> = BTreeSet::new();
    let mut found: BTreeSet<usize> = BTreeSet::new();
    for (window, view) in windows.iter().enumerate() {
        let later = SectionIndex::of(&view.later);
        let earlier = SectionIndex::of(&view.earlier);
        for &at in members {
            let amendment = &stated[at];
            if !window_can_hold(&amendment.enacted, &view.to.at) {
                continue;
            }
            looked_for.insert(at);
            let Some((section, under)) = address_path(&later, &earlier, &amendment.address) else {
                continue;
            };
            found.insert(at);
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
            None if looked_for.contains(&at) && !found.contains(&at) => {
                let amendment = &stated[at];
                outcomes[at] = Some(residue(
                    Stage::Window,
                    format!(
                        "the Code the dataset holds has no {} in any window after {}",
                        address_text(&amendment.address),
                        amendment.enacted
                    ),
                ));
            }
            None => {
                let amendment = &stated[at];
                outcomes[at] = Some(Outcome::Residue(Residue {
                    stage: Stage::Window,
                    reason: format!(
                        "nothing under {} changed in any window after {}",
                        amendment.address.section.as_deref().unwrap_or_default(),
                        amendment.enacted
                    ),
                    quiet: true,
                    from: None,
                    to: None,
                    changes: Vec::new(),
                }));
            }
        }
    }
    for ((window, _), group) in groups {
        let view = &windows[window];
        let contenders: Vec<Contender> = group
            .iter()
            .map(|at| Contender {
                amendment_id: &stated[*at].address.amendment_id,
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
                    changes: one_for_each_path(
                        caused
                            .iter()
                            .map(|(change, found)| CausedChange {
                                path: view.changes[*change].path.clone(),
                                why: found.to_string(),
                            })
                            .collect(),
                    ),
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
                    olrc: olrc_classification(
                        classifications,
                        &placed[at][0].section,
                        &stated[*at].public_law,
                    ),
                }),
                Resolution::Stopped(reason) => Outcome::Residue(Residue {
                    stage: Stage::Resolve,
                    reason,
                    quiet: false,
                    from: Some(view.from.clone()),
                    to: Some(view.to.clone()),
                    changes: changes_under_address(),
                }),
            });
        }
    }
    Ok(windows
        .into_iter()
        .map(|view| (view.from, view.to))
        .collect())
}

/// The caused changes with each path once, in the order first met.
///
/// A diff reports a renumbered provision whose words also changed twice at one
/// path: as the move, and as the change to its words. One path is one link, so
/// the two become one entry, and its reason says both.
fn one_for_each_path(caused: Vec<CausedChange>) -> Vec<CausedChange> {
    let mut merged: Vec<CausedChange> = Vec::new();
    for change in caused {
        match merged.iter_mut().find(|kept| kept.path == change.path) {
            Some(kept) if kept.why != change.why => {
                kept.why = format!("{} Also: {}", kept.why, change.why);
            }
            Some(_) => {}
            None => merged.push(change),
        }
    }
    merged
}

/// Residue that stopped before any window was found.
fn residue(stage: Stage, reason: String) -> Outcome {
    Outcome::Residue(Residue {
        stage,
        reason,
        quiet: false,
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
        let renumberings = renumberings(&dataset.storage().links_for_pair(&from, &to)?);
        let mut changes = Vec::new();
        collect_changes(&diff, &renumberings, &mut changes);
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

/// Who made each renumbering in a window, by the two paths it moved a provision
/// between, as the window's `legislature.redesignated_as` links say.
type Renumberings = BTreeMap<(String, String), Vec<String>>;

fn renumberings(links: &[Link]) -> Renumberings {
    let path_of = |target: &Target| match target {
        Target::Change { path, .. } | Target::Node(path) => Some(path.clone()),
        _ => None,
    };
    let mut made_by = Renumberings::new();
    for link in links {
        if link.kind != LinkKind::new(LinkKind::REDESIGNATED_AS) {
            continue;
        }
        let amendment = link
            .payload
            .as_ref()
            .and_then(|payload| payload.value.get("amendment_id"))
            .and_then(|id| id.as_str());
        if let (Some(from), Some(to), Some(amendment)) =
            (path_of(&link.subject), path_of(&link.object), amendment)
        {
            made_by
                .entry((from, to))
                .or_default()
                .push(amendment.to_string());
        }
    }
    made_by
}

/// Every change in a diff, with its words before and after: a provision whose
/// own words changed, or one that was added, removed or renumbered.
fn collect_changes(diff: &TreeDiff, renumberings: &Renumberings, changes: &mut Vec<Change>) {
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
            renumbered_by: Vec::new(),
        });
    }
    changes.extend(diff.added.iter().map(|node| Change {
        path: node.path.to_string(),
        before: String::new(),
        after: own_text(node),
        renumbered_by: Vec::new(),
    }));
    changes.extend(diff.removed.iter().map(|node| Change {
        path: node.path.to_string(),
        before: own_text(node),
        after: String::new(),
        renumbered_by: Vec::new(),
    }));
    changes.extend(diff.moved.iter().map(|moved| {
        let paths = (moved.from.path.to_string(), moved.to.path.to_string());
        Change {
            path: moved.to.path.to_string(),
            before: own_text(&moved.from),
            after: own_text(&moved.to),
            renumbered_by: renumberings.get(&paths).cloned().unwrap_or_default(),
        }
    }));
    for child in &diff.child_diffs {
        collect_changes(child, renumberings, changes);
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
