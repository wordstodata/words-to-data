use std::collections::{HashMap, HashSet};
use std::str::FromStr;

use rayon::prelude::*;
use regex::Regex;
use serde::{Deserialize, Serialize};
use similar::{ChangeTag, TextDiff};
use time::Date;

use crate::constants::STOP_WORDS;
use crate::document::{DocumentNode, NodeData, TextContentField};
use crate::legislature::BillAmendment;
use crate::uslm::bill_parser::Bill;

/// A change detected in a single text content field between two document versions
///
/// This struct captures the complete details of a change to one of the five text
/// content fields (Heading, Chapeau, Proviso, Content, or Continuation) in a
/// legislative element.
///
/// The changes are computed at word-level granularity using a diff algorithm,
/// allowing precise identification of which words were inserted, deleted, or
/// remained unchanged.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct FieldChangeEvent {
    /// Which text content field changed
    pub field_name: TextContentField,

    /// The publication date of the original version
    pub from_date: Date,

    /// The publication date of the new version
    pub to_date: Date,

    /// The complete original text of the field
    pub old_value: String,

    /// The complete new text of the field
    pub new_value: String,

    /// Word-level changes showing insertions, deletions, and unchanged portions
    pub changes: Vec<TextChange>,
}

/// A single word-level change within a text field
///
/// Represents one unit of change in a diff, typically a word or whitespace token.
/// Each change has a type (Insert, Delete, or Equal) and position indices in
/// the old and new text.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct TextChange {
    /// The text value of this change (a word or whitespace)
    pub value: String,

    /// Position in the original text (None for insertions)
    pub old_index: Option<i32>,

    /// Position in the new text (None for deletions)
    pub new_index: Option<i32>,

    /// The type of change
    pub tag: TextChangeType,
}

/// The type of change for a text fragment
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextChangeType {
    /// This text was added in the new version
    Insert,
    /// This text was removed from the old version
    Delete,
    /// This text is unchanged between versions
    Equal,
}

/// A hierarchical diff between two versions of a document tree
///
/// This struct captures all changes between two versions of the same legislative
/// element and its children. It mirrors the tree structure of `DocumentNode`,
/// with diffs computed recursively for all matching children.
///
/// # Structure
///
/// The diff includes:
/// - **Field changes**: Text modifications to the element's own content fields
/// - **Added elements**: New child elements in the new version
/// - **Removed elements**: Child elements that existed in the old version but not the new
/// - **Child diffs**: Recursive diffs for child elements that exist in both versions
///
/// # Examples
///
/// ```
/// use words_to_data::{diff::TreeDiff, uslm::parser::parse};
///
/// // Parse two versions of a document
/// let old_doc = parse("tests/test_data/usc/2025-07-18/usc07.xml", "2025-07-18").unwrap();
/// let new_doc = parse("tests/test_data/usc/2025-07-30/usc07.xml", "2025-07-30").unwrap();
///
/// // Compute the diff
/// let diff = TreeDiff::from_nodes(&old_doc, &new_doc);
///
/// // Examine changes
/// println!("Field changes: {}", diff.changes.len());
/// println!("Elements added: {}", diff.added.len());
/// println!("Elements removed: {}", diff.removed.len());
/// ```
// No `Eq` or `Hash`: a `NodeData` can carry a `Provenance`, which holds an `f32`.
// Nothing hashes a diff, so neither is missed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct TreeDiff {
    /// The structural path of the element being compared
    pub root_path: String,

    /// Text content field changes for this element
    pub changes: Vec<FieldChangeEvent>,

    /// Metadata from the original version of this element
    pub from_element: NodeData,

    /// Metadata from the new version of this element
    pub to_element: NodeData,

    /// Child elements that were added in the new version
    pub added: Vec<NodeData>,

    /// Child elements that were removed from the old version
    pub removed: Vec<NodeData>,

    /// Child elements renumbered between the two versions.
    ///
    /// Empty unless the caller supplied the redesignations that apply, which is
    /// every diff computed straight from two trees
    /// ([`TreeDiff::from_nodes_with`]).
    ///
    /// A projection, like the rest of this struct: it is computed from the two
    /// documents and the links, and nothing stores it
    /// (`docs/adr/0007-a-record-is-what-was-said-everything-else-is-derived.md`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub moved: Vec<NodeMove>,

    /// Recursive diffs for child elements present in both versions
    pub child_diffs: Vec<TreeDiff>,
}

