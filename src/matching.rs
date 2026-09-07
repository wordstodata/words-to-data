//! Candidate gathering for amendment matching.
//!
//! For one diff, this collects the locations each bill amendment might explain,
//! drawn from two channels: pre-computed similarity scores, and section
//! mentions found in the amendment text. The result is the candidate set that
//! `match-amendments` shows an LLM.
//!
//! This lives in the library rather than in the binary so that candidate
//! ordering can be tested. The order matters: it is the order the model reads
//! its options in, so it must not change between runs (#73).

use std::collections::HashMap;

use crate::dataset::Dataset;
use crate::diff::{AmendmentSimilarity, MentionMatch, TreeDiff};
use crate::legislature::AmendingAction;
use crate::storage::Storage;

/// A candidate US Code diff that an amendment might have caused.
pub struct Candidate {
    /// Shallow diff at this location (no children).
    pub diff: TreeDiff,
    /// Pre-computed similarity score for this amendment/location, if any.
    pub similarity: Option<AmendmentSimilarity>,
    /// Section mentions from the amendment text found at this location.
    pub mentions: Vec<MentionMatch>,
}

/// One amendment plus the candidate diffs to disambiguate between.
pub struct AmendmentMatch {
    pub bill_id: String,
    pub amendment_id: String,
    pub amending_text: String,
    pub action_types: Vec<AmendingAction>,
    pub candidates: Vec<Candidate>,
}

/// Number every node of the diff tree by its position in a document-order walk.
///
/// Candidate locations arrive from hash maps, so they carry no usable order of
/// their own. This index gives each one the position its provision holds in the
/// law, so candidates can be presented in the order a reader would meet them.
fn document_order_index(diff: &TreeDiff) -> HashMap<&str, usize> {
    fn walk<'a>(diff: &'a TreeDiff, index: &mut HashMap<&'a str, usize>) {
        let next = index.len();
        index.entry(diff.root_path.as_str()).or_insert(next);
        for child in &diff.child_diffs {
            walk(child, index);
        }
    }

    let mut index = HashMap::new();
    walk(diff, &mut index);
    index
}

/// The similarity score a candidate must beat to be worth showing a model.
///
/// The same value `score-amendments` uses, so the two commands agree on what
/// counts as a plausible explanation.
pub const DEFAULT_SIMILARITY_CUTOFF: f32 = 0.4;

/// Gather, for every amendment across every bill, the candidate diffs it may explain.
///
/// `similarity_cutoff` drops weak scores before they become candidates. Scoring
/// returns every amendment above zero at a path, a far larger set than the one
/// per path it used to return, so the cutoff is what holds the candidate volume
/// down (#75).
pub fn build_matches(
    dataset: &Dataset<impl Storage>,
    diff: &TreeDiff,
    similarity_cutoff: f32,
) -> Vec<AmendmentMatch> {
    let mut matches = Vec::new();
    let path_order = document_order_index(diff);

    // Bills and amendments are both held in hash maps, so neither arrives in a
    // usable order. Their ids are stable (an amendment id is a hash of its own
    // text), so ordering by id gives the same match list on every run.
    let mut bill_ids = dataset.list_bill_ids().expect("Error listing bills");
    bill_ids.sort();

    for bill_id in bill_ids {
        let bill = dataset
            .get_bill(&bill_id)
            .expect("Error reading bill")
            .expect("bill id from list_bill_ids should exist");

        // Similarity scores keyed by tree-diff path; mentions keyed by amendment id.
        let similarities = diff.calculate_amendment_similarities(&bill);
        let mentions = diff.scan_for_mentions(&bill);

        let mut amendments: Vec<_> = bill.amendments.values().collect();
        amendments.sort_by(|a, b| a.id.cmp(&b.id));

        for amendment in amendments {
            let amd_scores: Vec<&AmendmentSimilarity> = similarities
                .values()
                .flatten()
                .filter(|s| s.amendment_id == amendment.id && s.score > similarity_cutoff)
                .collect();
            let empty = Vec::new();
            let amd_mentions = mentions.get(&amendment.id).unwrap_or(&empty);

            // Union of every location implicated by a score or a mention. Both
            // channels come out of hash maps, so sort the union into document
            // order before building candidates; otherwise the model sees its
            // options shuffled differently on every run.
            let mut paths: Vec<&str> = amd_scores
                .iter()
                .map(|s| s.tree_diff_path.as_str())
                .chain(amd_mentions.iter().map(|m| m.tree_diff_path.as_str()))
                .collect();
            paths.sort_unstable_by_key(|path| {
                // A path missing from the index cannot be ordered by position,
                // so fall back to the path itself and keep it deterministic.
                (path_order.get(path).copied().unwrap_or(usize::MAX), *path)
            });
            paths.dedup();

            let mut candidates = Vec::new();
            for path in paths {
                let similarity = amd_scores
                    .iter()
                    .find(|s| s.tree_diff_path == path)
                    .map(|s| (*s).clone());
                let cand_mentions: Vec<MentionMatch> = amd_mentions
                    .iter()
                    .filter(|m| m.tree_diff_path == path)
                    .cloned()
                    .collect();

                // Only keep locations that actually have changes.
                if let Some(node) = diff.find(path) {
                    let shallow = node.shallow();
                    if !shallow.added.is_empty()
                        || !shallow.removed.is_empty()
                        || !shallow.changes.is_empty()
                    {
                        candidates.push(Candidate {
                            diff: shallow,
                            similarity,
                            mentions: cand_mentions,
                        });
                    }
                }
            }

            if !candidates.is_empty() {
                matches.push(AmendmentMatch {
                    bill_id: bill.bill_id.clone(),
                    amendment_id: amendment.id.clone(),
                    amending_text: amendment.amending_text.clone(),
                    action_types: amendment.action_types.clone(),
                    candidates,
                });
            }
        }
    }

    matches
}
