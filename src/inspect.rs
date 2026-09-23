//! Read-only dataset inspection.
//!
//! Backend-agnostic report builders that power the `words_to_data` inspect
//! subcommands (`info`, `versions`, ...). Every function takes any
//! [`Storage`](crate::storage::Storage) — which both `Dataset<InMemoryStorage>`
//! and `Dataset<SqliteStorage>` implement — so the CLI and its tests exercise
//! one code path across both backends.
//!
//! Reports are plain serde structs: the CLI prints them (human-readable or as
//! `--json`), and tests assert on the data rather than on formatting.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::annotation::ChangeAnnotation;
use crate::congress::{Party, PartyOnDate, VotePosition};
use crate::dataset::{DatasetError, ExpressionId, Scope, SearchResult, WorkId};
use crate::diff::{Redesignations, TreeDiff};
use crate::document::DocumentNode;
use crate::link::{ProvisionHistory, RedesignationStep, VerificationState};
use crate::storage::{LegislatureCounts, LegislatureReader, Storage};

/// Top-level summary of a dataset: its metadata plus headline counts.
#[derive(Debug, Clone, Serialize)]
pub struct DatasetInfo {
    pub name: String,
    pub description: String,
    pub author: String,
    pub license: String,
    pub version: String,
    pub source_urls: Vec<String>,
    /// Number of works (distinct documents) held.
    pub work_count: usize,
    /// Number of expressions (work-and-date pairs) held.
    pub expression_count: usize,
    /// How much legislative material this dataset holds, and `None` when it
    /// holds no legislature at all.
    ///
    /// Three readings, and a reader needs all three (#133). Counts above zero:
    /// this dataset speaks legislature and holds that much. Counts of zero: it
    /// speaks legislature and holds none. Absent: legislature is not a concept
    /// here, which is what a dataset of court opinions answers. Five plain
    /// numbers could state the first two readings and never the third.
    ///
    /// The answer comes from [`Storage::legislature`], which is the one
    /// capability query. Deciding it again here would be a second answer to a
    /// settled question.
    ///
    /// [`Storage::legislature`]: crate::storage::Storage::legislature
    #[serde(skip_serializing_if = "Option::is_none")]
    pub legislature: Option<LegislatureCounts>,
    /// Number of links held, over every kind.
    ///
    /// Links are what this project produces; the document text is the input. A
    /// report that counts only the input cannot tell an annotated dataset from
    /// a bare corpus.
    #[serde(skip_serializing_if = "is_zero")]
    pub link_count: usize,
    /// Number of links held of each kind, keyed by the kind named in full,
    /// namespace included.
    ///
    /// A reader that meets a kind it does not own, such as `westlaw.headnote`,
    /// must see it named rather than folded into a total: naming an unknown kind
    /// is what a reader can still do with it
    /// (`docs/adr/0002-links-live-in-the-core.md`).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub link_counts_by_kind: BTreeMap<String, usize>,
    /// Number of verbatim model replies held as evidence (#58).
    #[serde(skip_serializing_if = "is_zero")]
    pub reply_count: usize,
    /// What this dataset covers, so a caller can tell "absent from the law"
    /// from "absent from this dataset".
    pub scope: Scope,
}

/// Whether a count is zero, and so left out of the JSON.
///
/// A dataset that holds no links and no evidence holds none of these things.
/// Emitting a zero for each would grow a wall of them, and a wall of zeroes
/// reads as "this tool measured nothing" rather than "this dataset holds
/// nothing". `work_count` and `expression_count` are always emitted: they were
/// there before this rule, and an agent already reads them. The legislature
/// counts are not decided here at all, because zero and absent are two
/// different answers there.
fn is_zero(count: &usize) -> bool {
    *count == 0
}

/// One expression's headline facts (no element tree).
#[derive(Debug, Clone, Serialize)]
pub struct ExpressionSummary {
    /// The identifier, printed as `uscode/title_9@2025-07-18`.
    pub id: String,
    /// The work alone, for grouping and filtering.
    pub work: String,
    /// Publication date, `YYYY-MM-DD`.
    pub date: String,
    /// Optional human-readable label.
    pub label: Option<String>,
    /// Total elements in this expression's document tree (root included).
    pub element_count: usize,
}

/// One bill's headline facts, for listing many at once.
///
/// Distinct from [`BillSummary`], which carries every amendment and is what
/// `show-bill` returns for one bill.
#[derive(Debug, Clone, Serialize)]
pub struct BillListing {
    /// The identifier `show-bill` takes, such as `119-hr-1`.
    pub bill_id: String,
    /// How many amendments the bill carries.
    pub amendment_count: usize,
    /// How many of those carry extracted word-level changes.
    ///
    /// Zero across the board means `extract-changes` has not run, which is the
    /// difference between a dataset that can be scored and one that cannot.
    pub amendments_with_changes: usize,
}

/// List every bill, ordered by id.
///
/// Without this a bill id could only be learned from outside the tool:
/// `show-bill` demands one, `info` reports a count, and annotations carry ids
/// but a dataset has none until `match-amendments` has run (#83).
pub fn bills<S: Storage + LegislatureReader>(
    dataset: &S,
) -> Result<Vec<BillListing>, DatasetError> {
    let mut ids = dataset.list_bill_ids()?;
    ids.sort();

    let mut summaries = Vec::new();
    for id in ids {
        let Some(bill) = dataset.get_bill(&id)? else {
            continue;
        };
        summaries.push(BillListing {
            bill_id: bill.bill_id,
            amendment_count: bill.amendments.len(),
            amendments_with_changes: bill
                .amendments
                .values()
                .filter(|amendment| !amendment.changes.is_empty())
                .count(),
        });
    }
    Ok(summaries)
}

/// Count every element in a tree, including the root.
fn count_elements(element: &DocumentNode) -> usize {
    1 + element.children.iter().map(count_elements).sum::<usize>()
}

