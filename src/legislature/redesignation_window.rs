//! Which window a renumbering statement is recorded in (#172).
//!
//! A law states a renumbering once, and a dataset with three release points
//! holds two windows of each work. A statement can resolve in both: a shift run
//! such as "redesignating paragraphs (7) through (9) as paragraphs (8) through
//! (10)" leaves every path of the run present on every date. So "it resolves"
//! cannot say where the law acted.
//!
//! # The window rule
//!
//! The rule is the evidence matcher's
//! (`docs/adr/0013-matching-is-evidence-first-and-the-batch-calls-no-model.md`):
//! a statement is recorded in the first window that
//!
//! 1. ends after the law's enactment date, the date of its stored expression
//!    ([`window_can_hold`]), and
//! 2. shows a change to the text under the statement's container.
//!
//! A window with identical text at both ends cannot hold a move, so refusing it
//! is evidence and not a tie-break (decision 17 of #179). A later window that
//! also meets both conditions is named for review as a [`LaterWindow`] and is
//! never written as a link.
//!
//! # A dataset that grows
//!
//! A step over every window at once places each statement in the earliest
//! window that qualifies. A dataset that grows is read one new window at a
//! time, and a step that sees only the new window cannot know on its own that a
//! statement is already placed before it (#273). So the caller names each
//! window a statement already holds a link in, as a [`Placed`], and a statement
//! placed in an earlier window is not placed again: a later window that
//! qualifies is a [`LaterWindow`], as it is when the step sees every window. A
//! build from scratch and a grown dataset then hold the same links.
//!
//! **The text is compared without the links.** The diff a dataset gives pairs a
//! renumbered provision with what it became, from the links this step writes,
//! so asking it whether a window changed would read this step's own answer
//! back. Two documents compared by position say whether anything changed at
//! all, which is the whole question here.

use serde::{Deserialize, Serialize};

use crate::dataset::{DatasetError, ExpressionId, ExpressionPair, WorkId};
use crate::diff::TreeDiff;
use crate::document::DocumentNode;
use crate::legislature::evidence_matching::window_can_hold;
use crate::legislature::redesignation::{
    Reader, Reason, RedesignationReport, SectionIndex, StatedRedesignation, UnplacedStatement,
    resolve, walk_down,
};
use crate::link::{Link, LinkKind, Target, VerificationState};
use crate::review::{Verdict, newest_naming};
use crate::storage::DocumentReader;

/// A later window in which the text under a statement's container changed too.
///
/// Never a link. The statement is recorded in the first window after the law's
/// enactment that shows a change, and a change under the same container later
/// is something else, or the same change seen twice. It is named so a reviewer
/// can look.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaterWindow {
    /// The amendment the statement was read out of, by its content hash.
    pub amendment_id: String,
    /// The clause, as the bill wrote it.
    pub text: String,
    pub from: ExpressionId,
    pub to: ExpressionId,
}

/// A window in which a statement already holds a renumbering link.
///
/// Read out of the links a dataset holds with [`placements_in`]. A statement is
/// named by the amendment it came from and the words it was read out of, as
/// everywhere in the report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Placed {
    pub amendment_id: String,
    pub text: String,
    pub from: ExpressionId,
    pub to: ExpressionId,
}

/// Every window in which a statement of `bill_id` holds a standing renumbering
/// link.
///
/// Reads the links this step writes: the bill and the amendment are in the
/// payload, and the words of the statement are the evidence's reasoning. A link
/// of another kind or another bill is skipped.
///
/// A refuted link is skipped too. It was checked and found wrong, so it places
/// nothing, and a statement whose every link is refuted is work again, as an
/// amendment is (#268). A link is refuted by its own state or by the newest
/// review of it out of `reviews`, the two readings `inspect` refuses on.
pub fn placements_in(bill_id: &str, links: &[Link], reviews: &[Link]) -> Vec<Placed> {
    links
        .iter()
        .filter(|link| link.kind.0 == LinkKind::REDESIGNATED_AS)
        .filter(|link| !is_refuted(link, reviews))
        .filter_map(|link| {
            let payload = &link.payload.as_ref()?.value;
            if payload["bill_id"].as_str()? != bill_id {
                return None;
            }
            let Target::Change {
                work,
                from_date,
                to_date,
                ..
            } = &link.subject
            else {
                return None;
            };
            Some(Placed {
                amendment_id: payload["amendment_id"].as_str()?.to_string(),
                text: link.provenance.evidence.as_ref()?.reasoning.clone()?,
                from: ExpressionId::new(work.clone(), from_date),
                to: ExpressionId::new(work.clone(), to_date),
            })
        })
        .collect()
}

/// Whether a link is refuted, by its own state or by its newest review.
fn is_refuted(link: &Link, reviews: &[Link]) -> bool {
    link.provenance.verification == VerificationState::Refuted
        || newest_naming(&link.id(), reviews)
            .is_some_and(|review| review.verdict == Verdict::Refuted)
}

