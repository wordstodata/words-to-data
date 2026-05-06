use std::collections::{HashMap, HashSet};
use std::str::FromStr;

use rayon::prelude::*;
use regex::Regex;
use serde::{Deserialize, Serialize};
use similar::{ChangeTag, TextDiff};
use time::Date;

use crate::constants::STOP_WORDS;
use crate::uslm::{BillAmendment, ElementData, TextContentField, USLMElement, bill_parser::Bill};

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

/// A hierarchical diff between two versions of a USLM document tree
///
/// This struct captures all changes between two versions of the same legislative
/// element and its children. It mirrors the tree structure of `USLMElement`,
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
/// let diff = TreeDiff::from_elements(&old_doc, &new_doc);
///
/// // Examine changes
/// println!("Field changes: {}", diff.changes.len());
/// println!("Elements added: {}", diff.added.len());
/// println!("Elements removed: {}", diff.removed.len());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct TreeDiff {
    /// The structural path of the element being compared
    pub root_path: String,

    /// Text content field changes for this element
    pub changes: Vec<FieldChangeEvent>,

    /// Metadata from the original version of this element
    pub from_element: ElementData,

    /// Metadata from the new version of this element
    pub to_element: ElementData,

    /// Child elements that were added in the new version
    pub added: Vec<ElementData>,

    /// Child elements that were removed from the old version
    pub removed: Vec<ElementData>,

    /// Recursive diffs for child elements present in both versions
    pub child_diffs: Vec<TreeDiff>,
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

    /// Compute the diff between two USLM element trees
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
    /// let diff = TreeDiff::from_elements(&old, &new);
    /// assert_eq!(diff.root_path, old.data.path);
    /// ```
    pub fn from_elements(from_element: &USLMElement, to_element: &USLMElement) -> TreeDiff {
        assert!(from_element.data.path == to_element.data.path);
        let root_path = from_element.data.path.clone();
        // 1. Diff the root element's fields
        let changes = diff_elements(from_element, to_element);

        // 2. Build HashMaps of children by path
        let children_a: HashMap<String, &USLMElement> = from_element
            .children
            .iter()
            .map(|child| (child.data.path.clone(), child))
            .collect();
        let children_b: HashMap<String, &USLMElement> = to_element
            .children
            .iter()
            .map(|child| (child.data.path.clone(), child))
            .collect();

        // 3. Find added, removed, matched
        let mut added = vec![];
        let mut removed = vec![];
        let mut child_diffs = vec![];
        // Iterate once through A - handle matched and removed
        for (path, child_a) in &children_a {
            match children_b.get(path) {
                Some(child_b) => {
                    // Matched - recurse
                    let child_diff = TreeDiff::from_elements(child_a, child_b);
                    if !child_diff.child_diffs.is_empty() || !child_diff.changes.is_empty() {
                        child_diffs.push(child_diff);
                    }
                }
                None => {
                    // Removed
                    removed.push(child_a.data.clone()); //ElementSnapshot::from(child_a));
                }
            }
        }

        // Iterate through B for added only
        for (path, child_b) in &children_b {
            if !children_a.contains_key(path) {
                added.push(child_b.data.clone()); //ElementSnapshot::from(child_b));
            }
        }

        TreeDiff {
            changes,
            root_path,
            from_element: from_element.data.clone(),
            to_element: to_element.data.clone(),
            added,
            removed,
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
        if path == self.root_path.as_str() {
            return Some(self);
        }
        let remaining_path = path.strip_prefix(self.root_path.as_str())?;
        let next_step: Vec<&str> = remaining_path.split("/").collect();
        assert!(next_step.len() > 1);

        let child_id = next_step[1];
        let child_vec: Vec<&TreeDiff> = self
            .child_diffs
            .iter()
            .filter(|c| c.root_path.ends_with(child_id))
            .collect();
        if child_vec.is_empty() {
            None
        } else {
            assert!(child_vec.len() == 1);
            child_vec[0].find(path)
        }
    }

    /// Calculate the similarity of diffs in the TreeDiff with the amendment data from a bill
    ///
    /// Returns a hashmap with the key being the root_path in the tree diff and the value
    /// being the similarity data
    pub fn calculate_amendment_similarities(
        &self,
        data: &Bill,
    ) -> HashMap<String, AmendmentSimilarity> {
        let mut result = HashMap::new();
        self.calculate_similarities_recursive(&mut result, data);
        result
    }

    fn calculate_similarities_recursive(
        &self,
        result: &mut HashMap<String, AmendmentSimilarity>,
        data: &Bill,
    ) {
        // Check if this TreeDiff has any changes
        if !self.changes.is_empty() {
            // Find the best matching amendment
            for (amendment_id, amendment) in &data.amendments {
                if amendment.changes.is_empty() {
                    continue;
                }

                let similarity = self.calculate_match_with_amendment(amendment_id, amendment);

                if similarity.score > 0.0 {
                    // Insert or update if this is a better match
                    let entry = result
                        .entry(self.root_path.clone())
                        .or_insert(similarity.clone());

                    if similarity.score > entry.score {
                        *entry = similarity;
                    }
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

            // Deduplicate: keep only the longest match per tree_diff_path
            let mut best_by_path: HashMap<&str, &MentionMatch> = HashMap::new();
            for m in &all_matches {
                let dominated = best_by_path
                    .get(m.tree_diff_path.as_str())
                    .is_some_and(|existing| existing.matched_text.len() >= m.matched_text.len());
                if !dominated {
                    best_by_path.insert(&m.tree_diff_path, m);
                }
            }
            let matches: Vec<MentionMatch> = best_by_path.into_values().cloned().collect();

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
pub fn diff_elements(element_a: &USLMElement, element_b: &USLMElement) -> Vec<FieldChangeEvent> {
    assert!(element_a.data.path == element_b.data.path);
    assert!(element_a.data.element_type == element_b.data.element_type);
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
        if !field_changes.changes.is_empty() {
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
    element_a: &USLMElement,
    element_b: &USLMElement,
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

    let diff = TextDiff::from_words(a.as_str(), b.as_str());
    let changes: Vec<TextChange> = diff
        .iter_all_changes()
        // Remove non-changes
        .filter(|c| c.tag() != ChangeTag::Equal)
        // Case to our own diff stucture
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
        old_value: a,
        new_value: b,
        changes,
    }
}