/// List every expression with its label and element count.
///
/// Ordered by work, then by date. Pass `work` to list one document's history.
pub fn expressions<S: Storage>(
    dataset: &S,
    work: Option<&WorkId>,
) -> Result<Vec<ExpressionSummary>, DatasetError> {
    let works = match work {
        Some(one) => vec![one.clone()],
        None => dataset.works()?,
    };

    let mut summaries = Vec::new();
    for work in works {
        for info in dataset.expressions(&work)? {
            let element_count = dataset
                .get_expression(&info.id)?
                .map(|e| count_elements(&e.root))
                .unwrap_or(0);
            summaries.push(ExpressionSummary {
                id: info.id.to_string(),
                work: info.id.work.to_string(),
                date: info.id.at,
                label: info.label,
                element_count,
            });
        }
    }
    Ok(summaries)
}

/// A single field's change at a path between two versions.
#[derive(Debug, Clone, Serialize)]
pub struct PathFieldChange {
    /// Which text field changed, serde string form (e.g. `"heading"`).
    pub field: String,
    pub old_value: String,
    pub new_value: String,
}

/// One expression that holds a path, and how many provisions sit there.
///
/// A path can name more than one provision (`docs/adr/0001`), so an expression
/// is named once with a count. Repeating the same `work@date` once per
/// provision reads as a duplication bug rather than as a fact about the law.
#[derive(Debug, Clone, Serialize)]
pub struct PathPresence {
    /// The expression, as `work@date`.
    pub expression: String,
    /// How many provisions this expression holds at the path.
    pub provisions: usize,
}

/// Whether a provision at a path survived an expression pair.
///
/// A move carries the other end's path inside the variant. An optional field
/// beside a plain `InBoth` was rejected: a caller that read the state and
/// ignored the field would get exactly the false answer #165 removes, so the
/// wrong reading is made unrepresentable instead.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Presence {
    /// In both expressions, at this path, so its field changes are comparable.
    InBoth,
    /// Only in the newer expression: new law at this path.
    Added,
    /// Only in the older expression.
    Removed,
    /// The provision that was at this path is at another path in the newer
    /// expression. A bill renumbered it.
    MovedOut { to_path: String },
    /// The provision at this path in the newer expression was at another path
    /// in the older one. A bill renumbered it.
    MovedIn { from_path: String },
}

/// One redesignation link, as a report names it.
///
/// It says what the link says and who said it, so a reader can weigh the claim
/// instead of taking it. The verification state is here and no corroboration
/// figure is: a figure is evidence for a reviewer, not a substitute for one
/// (`CONTEXT.md`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RedesignationLink {
    pub from_path: String,
    pub to_path: String,
    /// The earlier of the two dates the link was observed between.
    pub from_date: String,
    /// The later of them.
    pub to_date: String,
    pub verification: VerificationState,
    /// The bill that stated the renumbering, where the link names one.
    pub bill_id: Option<String>,
}

/// Why a report did not follow a redesignation link that names its path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NotFollowed {
    /// No expression pair was given, so there is no window to resolve in.
    NoWindow,
    /// The link was checked and found wrong. Reporting a statement known to be
    /// false is worse than the string pairing this report replaces.
    Refuted,
}

/// A redesignation link the report saw and did not follow.
///
/// Said aloud rather than skipped. A silent fall back to pairing by path string
/// gives the reader today's false answer with nothing to show that a link was
/// passed over.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UnfollowedRedesignation {
    pub link: RedesignationLink,
    pub reason: NotFollowed,
}

/// One provision at a structural path, across an expression pair.
///
/// Provisions that share a path pair by position within their parent, so the
/// index can differ per side: a parent that itself shares a path may gain a
/// provision and shift every later index on one side only. Both indices are
/// therefore recorded, and neither can be derived from the other. A `None` is
/// what makes an entry an addition or a removal, and `presence` says which
/// rather than leaving the reader to infer it.
///
/// The pairing assumes that when the number of provisions at a path falls, the
/// survivors are the leading ones. A source that instead drops the first and
/// keeps the second is reported as a large edit plus a removal, rather than as
/// the removal it is. A stable provision identity is what fixes this; until
/// then position is what the source gives us. See issue #93.
#[derive(Debug, Clone, Serialize)]
pub struct ProvisionAtPath {
    /// Document-order index among the provisions at this path in the older
    /// expression, or `None` when the provision was added.
    pub from_position: Option<usize>,
    /// The same for the newer expression, or `None` when it was removed.
    pub to_position: Option<usize>,
    pub presence: Presence,
    /// Field-level changes for this provision. Always empty for an addition or
    /// a removal, which have nothing on the other side to compare against. For
    /// a move they are measured **across** the move, so "renumbered and
    /// otherwise untouched" is one answer rather than two commands.
    pub changes: Vec<PathFieldChange>,
    /// The redesignation links this entry relied on, oldest first. Empty for a
    /// provision no bill renumbered, which is the ordinary case.
    pub via: Vec<RedesignationLink>,
}

/// Everything known about one structural path: where it exists, what happened
/// to each provision there between two versions, and which annotations touch it.
#[derive(Debug, Clone, Serialize)]
pub struct PathReport {
    pub path: String,
    /// The expressions that hold the path, each named once with a count.
    ///
    /// Literal, and it stays literal. "How many provisions sit at this string
    /// on this date" is a question about the file, and its answer is a fact.
    pub present_in: Vec<PathPresence>,
    /// Each provision at the path across the requested expression pair, in
    /// document order. Empty when no pair was given.
    ///
    /// A path may hold more than one provision across a pair: where a bill
    /// renumbered, one provision left the path and another took it.
    pub provisions: Vec<ProvisionAtPath>,
    /// Redesignation links naming this path that the report did not follow,
    /// each with the reason. Every one of them when no pair was given, because
    /// then there is no window to resolve in.
    pub unfollowed_redesignations: Vec<UnfollowedRedesignation>,
    /// Annotations that reference this path (across all expression pairs).
    pub annotations: Vec<AnnotationSummary>,
}

