//! Application state and cache management

use std::collections::{HashMap, HashSet};

use words_to_data::dataset::Dataset as GenericDataset;
use words_to_data::diff::TreeDiff;
use words_to_data::storage::InMemoryStorage;

/// SLEUTH loads datasets from compact/JSON files, which are always backed by
/// in-memory storage. Alias so the rest of the app can spell it plainly.
pub type Dataset = GenericDataset<InMemoryStorage>;

use crate::message::{TimelineStyle, ViewMode};

/// Collapsible sections in detail pane
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DetailSection {
    Cosponsors,
    MemberVotes(usize), // roll call index
}

/// Main application state
pub struct AppState {
    /// Loaded dataset (None until loaded)
    pub dataset: Option<Dataset>,

    /// Currently selected element path
    pub selected_path: Option<String>,

    /// Current version index (0-based)
    pub selected_version_index: usize,

    /// Cached diff between previous and current version
    pub current_diff: Option<TreeDiff>,

    /// Set of paths with changes in current diff (for fast lookup)
    pub changed_paths: HashSet<String>,

    /// Set of paths that have descendant changes (for tree badges)
    pub paths_with_descendant_changes: HashSet<String>,

    /// Flattened diff cache: path -> shallow TreeDiff (O(1) lookup)
    pub diff_cache: HashMap<String, TreeDiff>,

    /// Set of expanded tree paths
    pub tree_expanded: HashSet<String>,

    /// Current view mode
    pub view_mode: ViewMode,

    /// Show blame gutter
    pub show_blame: bool,

    /// Timeline display style
    pub timeline_style: TimelineStyle,

    /// Search overlay visible
    pub show_search: bool,

    /// Loader overlay visible
    pub show_loader: bool,

    /// Current search query
    pub search_query: String,

    /// Loader path input
    pub loader_path: String,

    /// Path for blame detail modal (None = closed)
    pub blame_detail_path: Option<String>,

    /// Expanded sections in detail pane
    pub detail_expanded: HashSet<DetailSection>,

    /// Filter queries for member vote lists (roll call index -> query)
    pub member_vote_filters: HashMap<usize, String>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            dataset: None,
            selected_path: None,
            selected_version_index: 0,
            current_diff: None,
            changed_paths: HashSet::new(),
            paths_with_descendant_changes: HashSet::new(),
            diff_cache: HashMap::new(),
            tree_expanded: HashSet::new(),
            view_mode: ViewMode::default(),
            show_blame: true,
            timeline_style: TimelineStyle::default(),
            show_search: false,
            show_loader: false,
            search_query: String::new(),
            loader_path: String::new(),
            blame_detail_path: None,
            detail_expanded: HashSet::new(),
            member_vote_filters: HashMap::new(),
        }
    }
}

impl AppState {
    /// Create new app state
    pub fn new() -> Self {
        Self::default()
    }

    /// Recompute diff between previous and current version
    pub fn recompute_diff(&mut self) {
        self.current_diff = None;
        self.changed_paths.clear();
        self.paths_with_descendant_changes.clear();
        self.diff_cache.clear();

        let Some(ref dataset) = self.dataset else {
            return;
        };

        if self.selected_version_index == 0 || dataset.storage().versions.len() < 2 {
            return;
        }

        let from = &dataset.storage().versions[self.selected_version_index - 1];
        let to = &dataset.storage().versions[self.selected_version_index];

        let diff = TreeDiff::from_elements(&from.element, &to.element);
        self.flatten_diff_to_cache(&diff);

        // Pre-compute paths with descendant changes
        for changed_path in &self.changed_paths {
            let mut current = changed_path.as_str();
            while let Some(pos) = current.rfind('/') {
                current = &current[..pos];
                if !current.is_empty() {
                    self.paths_with_descendant_changes
                        .insert(current.to_string());
                }
            }
        }

        self.current_diff = Some(diff);
    }

    /// Flatten TreeDiff into cache HashMap for O(1) lookups
    fn flatten_diff_to_cache(&mut self, diff: &TreeDiff) {
        // Store shallow copy (no children) in cache
        if !diff.changes.is_empty() || !diff.added.is_empty() || !diff.removed.is_empty() {
            self.changed_paths.insert(diff.root_path.clone());
            self.diff_cache
                .insert(diff.root_path.clone(), diff.shallow());
        }
        for child in &diff.child_diffs {
            self.flatten_diff_to_cache(child);
        }
    }

    /// Check if path or any descendant has changes (O(1) lookup)
    pub fn has_descendant_changes(&self, path: &str) -> bool {
        self.paths_with_descendant_changes.contains(path)
    }

    /// Get diff for specific path (O(1) cache lookup)
    pub fn get_diff_for_path(&self, path: &str) -> Option<&TreeDiff> {
        self.diff_cache.get(path)
    }

    /// Get bill attribution info for a path (bill_id, party color)
    pub fn get_blame_for_path(&self, path: &str) -> Option<(String, iced::Color)> {
        use crate::theme::colors;
        use words_to_data::congress::Party;

        let dataset = self.dataset.as_ref()?;
        let annotations = dataset.annotations_for_path(path).ok()?;
        let ann = annotations.first()?;

        let bill_id = &ann.source_bill.bill_id;

        // Format as "hr7024·118" style
        let formatted_id = bill_id.replace("-", "·");

        let color = if let Some(sponsor_info) = dataset.get_sponsor_info(bill_id).ok().flatten() {
            if let Some(member) = dataset.get_member(&sponsor_info.sponsor).ok().flatten() {
                match &member.party {
                    Party::Republican => colors::PARTY_R,
                    Party::Democrat => colors::PARTY_D,
                    Party::Independent | Party::Other(_) => colors::PARTY_I,
                }
            } else {
                colors::TEXT_SECONDARY
            }
        } else {
            colors::TEXT_SECONDARY
        };

        Some((formatted_id, color))
    }
}