/// A child element that changed its number: the same law, addressed differently.
///
/// Reported separately from a change, an addition and a removal, because it is
/// none of those. Before this existed, a bill that struck paragraph (2) and
/// renumbered (3) as (2) read as a rewrite of (2) plus the disappearance of (3) —
/// two false statements about what the law did (#93).
///
/// The subtree below a moved element is **not** descended into. Every path inside
/// it changed with its parent, so pairing it would need the whole subtree
/// rebased, and the corpus's only case renumbered a provision whose words did
/// not change. A moved element whose contents also changed is the next case to
/// build, and there is no sample of it.
// No `Eq` or `Hash`, for the reason `TreeDiff` has none.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct NodeMove {
    /// The element as it was, at the path it had.
    pub from: NodeData,

    /// The element as it became, at the path it was given.
    pub to: NodeData,

    /// Text content changes at the element itself, when the same amendment
    /// changed its words as well as its number. Usually empty: a redesignation
    /// renumbers, and something else rewrites.
    pub changes: Vec<FieldChangeEvent>,
}

/// The redesignations that apply between two versions of a document.
///
/// A lookup from the path a provision had to the path it was given. The diff asks
/// this before it pairs by position, so a renumbered provision is paired with
/// what it became rather than with whatever took its number.
///
/// Built from either side of the model: from the links a dataset holds
/// ([`Redesignations::from_links`]), or from redesignations just read out of a
/// bill ([`Redesignations::from_pairs`]). The diff therefore depends on neither
/// storage nor a bill parser.
#[derive(Debug, Clone, Default)]
pub struct Redesignations {
    /// Old path to new path. One old path moves to one new path: a provision
    /// cannot become two.
    by_old_path: HashMap<String, String>,
}

impl Redesignations {
    /// Nothing known, so pairing is by position alone.
    ///
    /// The honest answer for a reader holding no links, and what
    /// [`TreeDiff::from_nodes`] uses.
    pub fn none() -> Self {
        Self::default()
    }

    /// Build from pairs of old path and new path.
    pub fn from_pairs(pairs: impl IntoIterator<Item = (String, String)>) -> Self {
        Self {
            by_old_path: pairs.into_iter().collect(),
        }
    }

    /// Build from the `legislature.redesignated_as` links among those given.
    ///
    /// A link of any other kind is skipped. Both ends of a redesignation link
    /// name a change — a provision as it read across two dates — so a link whose
    /// ends are shaped otherwise is not one of ours and is left alone.
    pub fn from_links(links: &[crate::link::Link]) -> Self {
        use crate::link::{LinkKind, Target};

        let redesignated_as = LinkKind::new(LinkKind::REDESIGNATED_AS);
        let path_of = |target: &Target| match target {
            Target::Change { path, .. } => Some(path.clone()),
            Target::Provision(path) => Some(path.clone()),
            _ => None,
        };
        Self::from_pairs(
            links
                .iter()
                .filter(|link| link.kind == redesignated_as)
                .filter_map(|link| Some((path_of(&link.subject)?, path_of(&link.object)?))),
        )
    }

    /// True when nothing is known, so the diff behaves exactly as it did before
    /// redesignations existed.
    pub fn is_empty(&self) -> bool {
        self.by_old_path.is_empty()
    }

    /// How many redesignations are known.
    pub fn len(&self) -> usize {
        self.by_old_path.len()
    }

    /// The path a provision was given, when one is known.
    fn new_path_of(&self, old_path: &str) -> Option<&str> {
        self.by_old_path.get(old_path).map(String::as_str)
    }
}