/// Serde string form of a text content field (e.g. `"heading"`).
fn field_str(field: &crate::document::TextContentField) -> String {
    serde_json::to_value(field)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// Assemble a combined view of a single path: which expressions hold it and
/// how many provisions sit there, what happened to each provision across an
/// optional expression pair, and every annotation that references it.
///
/// `matching` decides which annotations reference the path: the whole subtree
/// beneath it, or that path alone. See [`PathMatch`].
pub fn path_report<S: Storage>(
    dataset: &S,
    path: &str,
    pair: Option<(&ExpressionId, &ExpressionId)>,
    matching: PathMatch,
) -> Result<PathReport, DatasetError> {
    let present_in = presence_counts(dataset, path)?;

    let (provisions, unfollowed_redesignations) = match pair {
        Some((from_id, to_id)) => {
            let window = Window {
                from: &from_id.at,
                to: &to_id.at,
            };
            let moves = moves_for(dataset, path, &window)?;
            match (
                dataset.get_expression(from_id)?,
                dataset.get_expression(to_id)?,
            ) {
                (Some(from), Some(to)) => (
                    pair_provisions(&from.root, &to.root, path, &moves),
                    moves.unfollowed,
                ),
                // An expression the dataset does not hold is not an error here:
                // `present_in` still answers where the path lives.
                _ => (Vec::new(), moves.unfollowed),
            }
        }
        // No window, so nothing is resolved and nothing is walked. The links
        // are still named: a reader who asks about a path that redesignation
        // links name, and gets a clean answer, has no way to learn that the
        // path names different provisions on different dates.
        None => (
            Vec::new(),
            unresolved_without_a_window(&dataset.provision_history(path)?, path),
        ),
    };

    let annotations = annotations(dataset, AnnotationQuery::Path { path, matching })?;

    Ok(PathReport {
        path: path.to_string(),
        present_in,
        provisions,
        unfollowed_redesignations,
        annotations,
    })
}

/// The two dates an expression pair covers.
struct Window<'a> {
    from: &'a str,
    to: &'a str,
}

/// Where a walk has reached: a path, and the date it holds that path on.
///
/// The date is what keeps a run of renumberings apart from a chain of them. One
/// bill moved `45X(c)(6)(R)` to (S) and (S) to (T) **in the same period**, so
/// the two links are two provisions moving at once and not one provision moving
/// twice. A hop may only start where the hop before it ended, in time as well
/// as in place.
struct Standing {
    path: String,
    date: String,
}

/// What the redesignation links say about one path across one window.
///
/// Walked out of [`provision_history`], the projection that makes the diff
/// right. Nothing here decides for a second time whether two paths are one
/// provision; it reads the links the dataset already holds.
///
/// [`provision_history`]: crate::storage::LinkReader::provision_history
#[derive(Default)]
struct Moves {
    /// Where the provision at this path went.
    out: Option<Move>,
    /// Where the provision now at this path came from.
    into: Option<Move>,
    /// Links naming this path that the walk did not follow.
    unfollowed: Vec<UnfollowedRedesignation>,
}

/// One end of a move, and the links the walk followed to reach it.
struct Move {
    /// The other path: where the provision went, or where it came from.
    other: String,
    /// The links the walk relied on, oldest first.
    via: Vec<RedesignationLink>,
}

/// What the links say about `path` across `window`, counting a renumbering a
/// bill stated about a container above it.
///
/// A bill that renumbers `(c)` to `(d)` says nothing about `(c)(1)`, and moves
/// it all the same. So the nearest container that moved answers for everything
/// below it, and the child keeps its own segments under the container's new
/// name. This is the reading [`Redesignations::is_one_provision`] already takes
/// for the diff, which is why the two commands agree.
///
/// Nearest first, because a statement about a deeper container is the more
/// specific one.
fn moves_for<S: Storage>(
    dataset: &S,
    path: &str,
    window: &Window<'_>,
) -> Result<Moves, DatasetError> {
    let mut moves = Moves::default();

    for container in ancestry(path) {
        let walked = Moves::walk(&dataset.provision_history(container)?, container, window);
        moves.unfollowed.extend(walked.unfollowed);
        moves.out = moves.out.or_else(|| carried(path, container, walked.out));
        moves.into = moves.into.or_else(|| carried(path, container, walked.into));
        if moves.out.is_some() && moves.into.is_some() {
            break;
        }
    }
    Ok(moves)
}

/// A path and every container above it, nearest first.
fn ancestry(path: &str) -> impl Iterator<Item = &str> {
    std::iter::successors(Some(path), |at| at.rsplit_once('/').map(|(above, _)| above))
}

/// Where `path` lands when the `container` above it moved.
///
/// The container's new name, with the segments that sit below it unchanged.
fn carried(path: &str, container: &str, moved: Option<Move>) -> Option<Move> {
    let moved = moved?;
    let below = &path[container.len()..];
    Some(Move {
        other: format!("{}{below}", moved.other),
        via: moved.via,
    })
}

impl Moves {
    /// Follow the links away from `path` and back to it, inside `window`.
    fn walk(history: &ProvisionHistory, path: &str, window: &Window<'_>) -> Self {
        let mut unfollowed = Vec::new();
        let out = follow(history, path, window, Forwards, &mut unfollowed);
        let into = follow(history, path, window, Backwards, &mut unfollowed);
        Self {
            out,
            into,
            unfollowed,
        }
    }
}

/// Step away from `path` through the links, one hop at a time, and answer where
/// the walk ends. `None` when it took no hop, so the provision did not move.
///
/// Where the dataset holds release points between the two asked about, a
/// provision may move more than once, so this follows the chain rather than one
/// link. Each hop must begin on the date the hop before it ended, and no hop
/// may leave the window: a renumbering outside it is a statement about another
/// period, and this report is about this one (#172).
fn follow(
    history: &ProvisionHistory,
    path: &str,
    window: &Window<'_>,
    direction: Direction,
    unfollowed: &mut Vec<UnfollowedRedesignation>,
) -> Option<Move> {
    let mut standing = direction.start(path, window);
    let mut via: Vec<RedesignationLink> = Vec::new();

    while let Some(step) = history
        .steps
        .iter()
        .find(|step| direction.continues(step, &standing, window))
    {
        // A refuted link was checked and found wrong. Following it would state
        // something known to be false, which is worse than the string pairing
        // this walk replaces, so the walk stops and says so.
        if step.verification == VerificationState::Refuted {
            unfollowed.push(UnfollowedRedesignation {
                link: RedesignationLink::from(step),
                reason: NotFollowed::Refuted,
            });
            break;
        }
        via.push(RedesignationLink::from(step));
        standing = direction.next(step);
    }

    (standing.path != path).then_some(Move {
        other: standing.path,
        via,
    })
}