/// Where each statement of one law lands, window by window.
#[derive(Debug, Default)]
pub struct Placement {
    /// Each window, with the statements recorded in it resolved there.
    ///
    /// Every window the caller named is here, in the caller's order, and one
    /// that no statement lands in carries an empty report. The step ran over it
    /// and found nothing to record.
    pub windows: Vec<(ExpressionPair, RedesignationReport)>,
    /// The statements no window can hold, with the reason for each.
    ///
    /// A statement placed before every window named is here too, in
    /// `placed_earlier` and not as unplaced: no window named holds it, and the
    /// dataset already does.
    pub unplaced: RedesignationReport,
    /// Every later window that could also hold a statement.
    pub later_windows: Vec<LaterWindow>,
}

impl Placement {
    /// One report for the law, folded across works and windows.
    ///
    /// A statement resolves in the one work that holds its section and fails
    /// in every other, so the reports are folded rather than concatenated
    /// ([`RedesignationReport::across_works`]).
    pub fn report(&self) -> RedesignationReport {
        let reports = self
            .windows
            .iter()
            .map(|(_, report)| report.clone())
            .chain(std::iter::once(self.unplaced.clone()));
        let mut folded = RedesignationReport::across_works(reports);
        folded.later_windows = self.later_windows.clone();
        folded
    }
}

/// Place each statement of a law enacted on `enacted` in the one window of
/// `windows` that can hold it.
///
/// `windows` are pairs of expressions of one work each, as
/// [`crate::dataset::adjacent_expressions`] or an operator's span gives them.
/// Nothing is written: the caller records the links, and a report that only
/// reads, such as `validate`, uses the same answer.
pub fn place<R: DocumentReader + ?Sized>(
    reader: &R,
    stated: &[StatedRedesignation],
    enacted: &str,
    windows: &[ExpressionPair],
) -> Result<Placement, DatasetError> {
    place_beside(reader, stated, enacted, windows, &[])
}

/// [`place`], for a dataset that already holds some of the law's links.
///
/// `placed` names the windows the law's statements already hold a link in. A
/// statement placed in a window earlier than one of `windows` is not placed in
/// it again, and that window is a [`LaterWindow`] if it qualifies. This is
/// what keeps a grown dataset equal to a build from scratch (#273).
pub fn place_beside<R: DocumentReader + ?Sized>(
    reader: &R,
    stated: &[StatedRedesignation],
    enacted: &str,
    windows: &[ExpressionPair],
    placed: &[Placed],
) -> Result<Placement, DatasetError> {
    let mut placement = Placement::default();
    // A law named against no window has nothing to be checked against. Every
    // statement it makes is unplaced, and saying nothing would read as a law
    // that renumbered nothing (#153).
    if windows.is_empty() {
        placement.unplaced = RedesignationReport::without_a_window(stated);
        return Ok(placement);
    }

    let mut reports = Vec::new();
    for work in works_of(windows) {
        let mut views = Vec::new();
        for (from, to) in windows.iter().filter(|(from, _)| from.work == work) {
            let (earlier, later) = crate::storage::memory::require_same_work(reader, from, to)?;
            views.push(WindowView {
                from: from.clone(),
                to: to.clone(),
                earlier: earlier.root,
                later: later.root,
            });
        }
        // Oldest first, so the first window that qualifies is the earliest.
        // Two windows that open on one date are ordered by where they end.
        views.sort_by(|left, right| left.order().cmp(&right.order()));
        place_in_work(
            stated,
            enacted,
            &views,
            placed,
            &mut placement,
            &mut reports,
        );
    }
    placement.windows = windows
        .iter()
        .map(|window| {
            let report = reports
                .iter()
                .find(|(held, _)| held == window)
                .map(|(_, report)| report.clone())
                .unwrap_or_default();
            (window.clone(), report)
        })
        .collect();
    Ok(placement)
}

/// One window of one work, read once for every statement.
struct WindowView {
    from: ExpressionId,
    to: ExpressionId,
    earlier: DocumentNode,
    later: DocumentNode,
}

impl WindowView {
    /// Where the window sorts: by the date it opens on, then by the date it
    /// ends on.
    fn order(&self) -> (&str, &str) {
        (&self.from.at, &self.to.at)
    }
}

impl Placed {
    /// Where the window sorts, as [`WindowView::order`] sorts one.
    fn order(&self) -> (&str, &str) {
        (&self.from.at, &self.to.at)
    }
}

/// The earliest window of `work` in which a statement already holds a link.
fn earliest_placement<'a>(
    statement: &StatedRedesignation,
    work: &WorkId,
    placed: &'a [Placed],
) -> Option<&'a Placed> {
    placed
        .iter()
        .filter(|held| {
            held.from.work == *work
                && held.amendment_id == statement.amendment_id
                && held.text == statement.text
        })
        .min_by(|left, right| left.order().cmp(&right.order()))
}

/// The works the windows are of, each once, in the order first named.
fn works_of(windows: &[ExpressionPair]) -> Vec<WorkId> {
    let mut works: Vec<WorkId> = Vec::new();
    for (from, _) in windows {
        if !works.contains(&from.work) {
            works.push(from.work.clone());
        }
    }
    works
}