impl TreeDiff {
    /// Generate a regex for searching for mentions of an element
    ///
    /// Amendments and other legal texts tend to exclude chapters and titles when discussing legal references
    /// instead, they have a tendency to directly state references, and pieces of them.
    /// e.g.
    ///
    /// "According to Section 174 (a)(2)(A)"
    ///
    /// This function will generate compatible regexes for relevant strucutural elements to match those.
    pub fn mention_regex(&self) -> Option<Regex> {
        if self.root_path.contains("section") {
            let mut mreg = String::from(self.section_regex().unwrap().as_str());
            // Remove \D matcher from section regex
            mreg.truncate(mreg.len() - 2);
            let split: Vec<_> = self.root_path.split("/").collect();
            let mut started = false;
            for part in split {
                // Skip parts without underscore (like "uscode")
                let Some((part_name, part_num)) = part.split_once("_") else {
                    continue;
                };
                if started {
                    mreg += r"\(";
                    mreg += part_num;
                    mreg += r"\)\s*"
                }
                if part_name == "section" {
                    started = true;
                }
            }
            Some(Regex::from_str(mreg.as_str()).unwrap())
        } else {
            None
        }
    }

    pub fn section_regex(&self) -> Option<Regex> {
        if self.root_path.contains("section") {
            let mut regex = String::from(r"[Ss]ection\s*");
            let split: Vec<_> = self.root_path.split("/").collect();
            for part in split {
                // Skip parts without underscore (like "uscode")
                let Some((part_name, part_num)) = part.split_once("_") else {
                    continue;
                };
                if part_name == "section" {
                    regex += part_num;
                    regex += r"\D";
                    return Some(Regex::from_str(regex.as_str()).unwrap());
                }
            }
        }
        None
    }

    /// Generates a list of all candidate regexes for a TreeDiff
    pub fn all_regexes(&self) -> Vec<Regex> {
        let mut res = Vec::new();
        if let Some(sreg) = self.section_regex() {
            res.push(sreg.clone());
            if let Some(mreg) = self.mention_regex()
                && mreg.as_str() != sreg.as_str()
            {
                res.push(mreg);
            }
        }

        res
    }

    // fn all_regexes_rec(&self, visited: &mut HashSet<String>) -> Vec<Regex> {
    //     let mut regs: Vec<Regex> = Vec::new();
    //     if let Some(reg) = self.mention_regex() {
    //         let reg_str = reg.to_string();
    //         if !visited.contains(&reg_str) {
    //             visited.insert(reg_str);
    //             regs.push(reg);
    //         }
    //     }
    //     if let Some(reg) = self.section_regex() {
    //         let reg_str = reg.to_string();
    //         if !visited.contains(&reg_str) {
    //             visited.insert(reg_str);
    //             regs.push(reg);
    //         }
    //     }
    //     for child in self.child_diffs.iter() {
    //         let mut child_regs = child.all_regexes_rec(visited);
    //         regs.append(&mut child_regs);
    //     }
    //     regs
    // }