/// Which way along the links a walk goes.
#[derive(Clone, Copy)]
enum Direction {
    /// Away from the path, towards the later date: what the provision here
    /// became.
    Forwards,
    /// Back from the path, towards the earlier date: what the provision here
    /// used to be.
    Backwards,
}

use Direction::{Backwards, Forwards};

impl Direction {
    /// Where the walk starts: the path asked about, on the date this direction
    /// leaves from.
    fn start(self, path: &str, window: &Window<'_>) -> Standing {
        Standing {
            path: path.to_string(),
            date: match self {
                Forwards => window.from.to_string(),
                Backwards => window.to.to_string(),
            },
        }
    }

    /// Whether this step carries the walk on from where it stands, without
    /// leaving the window.
    fn continues(self, step: &RedesignationStep, standing: &Standing, window: &Window<'_>) -> bool {
        match self {
            Forwards => {
                step.from_path == standing.path
                    && step.from_date >= standing.date
                    && step.to_date.as_str() <= window.to
            }
            Backwards => {
                step.to_path == standing.path
                    && step.to_date <= standing.date
                    && step.from_date.as_str() >= window.from
            }
        }
    }

    /// Where this step puts the walk next.
    fn next(self, step: &RedesignationStep) -> Standing {
        let (path, date) = match self {
            Forwards => (&step.to_path, &step.to_date),
            Backwards => (&step.from_path, &step.from_date),
        };
        Standing {
            path: path.clone(),
            date: date.clone(),
        }
    }
}

impl From<&RedesignationStep> for RedesignationLink {
    fn from(step: &RedesignationStep) -> Self {
        Self {
            from_path: step.from_path.clone(),
            to_path: step.to_path.clone(),
            from_date: step.from_date.clone(),
            to_date: step.to_date.clone(),
            verification: step.verification,
            bill_id: step.bill_id.clone(),
        }
    }
}

/// The redesignation links that name a path, when no expression pair was given.
///
/// One entry each, all unfollowed, because there is no window to follow them
/// in.
fn unresolved_without_a_window(
    history: &ProvisionHistory,
    path: &str,
) -> Vec<UnfollowedRedesignation> {
    history
        .steps
        .iter()
        .filter(|step| step.from_path == path || step.to_path == path)
        .map(|step| UnfollowedRedesignation {
            link: RedesignationLink::from(step),
            reason: NotFollowed::NoWindow,
        })
        .collect()
}

/// Which expressions hold the path, each named once with a provision count.
fn presence_counts<S: Storage>(dataset: &S, path: &str) -> Result<Vec<PathPresence>, DatasetError> {
    let mut ids: Vec<String> = dataset
        .find_nodes(path)?
        .into_iter()
        .map(|(id, _)| id.to_string())
        .collect();
    ids.sort();

    let mut counted: Vec<PathPresence> = Vec::new();
    for id in ids {
        match counted.last_mut() {
            Some(last) if last.expression == id => last.provisions += 1,
            _ => counted.push(PathPresence {
                expression: id,
                provisions: 1,
            }),
        }
    }
    Ok(counted)
}

/// The children of `parent` that sit at `path`, in document order.
fn kin_at<'a>(parent: &'a DocumentNode, path: &str) -> Vec<&'a DocumentNode> {
    parent
        .children
        .iter()
        .filter(|child| *child.data.path == *path)
        .collect()
}

/// The field-level changes between two dates of one provision.
///
/// The two nodes need not share a path: a renumbered provision sits at a
/// different path on each side, and "renumbered and otherwise untouched" is the
/// statement a reader needs. The pair being compared is handed to the diff as a
/// known redesignation, so the assertion the diff makes about its two arguments
/// holds by construction and this cannot panic.
fn field_changes(from: &DocumentNode, to: &DocumentNode) -> Vec<PathFieldChange> {
    let known =
        Redesignations::from_pairs([(from.data.path.to_string(), to.data.path.to_string())]);
    TreeDiff::from_nodes_with(from, to, &known)
        .changes
        .iter()
        .map(|c| PathFieldChange {
            field: field_str(&c.field_name),
            old_value: c.old_value.clone(),
            new_value: c.new_value.clone(),
        })
        .collect()
}

