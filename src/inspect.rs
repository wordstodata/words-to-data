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

use serde::Serialize;

use crate::annotation::ChangeAnnotation;
use crate::dataset::{DatasetError, Scope, SearchResult};
use crate::diff::TreeDiff;
use crate::storage::Storage;
use crate::uslm::USLMElement;

/// Top-level summary of a dataset: its metadata plus headline counts.
#[derive(Debug, Clone, Serialize)]
pub struct DatasetInfo {
    pub name: String,
    pub description: String,
    pub author: String,
    pub license: String,
    pub version: String,
    pub source_urls: Vec<String>,
    /// Number of version snapshots (US Code release points).
    pub version_count: usize,
    /// Number of bills recorded in the dataset.
    pub bill_count: usize,
    /// What this dataset covers, so a caller can tell "absent from the law"
    /// from "absent from this dataset".
    pub scope: Scope,
}

/// One version snapshot's headline facts (no element tree).
#[derive(Debug, Clone, Serialize)]
pub struct VersionSummary {
    /// Publication date, `YYYY-MM-DD`.
    pub date: String,
    /// Optional human-readable label.
    pub label: Option<String>,
    /// Total elements in the version's document tree (root included).
    pub element_count: usize,
}

/// Count every element in a tree, including the root.
fn count_elements(element: &USLMElement) -> usize {
    1 + element.children.iter().map(count_elements).sum::<usize>()
}

/// List every version chronologically with its label and element count.
pub fn versions<S: Storage>(dataset: &S) -> Result<Vec<VersionSummary>, DatasetError> {
    dataset
        .list_versions()?
        .into_iter()
        .map(|info| {
            let element_count = dataset
                .get_version(&info.date)?
                .map(|snap| count_elements(&snap.element))
                .unwrap_or(0);
            Ok(VersionSummary {
                date: info.date,
                label: info.label,
                element_count,
            })
        })
        .collect()
}

/// A single field's change at a path between two versions.
#[derive(Debug, Clone, Serialize)]
pub struct PathFieldChange {
    /// Which text field changed, serde string form (e.g. `"heading"`).
    pub field: String,
    pub old_value: String,
    pub new_value: String,
}

/// Everything known about one structural path: where it exists, what changed
/// there between two versions, and which annotations touch it.
#[derive(Debug, Clone, Serialize)]
pub struct PathReport {
    pub path: String,
    /// Dates of the versions in which the element exists, ascending.
    pub present_in: Vec<String>,
    /// Field-level changes for the requested version pair (empty if no pair given).
    pub changes: Vec<PathFieldChange>,
    /// Annotations that reference this path (across all version pairs).
    pub annotations: Vec<AnnotationSummary>,
}