    /// Compute the diff between two document trees
    ///
    /// Compares two versions of the same legislative element and computes all
    /// changes at both the current level and recursively through all children.
    ///
    /// # Arguments
    ///
    /// * `from_element` - The original (older) version of the element
    /// * `to_element` - The new (newer) version of the element
    ///
    /// # Panics
    ///
    /// Panics if the two elements don't have the same structural path, as they
    /// must represent the same logical element in different versions.
    ///
    /// # Returns
    ///
    /// A `TreeDiff` containing all detected changes between the two versions.
    ///
    /// # Examples
    ///
    /// ```
    /// # use words_to_data::{diff::TreeDiff, uslm::parser::parse};
    /// let old = parse("tests/test_data/usc/2025-07-18/usc07.xml", "2025-07-18").unwrap();
    /// let new = parse("tests/test_data/usc/2025-07-30/usc07.xml", "2025-07-30").unwrap();
    ///
    /// let diff = TreeDiff::from_nodes(&old, &new);
    /// ```
    /// True when this diff records nothing at all: no field changes, no added or
    /// removed children, and no changed descendant.
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
            && self.added.is_empty()
            && self.removed.is_empty()
            && self.moved.is_empty()
            && self.child_diffs.is_empty()
    }

    /// Group children by path, keeping document order within each.
    ///
    /// A path can name more than one child, so the value is every child that
    /// carries it rather than one of them.
    fn children_by_path<'a>(
        children: &[&'a DocumentNode],
    ) -> HashMap<&'a str, Vec<&'a DocumentNode>> {
        let mut by_path: HashMap<&str, Vec<&DocumentNode>> = HashMap::new();
        for child in children {
            by_path.entry(&child.data.path).or_default().push(child);
        }
        by_path
    }

    /// Diff two versions of one element, pairing its children by position.
    ///
    /// Position is all a pair of documents says on its own. Where a bill
    /// renumbered a provision, pairing by position reads the renumbering as a
    /// rewrite; give the redesignations to [`TreeDiff::from_nodes_with`] and it
    /// pairs across them instead.
    pub fn from_nodes(from_element: &DocumentNode, to_element: &DocumentNode) -> TreeDiff {
        Self::from_nodes_with(from_element, to_element, &Redesignations::none())
    }

    /// Diff two versions of one element, pairing a renumbered provision with
    /// what it became.
    ///
    /// A redesignation takes precedence over position; where none is known,
    /// position is what we have (`docs/adr/0001-structural-paths-locate-not-identify.md`).
    pub fn from_nodes_with(
        from_element: &DocumentNode,
        to_element: &DocumentNode,
        known: &Redesignations,
    ) -> TreeDiff {
        assert!(from_element.data.path == to_element.data.path);
        let root_path = from_element.data.path.clone();
        // 1. Diff the root element's fields
        let changes = diff_nodes(from_element, to_element);

        // 2. Pair off the children a bill renumbered, and take them out of the
        // position pairing below. A renumbered child holds a number that was
        // somebody else's, so leaving it in would make the two fight over it.
        let renumbered = plan_moves(from_element, to_element, known);
        let moved = renumbered.moves(from_element, to_element);
        let children_left_a = renumbered.remaining(&from_element.children, Side::Old);
        let children_left_b = renumbered.remaining(&to_element.children, Side::New);

        // 3. Build HashMaps of children by path
        // A path can name more than one child: the law sometimes numbers two
        // provisions alike and the document records both (`docs/adr/0001`). So
        // each path maps to the children that carry it, in document order,
        // rather than to a single element. Keying by path alone made the second
        // provision invisible — a change to it could not be reported at all.
        let children_a = Self::children_by_path(&children_left_a);
        let children_b = Self::children_by_path(&children_left_b);

        // 4. Find added, removed, matched
        let mut added = vec![];
        let mut removed = vec![];
        let mut child_diffs = vec![];
        // Walk the children themselves, not the maps built from them. The child
        // vectors carry source document order, so all three lists come out in
        // the order the provisions appear in the law. Iterating the maps let
        // hash order decide, which changed between runs (#73).
        //
        // Where a path names several provisions, they pair by position: the
        // first on one side answers to the first on the other. Order therefore
        // carries meaning, and a swap in the source is a real change.
        let mut seen: HashMap<&str, usize> = HashMap::new();
        for child_a in &children_left_a {
            let path = &*child_a.data.path;
            let occurrence = seen.entry(path).or_insert(0);
            let index = *occurrence;
            *occurrence += 1;

            match children_b.get(path).and_then(|kin| kin.get(index)) {
                Some(child_b) => {
                    // Matched - recurse
                    // Keep any child that records something. Testing only
                    // `changes` and `child_diffs` here dropped a child whose
                    // sole content was an added or removed element, which lost
                    // every pure insertion in the tree (#54).
                    let child_diff = TreeDiff::from_nodes_with(child_a, child_b, known);
                    if !child_diff.is_empty() {
                        child_diffs.push(child_diff);
                    }
                }
                None => {
                    // Removed: nothing on the other side holds this position.
                    removed.push(child_a.data.clone()); //ElementSnapshot::from(child_a));
                }
            }
        }

        // Iterate through B for added only, again in document order.
        let mut seen = HashMap::new();
        for child_b in &children_left_b {
            let path = &*child_b.data.path;
            let occurrence = seen.entry(path).or_insert(0);
            let index = *occurrence;
            *occurrence += 1;

            if children_a
                .get(path)
                .and_then(|kin| kin.get(index))
                .is_none()
            {
                added.push(child_b.data.clone()); //ElementSnapshot::from(child_b));
            }
        }

        TreeDiff {
            changes,
            root_path: root_path.to_string(),
            from_element: from_element.data.clone(),
            to_element: to_element.data.clone(),
            added,
            removed,
            moved,
            child_diffs,
        }
    }

    /// Search for a diff by its structural path
    ///
    /// Recursively searches this element and all descendants for an element
    /// with the specified path. The path must be a fully qualified structural
    /// path (e.g., "uscode/title_7/chapter_1/section_1").
    ///
    /// # Arguments
    ///
    /// * `path` - The full structural path of the element to find
    ///
    /// # Returns
    ///
    /// Returns `Some(&TreeDiff)` if an element with the matching path is found,
    /// or `None` if no such element exists in this tree.
    pub fn find(&self, path: &str) -> Option<&TreeDiff> {
        self.find_all(path).into_iter().next()
    }

    /// Every diff node at this structural path, in document order
    ///
    /// A path can name more than one provision, so it can name more than one
    /// diff node. Prefer this over [`TreeDiff::find`] wherever taking the first
    /// would quietly drop a change to the others.
    pub fn find_all(&self, path: &str) -> Vec<&TreeDiff> {
        if path == self.root_path.as_str() {
            return vec![self];
        }
        // Requiring the separator keeps a shared prefix from reading as a
        // descendant, and leaves nothing to assert about.
        let Some(remaining) = path
            .strip_prefix(self.root_path.as_str())
            .and_then(|rest| rest.strip_prefix('/'))
        else {
            return Vec::new();
        };

        let segment = remaining.split('/').next().unwrap_or(remaining);
        let child_path = format!("{}/{segment}", self.root_path);

        self.child_diffs
            .iter()
            // Whole path, not a suffix of it.
            .filter(|child| child.root_path == child_path)
            .flat_map(|child| child.find_all(path))
            .collect()
    }

    /// Calculate the similarity of diffs in the TreeDiff with the amendment data from a bill
    ///
    /// Returns a hashmap keyed by the root_path in the tree diff. Each value
    /// holds every amendment that scores above zero at that path, best score
    /// first, ties broken by amendment id.
    ///
    /// More than one amendment can genuinely explain one change: one strikes
    /// text while another inserts at the same subsection. Keeping only the
    /// highest score discarded the rest, so the caller now narrows the list
    /// itself, by cutoff or by asking a model (#75).
    pub fn calculate_amendment_similarities(
        &self,
        data: &Bill,
    ) -> HashMap<String, Vec<AmendmentSimilarity>> {
        let mut result: HashMap<String, Vec<AmendmentSimilarity>> = HashMap::new();
        self.calculate_similarities_recursive(&mut result, data);

        // The amendments were walked in hash order, so sort each path's list.
        // Score alone is not enough to settle it: ties are common, and every
        // difference seen between two runs was a tie (#75).
        for similarities in result.values_mut() {
            similarities.sort_by(|a, b| {
                b.score
                    .total_cmp(&a.score)
                    .then_with(|| a.amendment_id.cmp(&b.amendment_id))
            });
        }
        result
    }

    fn calculate_similarities_recursive(
        &self,
        result: &mut HashMap<String, Vec<AmendmentSimilarity>>,
        data: &Bill,
    ) {
        // Check if this TreeDiff has any changes
        if !self.changes.is_empty() {
            // Keep every amendment that explains any of them
            for (amendment_id, amendment) in &data.amendments {
                if amendment.changes.is_empty() {
                    continue;
                }

                let similarity = self.calculate_match_with_amendment(amendment_id, amendment);

                if similarity.score > 0.0 {
                    result
                        .entry(self.root_path.clone())
                        .or_default()
                        .push(similarity);
                }
            }
        }

        // Recurse into children
        for child_diff in &self.child_diffs {
            child_diff.calculate_similarities_recursive(result, data);
        }
    }

    fn calculate_match_with_amendment(
        &self,
        amendment_id: &str,
        amendment: &BillAmendment,
    ) -> AmendmentSimilarity {
        // Collect all changed words from this TreeDiff (deletions + insertions)
        let tree_diff_words: HashSet<String> = self.collect_tree_diff_words();
        let tree_diff_count = tree_diff_words.len();

        // Find the best-matching BillDiff within this amendment
        let mut best_score = 0.0_f32;
        let mut best_precision = 0.0_f32;
        let mut best_recall = 0.0_f32;
        let mut best_matched = 0_i32;

        for bill_diff in &amendment.changes {
            // Collect words from this specific BillDiff
            let mut bill_diff_words: HashSet<String> = HashSet::new();
            for word in &bill_diff.removed {
                let trimmed = word.trim();
                if !trimmed.is_empty() && !is_stop_word(trimmed) {
                    bill_diff_words.insert(trimmed.to_lowercase());
                }
            }
            for word in &bill_diff.added {
                let trimmed = word.trim();
                if !trimmed.is_empty() && !is_stop_word(trimmed) {
                    bill_diff_words.insert(trimmed.to_lowercase());
                }
            }

            if bill_diff_words.is_empty() {
                continue;
            }

            // Calculate intersection with this BillDiff
            let matched_words: i32 = tree_diff_words
                .iter()
                .filter(|w| bill_diff_words.contains(*w))
                .count() as i32;

            let bill_diff_count = bill_diff_words.len();

            // Calculate precision: how well this BillDiff explains TreeDiff
            let precision = if tree_diff_count > 0 {
                matched_words as f32 / tree_diff_count as f32
            } else {
                0.0
            };

            // Calculate recall: how much of this BillDiff is in TreeDiff
            let recall = if bill_diff_count > 0 {
                matched_words as f32 / bill_diff_count as f32
            } else {
                0.0
            };

            // Calculate F1 score for this BillDiff
            let score = if precision + recall > 0.0 {
                2.0 * precision * recall / (precision + recall)
            } else {
                0.0
            };

            // Keep the best match
            if score > best_score {
                best_score = score;
                best_precision = precision;
                best_recall = recall;
                best_matched = matched_words;
            }
        }

        AmendmentSimilarity {
            tree_diff_path: self.root_path.clone(),
            amendment_id: amendment_id.to_string(),
            score: best_score,
            precision: best_precision,
            recall: best_recall,
            matched_words: best_matched,
            tree_diff_words: tree_diff_count as i32,
        }
    }

    /// Collect all significant changed words from this TreeDiff
    fn collect_tree_diff_words(&self) -> HashSet<String> {
        let mut words = HashSet::new();
        for field_change in &self.changes {
            for text_change in &field_change.changes {
                let word = text_change.value.trim();
                // Skip empty strings and stop words (case-insensitive)
                if word.is_empty() || is_stop_word(word) {
                    continue;
                }
                match text_change.tag {
                    TextChangeType::Delete | TextChangeType::Insert => {
                        words.insert(word.to_lowercase());
                    }
                    TextChangeType::Equal => {}
                }
            }
        }
        words
    }

    /// Scan all amendment texts for mentions of changed sections.
    ///
    /// Uses the regexes from `all_regexes()` to find section mentions in each
    /// amendment's `amending_text`. This helps identify which amendments might
    /// be responsible for changes at specific structural paths.
    ///
    /// # Arguments
    ///
    /// * `data` - Bill data from a parsed bill
    ///
    /// # Returns
    ///
    /// A map from amendment_id to list of matches found in that amendment's text.
    /// For each tree_diff_path, only the most specific (longest) match is kept.
    pub fn scan_for_mentions(&self, data: &Bill) -> HashMap<String, Vec<MentionMatch>> {
        // Collect all regexes with their source paths recursively
        let regex_with_paths = self.collect_regexes_with_paths();

        // Scan each amendment's text against all regexes
        let mut results: HashMap<String, Vec<MentionMatch>> = HashMap::new();

        for (amendment_id, amendment) in &data.amendments {
            let text = &amendment.amending_text;
            let all_matches: Vec<MentionMatch> = regex_with_paths
                .par_iter()
                .filter_map(|(path, reg)| {
                    reg.find(text).map(|mat| MentionMatch {
                        tree_diff_path: path.clone(),
                        matched_text: mat.as_str().to_string(),
                    })
                })
                .collect();

            // Deduplicate: keep only the longest match per tree_diff_path.
            // Collecting into a Vec rather than a map keeps the paths in the
            // order the tree walk produced them, which is document order. A map
            // returned them in hash order, which moved between runs (#73).
            let mut best_by_path: Vec<&MentionMatch> = Vec::new();
            for m in &all_matches {
                match best_by_path
                    .iter_mut()
                    .find(|existing| existing.tree_diff_path == m.tree_diff_path)
                {
                    // Ties keep the match already held, as before.
                    Some(existing) => {
                        if m.matched_text.len() > existing.matched_text.len() {
                            *existing = m;
                        }
                    }
                    None => best_by_path.push(m),
                }
            }
            let matches: Vec<MentionMatch> = best_by_path.into_iter().cloned().collect();

            if !matches.is_empty() {
                results.insert(amendment_id.clone(), matches);
            }
        }

        results
    }

    /// Return a shallow copy of this TreeDiff without children.
    ///
    /// Useful when correlating a specific diff node with other data
    /// without needing the full subtree.
    pub fn shallow(&self) -> TreeDiff {
        TreeDiff {
            root_path: self.root_path.clone(),
            changes: self.changes.clone(),
            from_element: self.from_element.clone(),
            to_element: self.to_element.clone(),
            added: self.added.clone(),
            removed: self.removed.clone(),
            moved: self.moved.clone(),
            child_diffs: vec![],
        }
    }

    /// Recursively collect regexes with their source paths from this TreeDiff.
    fn collect_regexes_with_paths(&self) -> Vec<(String, Regex)> {
        let mut result = Vec::new();

        // Add regexes from this node
        for reg in self.all_regexes() {
            result.push((self.root_path.clone(), reg));
        }

        // Recurse into children
        for child in &self.child_diffs {
            result.extend(child.collect_regexes_with_paths());
        }

        result
    }
}