/// Pair the provisions at one path between two expression trees.
///
/// A provision that was added or removed is recorded on its *parent*, not at
/// the path itself, so the parent is what has to be found. Walking the diff
/// tree instead cannot answer this: it keeps only the children that record
/// something, so an unchanged provision leaves no node at all.
fn pair_provisions(
    from_root: &DocumentNode,
    to_root: &DocumentNode,
    path: &str,
    moves: &Moves,
) -> Vec<ProvisionAtPath> {
    // Where a bill renumbered, the path names one provision before and another
    // after, so pairing the two would compare two different provisions. That is
    // the whole of #165.
    if moves.out.is_some() || moves.into.is_some() {
        return pair_across_moves(from_root, to_root, path, moves);
    }

    let Some((parent_path, _)) = path.rsplit_once('/') else {
        return Vec::new();
    };

    let from_parents = from_root.find_all(parent_path);
    let to_parents = to_root.find_all(parent_path);

    // The root of an expression has no parent inside its own tree, so the
    // lookup above finds nothing. It is still a real path to ask about.
    if from_parents.is_empty() && to_parents.is_empty() {
        return root_provision(from_root, to_root, path);
    }

    // A parent path can itself name several provisions, and those pair by
    // position too. Stepping through the parents in document order keeps each
    // comparison inside the parent it belongs to, and keeps the running
    // indices in the order the provisions appear in the law.
    let mut provisions = Vec::new();
    let mut from_position = 0;
    let mut to_position = 0;

    for i in 0..from_parents.len().max(to_parents.len()) {
        let from_kin = from_parents
            .get(i)
            .map(|p| kin_at(p, path))
            .unwrap_or_default();
        let to_kin = to_parents
            .get(i)
            .map(|p| kin_at(p, path))
            .unwrap_or_default();
        let paired = from_kin.len().min(to_kin.len());

        for j in 0..paired {
            provisions.push(ProvisionAtPath {
                from_position: Some(from_position),
                to_position: Some(to_position),
                presence: Presence::InBoth,
                changes: field_changes(from_kin[j], to_kin[j]),
                via: Vec::new(),
            });
            from_position += 1;
            to_position += 1;
        }

        // Pairing is a prefix, so whichever side is longer carries the tail.
        // Under one parent a path is therefore added or removed, never both.
        for _ in paired..from_kin.len() {
            provisions.push(ProvisionAtPath {
                from_position: Some(from_position),
                to_position: None,
                presence: Presence::Removed,
                changes: Vec::new(),
                via: Vec::new(),
            });
            from_position += 1;
        }
        for _ in paired..to_kin.len() {
            provisions.push(ProvisionAtPath {
                from_position: None,
                to_position: Some(to_position),
                presence: Presence::Added,
                changes: Vec::new(),
                via: Vec::new(),
            });
            to_position += 1;
        }
    }

    provisions
}

/// Pair the provisions at a path a bill renumbered.
///
/// Each side is answered on its own, because a renumbering acts on one side at
/// a time. A provision that was here and moved away is reported against where
/// it went; a provision that is here now and came from elsewhere is reported
/// against where it came from; and whatever is left over on either side is an
/// addition or a removal. Nothing at this path is ever "in both", because the
/// two ends are two different provisions — which is the false statement #165
/// exists to remove.
fn pair_across_moves(
    from_root: &DocumentNode,
    to_root: &DocumentNode,
    path: &str,
    moves: &Moves,
) -> Vec<ProvisionAtPath> {
    let from_kin = from_root.find_all(path);
    let to_kin = to_root.find_all(path);
    let mut provisions = Vec::new();

    for (position, node) in from_kin.iter().enumerate() {
        // The other end has to be there. A bill that renumbered a container and
        // struck this provision in the same breath leaves a destination the law
        // does not hold, and naming it would state a place that does not exist.
        let moved = moves
            .out
            .as_ref()
            .and_then(|out| Some((out, *to_root.find_all(&out.other).get(position)?)));

        provisions.push(match moved {
            // Compared across the move, so "renumbered and otherwise untouched"
            // is one answer rather than a second command.
            Some((out, landed)) => ProvisionAtPath {
                from_position: Some(position),
                to_position: None,
                presence: Presence::MovedOut {
                    to_path: out.other.clone(),
                },
                changes: field_changes(node, landed),
                via: out.via.clone(),
            },
            None => ProvisionAtPath {
                from_position: Some(position),
                to_position: None,
                presence: Presence::Removed,
                changes: Vec::new(),
                via: Vec::new(),
            },
        });
    }

    for (position, node) in to_kin.iter().enumerate() {
        let moved = moves
            .into
            .as_ref()
            .and_then(|into| Some((into, *from_root.find_all(&into.other).get(position)?)));

        provisions.push(match moved {
            Some((into, left)) => ProvisionAtPath {
                from_position: None,
                to_position: Some(position),
                presence: Presence::MovedIn {
                    from_path: into.other.clone(),
                },
                changes: field_changes(left, node),
                via: into.via.clone(),
            },
            None => ProvisionAtPath {
                from_position: None,
                to_position: Some(position),
                presence: Presence::Added,
                changes: Vec::new(),
                via: Vec::new(),
            },
        });
    }

    provisions
}

/// The expression root as a single provision. Nothing inside the tree can add
/// or remove it, so it is in both expressions, in one, or in neither.
fn root_provision(
    from_root: &DocumentNode,
    to_root: &DocumentNode,
    path: &str,
) -> Vec<ProvisionAtPath> {
    match (*from_root.data.path == *path, *to_root.data.path == *path) {
        (true, true) => vec![ProvisionAtPath {
            from_position: Some(0),
            to_position: Some(0),
            presence: Presence::InBoth,
            changes: field_changes(from_root, to_root),
            via: Vec::new(),
        }],
        (true, false) => vec![ProvisionAtPath {
            from_position: Some(0),
            to_position: None,
            presence: Presence::Removed,
            changes: Vec::new(),
            via: Vec::new(),
        }],
        (false, true) => vec![ProvisionAtPath {
            from_position: None,
            to_position: Some(0),
            presence: Presence::Added,
            changes: Vec::new(),
            via: Vec::new(),
        }],
        // The path names nothing in either expression.
        (false, false) => Vec::new(),
    }
}

/// The outcome of a dataset integrity check.
#[derive(Debug, Clone, Serialize)]
pub struct ValidationReport {
    /// True when no issues were found.
    pub ok: bool,
    /// Human-readable description of each problem found.
    pub issues: Vec<String>,
    /// How many annotations were inspected.
    pub checked_annotations: usize,
}