/// Place every statement in the windows of one work.
fn place_in_work(
    stated: &[StatedRedesignation],
    enacted: &str,
    views: &[WindowView],
    placed: &[Placed],
    placement: &mut Placement,
    reports: &mut Vec<(ExpressionPair, RedesignationReport)>,
) {
    let indexes: Vec<(SectionIndex, SectionIndex)> = views
        .iter()
        .map(|view| {
            (
                SectionIndex::of(&view.earlier),
                SectionIndex::of(&view.later),
            )
        })
        .collect();

    // The statements each window holds, by the window's place in `views`.
    let mut landed: Vec<Vec<StatedRedesignation>> = vec![Vec::new(); views.len()];
    let mut held_by_none = Vec::new();
    for statement in stated {
        let can_hold: Vec<usize> = (0..views.len())
            .filter(|&at| window_can_hold(enacted, &views[at].to.at))
            .filter(|&at| {
                let (earlier, later) = &indexes[at];
                container_changed(statement, earlier, later, &views[at].later)
            })
            .collect();
        // A window after the one the statement is already placed in is a later
        // window, whatever it shows (#273).
        let held = earliest_placement(statement, &views[0].from.work, placed);
        let (open, after): (Vec<usize>, Vec<usize>) = match held {
            Some(held) => can_hold
                .into_iter()
                .partition(|&at| views[at].order() <= held.order()),
            None => (can_hold, Vec::new()),
        };
        let later: Vec<usize> = match (open.split_first(), held) {
            (Some((&first, later)), _) => {
                landed[first].push(statement.clone());
                later.iter().chain(&after).copied().collect()
            }
            // Placed before every window named. The link is there already, so
            // the statement is placed, and no window here is where the law
            // acted.
            (None, Some(held)) => {
                placement.unplaced.placed_earlier.push(held.clone());
                after
            }
            (None, None) => {
                held_by_none.push(statement.clone());
                after
            }
        };
        placement
            .later_windows
            .extend(later.iter().map(|&at| LaterWindow {
                amendment_id: statement.amendment_id.clone(),
                text: statement.text.clone(),
                from: views[at].from.clone(),
                to: views[at].to.clone(),
            }));
    }

    for (view, statements) in views.iter().zip(&landed) {
        reports.push((
            (view.from.clone(), view.to.clone()),
            resolve(statements, &view.earlier, &view.later),
        ));
    }
    let unplaced = why_no_window(&held_by_none, enacted, views);
    placement.unplaced.absorb(unplaced);
}

/// Why no window of a work can hold these statements.
///
/// The resolver's own reason where it has one: a statement whose section this
/// work does not hold, or whose provision is not there, says so. Where the
/// resolver would place the statement, the window is the reason, and placing it
/// anyway would write a move into a window in which the law did nothing.
fn why_no_window(
    statements: &[StatedRedesignation],
    enacted: &str,
    views: &[WindowView],
) -> RedesignationReport {
    let Some(first) = views.first() else {
        return RedesignationReport::default();
    };
    let tried = resolve(statements, &first.earlier, &first.later);
    let resolves = |amendment_id: &str, text: &str| {
        tried
            .resolved
            .iter()
            .any(|row| row.amendment_id == amendment_id && row.text == text)
    };
    let mut report = RedesignationReport::default();
    // A statement that places even one of its renumberings is refused for the
    // window, and its other renumberings' reasons would hide that.
    report.unplaced = tried
        .unplaced
        .iter()
        .filter(|unplaced| !resolves(&unplaced.amendment_id, &unplaced.text))
        .cloned()
        .collect();
    let index = SectionIndex::of(&first.earlier);
    for statement in statements {
        if !resolves(&statement.amendment_id, &statement.text) {
            continue;
        }
        let container = container_in(statement, &index)
            .map(|node| node.data.path.to_string())
            .unwrap_or_default();
        report.unplaced.push(UnplacedStatement {
            amendment_id: statement.amendment_id.clone(),
            text: statement.text.clone(),
            path: statement.path.clone(),
            reason: Reason::NothingChangedUnder {
                container,
                enacted: enacted.to_string(),
            },
            reader: Reader::Rule,
        });
    }
    report
}

/// The provision a statement renumbers inside, as a document holds it.
fn container_in<'a>(
    statement: &StatedRedesignation,
    index: &SectionIndex<'a>,
) -> Option<&'a DocumentNode> {
    let [section] = index.get(statement.section.as_deref()?) else {
        return None;
    };
    walk_down(section, &statement.container).ok()
}

/// Whether the text under a statement's container differs at a window's two
/// ends.
///
/// A container held at one end only has changed. One held at neither end has
/// not: nothing is there.
fn container_changed(
    statement: &StatedRedesignation,
    earlier: &SectionIndex,
    later: &SectionIndex,
    later_root: &DocumentNode,
) -> bool {
    match container_in(statement, earlier) {
        Some(was) => match later_root.find_all(&was.data.path).as_slice() {
            [became] => !TreeDiff::from_nodes(was, became).is_empty(),
            _ => true,
        },
        None => container_in(statement, later).is_some(),
    }
}