/// Similarity between a TreeDiff and a bill amendment
///
/// Used to rank how likely a BillAmendment caused the changes at a TreeDiff location.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AmendmentSimilarity {
    /// The structural path of the TreeDiff node
    pub tree_diff_path: String,
    /// The ID of the matched BillAmendment
    pub amendment_id: String,
    /// Primary ranking metric (precision-weighted F1)
    pub score: f32,
    /// How well the amendment explains the TreeDiff's changes
    /// |TreeDiff ∩ Amendment| / |TreeDiff|
    pub precision: f32,
    /// How much of the amendment is represented in this TreeDiff
    /// |TreeDiff ∩ Amendment| / |Amendment|
    pub recall: f32,
    /// Number of words that matched between TreeDiff and Amendment
    pub matched_words: i32,
    /// Total significant words in the TreeDiff's changes
    pub tree_diff_words: i32,
}

/// A match found when scanning amendment text for section mentions.
///
/// When scanning bill amendments against a TreeDiff's regexes, this struct
/// captures each match, linking the structural path from the TreeDiff to
/// the text that matched in the amendment.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MentionMatch {
    /// The structural path from the TreeDiff that generated this match
    pub tree_diff_path: String,
    /// The text that matched the regex pattern
    pub matched_text: String,
}

/// Which of an element's children paired across a renumbering.
///
/// Held as index pairs into the two child lists, because both sides need to be
/// taken out of the position pairing and a path cannot say which occurrence.
#[derive(Debug, Default)]
struct MovePlan {
    /// One old child's index against the new child's index it became.
    pairs: Vec<(usize, usize)>,
}