/// Check a dataset for internal consistency:
///
/// - each work's expression dates are strictly ascending and unique,
/// - every annotation's expression pair actually exists,
/// - every annotation's `amendment_id` resolves to a real bill amendment,
/// - every annotation path names an element present in some expression.
pub fn validate<S: Storage + LegislatureReader>(
    dataset: &S,
) -> Result<ValidationReport, DatasetError> {
    let mut issues = Vec::new();

    // 1. Dates strictly ascending and unique *within each work*. Across works
    //    there is no order to check: two documents may share a date, or share
    //    none, and neither is a fault.
    for work in dataset.works()? {
        let expressions = dataset.expressions(&work)?;
        for pair in expressions.windows(2) {
            if pair[0].id.at >= pair[1].id.at {
                issues.push(format!(
                    "expressions of {work} out of order or duplicated: {} then {}",
                    pair[0].id.at, pair[1].id.at
                ));
            }
        }
    }

    // 2. Set of every amendment id across every bill.
    let mut amendment_ids = std::collections::HashSet::new();
    for bill_id in dataset.list_bill_ids()? {
        if let Some(bill) = dataset.get_bill(&bill_id)? {
            amendment_ids.extend(bill.amendments.keys().cloned());
        }
    }

    // 3 & 4. Check each annotation's pair, amendment, and paths.
    let mut checked_annotations = 0;
    for (from, to) in dataset.annotation_pairs()? {
        for end in [&from, &to] {
            if dataset.get_expression(end)?.is_none() {
                issues.push(format!(
                    "annotation pair references missing expression: {end}"
                ));
            }
        }

        let anns = dataset.get_annotations(&from, &to)?.unwrap_or_default();
        for ann in &anns {
            checked_annotations += 1;
            let amendment_id = &ann.source_bill.amendment_id;
            if !amendment_ids.contains(amendment_id) {
                issues.push(format!(
                    "annotation ({from} -> {to}) references unknown amendment id: {amendment_id}"
                ));
            }
            for path in &ann.paths {
                // Only whether the path is there, not what sits at it: asking
                // for the element loads the whole document, once per path.
                if !dataset.has_node(path)? {
                    issues.push(format!(
                        "annotation ({from} -> {to}) references path not found in any expression: {path}"
                    ));
                }
            }
        }
    }

    Ok(ValidationReport {
        ok: issues.is_empty(),
        issues,
        checked_annotations,
    })
}

/// Which paths a path filter accepts.
///
/// A bill amends a subsection, paragraph, subparagraph or clause, so that is
/// where a change annotation lands. A section is the unit a person names. The
/// two are therefore almost never the same path, and matching them for equality
/// answers nothing for most of the annotated law.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PathMatch {
    /// The path given and every path beneath it. The default, because it is
    /// what naming a section means.
    #[default]
    Subtree,
    /// Only an annotation recorded on exactly the path given.
    Exact,
}

impl PathMatch {
    /// Whether an annotation recorded on `annotated` answers for `asked`.
    fn accepts(self, asked: &str, annotated: &str) -> bool {
        match self {
            // Segment-aware, so `section_16` does not answer for `section_163`.
            Self::Subtree => crate::uslm::path::covers_path(asked, annotated),
            Self::Exact => asked == annotated,
        }
    }
}

/// Which annotations to list. The three variants map to the mutually exclusive
/// filters of the `annotations` subcommand.
pub enum AnnotationQuery<'a> {
    /// Annotations recorded for a specific expression pair.
    Pair {
        from: &'a ExpressionId,
        to: &'a ExpressionId,
    },
    /// Annotations sourced from a specific bill (across all pairs).
    Bill(&'a str),
    /// Annotations touching a specific structural path (across all pairs).
    Path {
        path: &'a str,
        /// Which paths count as touching it.
        matching: PathMatch,
    },
}

/// A flattened annotation for display, tagged with the expression pair it belongs to.
#[derive(Debug, Clone, Serialize)]
pub struct AnnotationSummary {
    /// The work both ends of the pair belong to.
    pub work: String,
    /// Older expression of the pair, as `work@date`.
    pub from: String,
    /// Newer expression of the pair, as `work@date`.
    pub to: String,
    /// Older date of the pair.
    pub from_date: String,
    /// Newer date of the pair.
    pub to_date: String,
    /// Legal operation, serde string form (e.g. `"delete"`).
    pub operation: String,
    pub bill_id: String,
    pub amendment_id: String,
    pub causative_text: String,
    /// Verification status, serde string form (e.g. `"Pending"`).
    pub status: String,
    pub confidence: Option<f32>,
    pub annotator: String,
    pub paths: Vec<String>,
}

/// Build a summary for `ann`, tagging it with the expression pair it was found under.
fn summarize(from: &ExpressionId, to: &ExpressionId, ann: &ChangeAnnotation) -> AnnotationSummary {
    AnnotationSummary {
        work: from.work.to_string(),
        from: from.to_string(),
        to: to.to_string(),
        from_date: from.at.clone(),
        to_date: to.at.clone(),
        operation: action_str(&ann.operation),
        bill_id: ann.source_bill.bill_id.clone(),
        amendment_id: ann.source_bill.amendment_id.clone(),
        causative_text: ann.source_bill.causative_text.clone(),
        status: format!("{:?}", ann.metadata.status),
        confidence: ann.metadata.confidence,
        annotator: ann.metadata.annotator.clone(),
        paths: ann.paths.clone(),
    }
}

/// List annotations matching `query`, each tagged with its expression pair.
///
/// Bill and path filters iterate every pair so the pair is always known
/// (the underlying `annotations_for_*` queries drop it).
pub fn annotations<S: Storage>(
    dataset: &S,
    query: AnnotationQuery,
) -> Result<Vec<AnnotationSummary>, DatasetError> {
    let mut out = Vec::new();
    match query {
        AnnotationQuery::Pair { from, to } => {
            for ann in dataset.get_annotations(from, to)?.unwrap_or_default() {
                out.push(summarize(from, to, &ann));
            }
        }
        AnnotationQuery::Bill(bill_id) => {
            for (from, to) in dataset.annotation_pairs()? {
                for ann in dataset.get_annotations(&from, &to)?.unwrap_or_default() {
                    if ann.source_bill.bill_id == bill_id {
                        out.push(summarize(&from, &to, &ann));
                    }
                }
            }
        }
        AnnotationQuery::Path { path, matching } => {
            for (from, to) in dataset.annotation_pairs()? {
                for ann in dataset.get_annotations(&from, &to)?.unwrap_or_default() {
                    if ann.paths.iter().any(|p| matching.accepts(path, p)) {
                        out.push(summarize(&from, &to, &ann));
                    }
                }
            }
        }
    }
    Ok(out)
}