/// Serde string form of a text content field (e.g. `"heading"`).
fn field_str(field: &crate::uslm::TextContentField) -> String {
    serde_json::to_value(field)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// Assemble a combined view of a single path: presence, field changes for an
/// optional version pair, and every annotation that references it.
pub fn path_report<S: Storage>(
    dataset: &S,
    path: &str,
    pair: Option<(&str, &str)>,
) -> Result<PathReport, DatasetError> {
    let mut present_in: Vec<String> = dataset
        .find_element(path)?
        .into_iter()
        .map(|(date, _)| date)
        .collect();
    present_in.sort();

    let changes = match pair {
        Some((from, to)) => dataset
            .compute_diff(from, to)?
            .find(path)
            .map(|node| {
                node.changes
                    .iter()
                    .map(|c| PathFieldChange {
                        field: field_str(&c.field_name),
                        old_value: c.old_value.clone(),
                        new_value: c.new_value.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default(),
        None => Vec::new(),
    };

    let annotations = annotations(dataset, AnnotationQuery::Path(path))?;

    Ok(PathReport {
        path: path.to_string(),
        present_in,
        changes,
        annotations,
    })
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
/// - version dates are strictly ascending and unique,
/// - every annotation's version pair actually exists,
/// - every annotation's `amendment_id` resolves to a real bill amendment,
/// - every annotation path names an element present in some version.
pub fn validate<S: Storage>(dataset: &S) -> Result<ValidationReport, DatasetError> {
    let mut issues = Vec::new();

    // 1. Versions strictly ascending and unique by date.
    let versions = dataset.list_versions()?;
    for pair in versions.windows(2) {
        if pair[0].date >= pair[1].date {
            issues.push(format!(
                "versions out of order or duplicated: {} then {}",
                pair[0].date, pair[1].date
            ));
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
        if dataset.get_version(&from)?.is_none() {
            issues.push(format!(
                "annotation pair references missing version: {from}"
            ));
        }
        if dataset.get_version(&to)?.is_none() {
            issues.push(format!("annotation pair references missing version: {to}"));
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
                if dataset.find_element(path)?.is_empty() {
                    issues.push(format!(
                        "annotation ({from} -> {to}) references path not found in any version: {path}"
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

/// Which annotations to list. The three variants map to the mutually exclusive
/// filters of the `annotations` subcommand.
pub enum AnnotationQuery<'a> {
    /// Annotations recorded for a specific version pair.
    Pair { from: &'a str, to: &'a str },
    /// Annotations sourced from a specific bill (across all version pairs).
    Bill(&'a str),
    /// Annotations touching a specific structural path (across all version pairs).
    Path(&'a str),
}

/// A flattened annotation for display, tagged with the version pair it belongs to.
#[derive(Debug, Clone, Serialize)]
pub struct AnnotationSummary {
    /// Older version date of the pair this annotation belongs to.
    pub from_date: String,
    /// Newer version date of the pair this annotation belongs to.
    pub to_date: String,
    /// Legal operation, serde string form (e.g. `"strike"`).
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

/// Build a summary for `ann`, tagging it with the version pair it was found under.
fn summarize(from: &str, to: &str, ann: &ChangeAnnotation) -> AnnotationSummary {
    AnnotationSummary {
        from_date: from.to_string(),
        to_date: to.to_string(),
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

/// List annotations matching `query`, each tagged with its version pair.
///
/// Bill and path filters iterate every version pair so the pair is always known
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
        AnnotationQuery::Path(path) => {
            for (from, to) in dataset.annotation_pairs()? {
                for ann in dataset.get_annotations(&from, &to)?.unwrap_or_default() {
                    if ann.paths.iter().any(|p| p == path) {
                        out.push(summarize(&from, &to, &ann));
                    }
                }
            }
        }
    }
    Ok(out)
}

/// The paths touched between two versions, split by kind of change.
#[derive(Debug, Clone, Serialize)]
pub struct DiffSummary {
    pub from_date: String,
    pub to_date: String,
    /// Paths whose text fields changed.
    pub changed_paths: Vec<String>,
    /// Paths of elements added in the newer version.
    pub added_paths: Vec<String>,
    /// Paths of elements removed from the older version.
    pub removed_paths: Vec<String>,
}

/// Recursively collect changed/added/removed paths from a diff tree.
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
    for child in &diff.child_diffs {
        collect_diff_paths(child, summary);
    }
}

/// Summarize the changes between two versions as lists of affected paths.
pub fn diff<S: Storage>(dataset: &S, from: &str, to: &str) -> Result<DiffSummary, DatasetError> {
    let tree = dataset.compute_diff(from, to)?;
    let mut summary = DiffSummary {
        from_date: from.to_string(),
        to_date: to.to_string(),
        changed_paths: Vec::new(),
        added_paths: Vec::new(),
        removed_paths: Vec::new(),
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

/// Measure annotation coverage of the diff between two versions.
pub fn coverage<S: Storage>(
    dataset: &S,
    from: &str,
    to: &str,
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
        from_date: from.to_string(),
        to_date: to.to_string(),
        changed_path_count,
        annotated_count,
        unannotated_count,
        unannotated_paths,
        coverage,
    })
}

/// Full-text search across every version, returning each field match.
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
    /// Amending actions (e.g. `strike`, `insert`) in serde string form.
    pub action_types: Vec<String>,
    pub amending_text: String,
    /// How many word-level changes have been extracted for this amendment.
    pub change_count: usize,
}

/// Serde string form of an amending action (e.g. `"strikeandinsert"`).
fn action_str(action: &crate::legislature::AmendingAction) -> String {
    serde_json::to_value(action)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// Summarize a single bill's amendments, or `None` if the bill isn't present.
pub fn show_bill<S: Storage>(
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

/// Summarize a dataset's metadata and contents.
pub fn info<S: Storage>(dataset: &S) -> Result<DatasetInfo, DatasetError> {
    let meta = dataset.metadata();
    Ok(DatasetInfo {
        name: meta.name.clone(),
        description: meta.description.clone(),
        author: meta.author.clone(),
        license: meta.license.clone(),
        version: meta.version.clone(),
        source_urls: meta.source_urls.clone(),
        version_count: dataset.list_versions()?.len(),
        bill_count: dataset.list_bill_ids()?.len(),
        scope: Scope::derive(dataset)?,
    })
}