/// Which side of a diff a child list belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    Old,
    New,
}

/// Pair off the children a bill renumbered.
///
/// Old-document order, so the answer does not depend on a hash. A new child is
/// claimed once: two old provisions renumbered to one path would be a statement
/// the bill did not make, and the second is left to position pairing rather than
/// paired over the first.
fn plan_moves(
    from_element: &DocumentNode,
    to_element: &DocumentNode,
    known: &Redesignations,
) -> MovePlan {
    if known.is_empty() {
        return MovePlan::default();
    }
    let mut pairs = Vec::new();
    let mut claimed: HashSet<usize> = HashSet::new();
    for (old_index, child_a) in from_element.children.iter().enumerate() {
        let Some(new_path) = known.new_path_of(&child_a.data.path) else {
            continue;
        };
        let found = to_element
            .children
            .iter()
            .enumerate()
            .find(|(new_index, child_b)| {
                *child_b.data.path == *new_path && !claimed.contains(new_index)
            });
        if let Some((new_index, _)) = found {
            claimed.insert(new_index);
            pairs.push((old_index, new_index));
        }
    }
    MovePlan { pairs }
}

impl MovePlan {
    /// The moves themselves, with any change to the moved element's own words.
    fn moves(&self, from_element: &DocumentNode, to_element: &DocumentNode) -> Vec<NodeMove> {
        self.pairs
            .iter()
            .filter_map(|(old_index, new_index)| {
                let child_a = from_element.children.get(*old_index)?;
                let child_b = to_element.children.get(*new_index)?;
                Some(NodeMove {
                    from: child_a.data.clone(),
                    to: child_b.data.clone(),
                    changes: field_changes(child_a, child_b),
                })
            })
            .collect()
    }