/// The paths touched between two expressions of one work, split by kind of change.
#[derive(Debug, Clone, Serialize)]
pub struct DiffSummary {
    /// The work both expressions belong to.
    pub work: String,
    /// Older expression, as `work@date`.
    pub from: String,
    /// Newer expression, as `work@date`.
    pub to: String,
    pub from_date: String,
    pub to_date: String,
    /// Paths whose text fields changed.
    pub changed_paths: Vec<String>,
    /// Paths of elements added in the newer version.
    pub added_paths: Vec<String>,
    /// Paths of elements removed from the older version.
    pub removed_paths: Vec<String>,
    /// Elements a bill renumbered: where each was, and where it went.
    ///
    /// Empty unless the dataset holds redesignation links for the pair. Reported
    /// beside the other three rather than folded into them, because a move is
    /// none of them: reading it as a removal plus an addition is the false
    /// statement the links exist to remove (#93).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub moved_paths: Vec<MovedPath>,
}

/// One element that changed its number, for a report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MovedPath {
    pub from: String,
    pub to: String,
}

/// Recursively collect changed/added/removed/moved paths from a diff tree.
fn collect_diff_paths(diff: &TreeDiff, summary: &mut DiffSummary) {
    if !diff.changes.is_empty() {
        summary.changed_paths.push(diff.root_path.clone());
    }
    summary
        .added_paths
        .extend(diff.added.iter().map(|e| e.path.to_string()));
    summary
        .removed_paths
        .extend(diff.removed.iter().map(|e| e.path.to_string()));
    summary
        .moved_paths
        .extend(diff.moved.iter().map(|m| MovedPath {
            from: m.from.path.to_string(),
            to: m.to.path.to_string(),
        }));
    for child in &diff.child_diffs {
        collect_diff_paths(child, summary);
    }
}

/// Summarize the changes between two expressions as lists of affected paths.
pub fn diff<S: Storage>(
    dataset: &S,
    from: &ExpressionId,
    to: &ExpressionId,
) -> Result<DiffSummary, DatasetError> {
    let tree = dataset.compute_diff(from, to)?;
    let mut summary = DiffSummary {
        work: from.work.to_string(),
        from: from.to_string(),
        to: to.to_string(),
        from_date: from.at.clone(),
        to_date: to.at.clone(),
        changed_paths: Vec::new(),
        added_paths: Vec::new(),
        removed_paths: Vec::new(),
        moved_paths: Vec::new(),
    };
    collect_diff_paths(&tree, &mut summary);
    Ok(summary)
}

/// How much of the change between two versions has been annotated.
///
/// The "change universe" is every distinct path that changed, was added, or was
/// removed. `unannotated_paths` lists changed paths with no annotation. Note
/// that not every changed path is caused by an amendment (editorial edits,
/// reclassifications, etc.), so full coverage is not necessarily expected.
#[derive(Debug, Clone, Serialize)]
pub struct CoverageReport {
    /// The work both expressions belong to.
    pub work: String,
    /// Older expression, as `work@date`.
    pub from: String,
    /// Newer expression, as `work@date`.
    pub to: String,
    pub from_date: String,
    pub to_date: String,
    /// Distinct paths in the change universe.
    pub changed_path_count: usize,
    /// Change-universe paths that carry at least one annotation.
    pub annotated_count: usize,
    /// Change-universe paths with no annotation.
    pub unannotated_count: usize,
    /// The unannotated changed paths, sorted.
    pub unannotated_paths: Vec<String>,
    /// `annotated_count / changed_path_count`, or 1.0 when nothing changed.
    pub coverage: f64,
}

/// Measure annotation coverage of the diff between two expressions.
pub fn coverage<S: Storage>(
    dataset: &S,
    from: &ExpressionId,
    to: &ExpressionId,
) -> Result<CoverageReport, DatasetError> {
    let summary = diff(dataset, from, to)?;
    let mut universe = std::collections::HashSet::new();
    universe.extend(summary.changed_paths);
    universe.extend(summary.added_paths);
    universe.extend(summary.removed_paths);

    let annotated: std::collections::HashSet<String> = dataset
        .get_annotations(from, to)?
        .unwrap_or_default()
        .iter()
        .flat_map(|a| a.paths.iter().cloned())
        .collect();

    let mut unannotated_paths: Vec<String> = universe
        .iter()
        .filter(|p| !annotated.contains(*p))
        .cloned()
        .collect();
    unannotated_paths.sort();

    let changed_path_count = universe.len();
    let unannotated_count = unannotated_paths.len();
    let annotated_count = changed_path_count - unannotated_count;
    let coverage = if changed_path_count == 0 {
        1.0
    } else {
        annotated_count as f64 / changed_path_count as f64
    };

    Ok(CoverageReport {
        work: from.work.to_string(),
        from: from.to_string(),
        to: to.to_string(),
        from_date: from.at.clone(),
        to_date: to.at.clone(),
        changed_path_count,
        annotated_count,
        unannotated_count,
        unannotated_paths,
        coverage,
    })
}

/// Full-text search across every expression, returning each field match.
///
/// Backends differ in coverage: the in-memory store searches all text fields,
/// while SQLite indexes headings and content — a heading or content term is
/// found by both.
pub fn search<S: Storage>(dataset: &S, query: &str) -> Result<Vec<SearchResult>, DatasetError> {
    dataset.search_text(query)
}

/// A bill and a per-amendment summary of what it does.
#[derive(Debug, Clone, Serialize)]
pub struct BillSummary {
    pub bill_id: String,
    pub amendment_count: usize,
    /// Amendments sorted by id for stable output.
    pub amendments: Vec<AmendmentSummary>,
}

/// One amendment, without its extracted word-level changes.
#[derive(Debug, Clone, Serialize)]
pub struct AmendmentSummary {
    pub id: String,
    /// Amending actions (e.g. `delete`, `insert`) in serde string form.
    pub action_types: Vec<String>,
    pub amending_text: String,
    /// How many word-level changes have been extracted for this amendment.
    pub change_count: usize,
}