    /// The children on one side that no move claimed, in document order.
    fn remaining<'a>(&self, children: &'a [DocumentNode], side: Side) -> Vec<&'a DocumentNode> {
        let claimed = |index: usize| {
            self.pairs.iter().any(|(old_index, new_index)| match side {
                Side::Old => *old_index == index,
                Side::New => *new_index == index,
            })
        };
        children
            .iter()
            .enumerate()
            .filter(|(index, _)| !claimed(*index))
            .map(|(_, child)| child)
            .collect()
    }
}

/// Check if a word is a stop word (case-insensitive)
fn is_stop_word(word: &str) -> bool {
    let lower = word.to_lowercase();
    STOP_WORDS.contains(&lower.as_str())
}

/// Compute field-level changes between two elements
///
/// Compares all five text content fields (Heading, Chapeau, Proviso, Content,
/// Continuation) between two versions of the same element and returns change
/// events for any fields that differ.
///
/// # Arguments
///
/// * `element_a` - The original version of the element
/// * `element_b` - The new version of the element
///
/// # Returns
///
/// A vector of `FieldChangeEvent` for each field that has changes.
/// Fields that are identical in both versions are omitted.
///
/// # Panics
///
/// Panics if the elements have different paths or types.
pub fn diff_nodes(element_a: &DocumentNode, element_b: &DocumentNode) -> Vec<FieldChangeEvent> {
    assert!(element_a.data.path == element_b.data.path);
    // Two nodes at one path must be the same kind of thing to be one provision
    // across two dates. The type is an open string, so this compares what the
    // producer wrote rather than a vocabulary the core owns (#129).
    assert!(element_a.data.node_type == element_b.data.node_type);
    field_changes(element_a, element_b)
}

/// Compute field-level changes between two elements that need not share a path.
///
/// What [`diff_nodes`] does, without its two checks. A renumbered provision sits
/// at a different path on each side, by definition, and a bill can renumber it to
/// another level as well — `redesignating paragraph (1) as subparagraph (A)` — so
/// neither the path nor the type can be required to match.
fn field_changes(element_a: &DocumentNode, element_b: &DocumentNode) -> Vec<FieldChangeEvent> {
    let mut changes: Vec<FieldChangeEvent> = Vec::new();
    for field_name in [
        TextContentField::Heading,
        TextContentField::Chapeau,
        TextContentField::Proviso,
        TextContentField::Content,
        TextContentField::Continuation,
    ]
    .into_iter()
    {
        let field_changes = diff_field(element_a, element_b, field_name);
        // Only include if there are actual changes (Insert or Delete), not just Equal
        let has_real_changes = field_changes
            .changes
            .iter()
            .any(|c| c.tag != TextChangeType::Equal);
        if has_real_changes {
            changes.push(field_changes);
        }
    }
    changes
}

// databases don't like usizes, make it an i32
// text content will never exceed the i32 range
fn rewrap_usize(s: Option<usize>) -> Option<i32> {
    s.map(|val| val as i32)
}

fn diff_field(
    element_a: &DocumentNode,
    element_b: &DocumentNode,
    field_name: TextContentField,
) -> FieldChangeEvent {
    let a = element_a
        .data
        .get_text_content(field_name)
        .unwrap_or_default();
    let b = element_b
        .data
        .get_text_content(field_name)
        .unwrap_or_default();

    let diff = TextDiff::from_words(a.as_ref(), b.as_ref());
    let changes: Vec<TextChange> = diff
        .iter_all_changes()
        // Keep all changes including Equal for word-level diff rendering
        .map(|c| {
            let tag = match c.tag() {
                ChangeTag::Delete => TextChangeType::Delete,
                ChangeTag::Insert => TextChangeType::Insert,
                ChangeTag::Equal => TextChangeType::Equal,
            };
            TextChange {
                value: String::from(c.value()),
                old_index: rewrap_usize(c.old_index()),
                new_index: rewrap_usize(c.new_index()),
                tag,
            }
        })
        .collect();
    FieldChangeEvent {
        field_name,
        from_date: element_a.data.date,
        to_date: element_b.data.date,
        old_value: a.to_string(),
        new_value: b.to_string(),
        changes,
    }
}