/// Serde string form of an amending action (e.g. `"repeal_and_reserve"`).
fn action_str(action: &crate::legislature::AmendingAction) -> String {
    serde_json::to_value(action)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// Summarize a single bill's amendments, or `None` if the bill isn't present.
pub fn show_bill<S: Storage + LegislatureReader>(
    dataset: &S,
    bill_id: &str,
) -> Result<Option<BillSummary>, DatasetError> {
    let Some(bill) = dataset.get_bill(bill_id)? else {
        return Ok(None);
    };

    let mut amendments: Vec<AmendmentSummary> = bill
        .amendments
        .values()
        .map(|a| AmendmentSummary {
            id: a.id.clone(),
            action_types: a.action_types.iter().map(action_str).collect(),
            amending_text: a.amending_text.clone(),
            change_count: a.changes.len(),
        })
        .collect();
    amendments.sort_by(|a, b| a.id.cmp(&b.id));

    Ok(Some(BillSummary {
        bill_id: bill.bill_id,
        amendment_count: amendments.len(),
        amendments,
    }))
}

/// How many members of one party took one position on a roll call.
#[derive(Debug, Clone, Serialize)]
pub struct PartyVoteCount {
    pub party: Party,
    pub position: VotePosition,
    pub count: usize,
}

/// One vote whose party the roll call's date cannot settle.
#[derive(Debug, Clone, Serialize)]
pub struct UnresolvedVote {
    pub bioguide_id: String,
    /// `None` when this dataset holds no member under that id.
    pub name: Option<String>,
    pub position: VotePosition,
    /// Why the party is unresolved: years two parties share, or nothing on file.
    pub party: PartyOnDate,
}

/// One roll call, with every party taken from the day the vote was held.
///
/// A member's affiliation is dated, so a roll call cannot be reported through
/// the party a member holds today: that restates a 2025 vote as a 2026 fact
/// (#105).
#[derive(Debug, Clone, Serialize)]
pub struct RollCallTally {
    pub roll_number: u32,
    /// The date of the roll call, as the source gave it.
    pub date: String,
    pub question: String,
    pub result: String,
    /// One row per party and position, in party then position order. It counts
    /// only the votes whose party the day of the roll call settles.
    pub by_party: Vec<PartyVoteCount>,
    /// The votes no row counts, each with the reason. These and `by_party`
    /// together are every vote on the roll call: a party that cannot be told is
    /// left uncounted rather than guessed at.
    pub unresolved: Vec<UnresolvedVote>,
}

/// Tally every roll call on one bill, or `None` if the bill has no votes here.
pub fn votes<S: Storage + LegislatureReader>(
    dataset: &S,
    bill_id: &str,
) -> Result<Option<Vec<RollCallTally>>, DatasetError> {
    let Some(bill_votes) = dataset.get_bill_votes(bill_id)? else {
        return Ok(None);
    };

    let mut tallies = Vec::new();
    for roll_call in &bill_votes.roll_calls {
        let day = roll_call.day();
        let mut counted: BTreeMap<(Party, VotePosition), usize> = BTreeMap::new();
        let mut unresolved = Vec::new();

        for vote in &roll_call.member_votes {
            let member = dataset.get_member(&vote.bioguide_id)?;
            let party = match (day, &member) {
                (Some(day), Some(member)) => member.party_on(day),
                // Either the date is unreadable or the member is not held. In
                // both cases the party is not known from this dataset.
                _ => PartyOnDate::Unknown,
            };

            match party.resolved() {
                Some(resolved) => {
                    *counted
                        .entry((resolved.clone(), vote.position))
                        .or_default() += 1;
                }
                None => unresolved.push(UnresolvedVote {
                    bioguide_id: vote.bioguide_id.clone(),
                    name: member.map(|member| member.name),
                    position: vote.position,
                    party,
                }),
            }
        }

        unresolved.sort_by(|a, b| a.bioguide_id.cmp(&b.bioguide_id));
        tallies.push(RollCallTally {
            roll_number: roll_call.roll_number,
            date: roll_call.date.clone(),
            question: roll_call.question.clone(),
            result: roll_call.result.clone(),
            by_party: counted
                .into_iter()
                .map(|((party, position), count)| PartyVoteCount {
                    party,
                    position,
                    count,
                })
                .collect(),
            unresolved,
        });
    }

    tallies.sort_by_key(|tally| tally.roll_number);
    Ok(Some(tallies))
}

/// Summarize a dataset's metadata and contents.
///
/// Any [`Storage`] backend answers, legislature or not. Whether this dataset
/// holds a legislature is asked at run time, through
/// [`Storage::legislature`], and the answer reaches the reader:
/// [`DatasetInfo::legislature`] is absent for a dataset that does not speak
/// legislature and zero for one that speaks it and holds none (#133).
///
/// [`Storage::legislature`]: crate::storage::Storage::legislature
pub fn info<S: Storage>(dataset: &S) -> Result<DatasetInfo, DatasetError> {
    let meta = dataset.metadata();
    let scope = Scope::derive(dataset)?;
    // Counted, never loaded: a count query costs the same on a 2 GB dataset as
    // on a small one, and building the records to count them does not.
    let links = dataset.count_links_by_kind()?;
    let legislature = dataset
        .legislature()
        .map(|legislature| legislature.legislature_counts())
        .transpose()?;
    Ok(DatasetInfo {
        name: meta.name.clone(),
        description: meta.description.clone(),
        author: meta.author.clone(),
        license: meta.license.clone(),
        version: meta.version.clone(),
        source_urls: meta.source_urls.clone(),
        work_count: scope.held.len(),
        expression_count: scope.held.iter().map(|held| held.dates.len()).sum(),
        legislature,
        link_count: links.values().sum(),
        link_counts_by_kind: links,
        reply_count: dataset.count_replies()?,
        scope,
    })
}
