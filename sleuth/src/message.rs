//! Message types for SLEUTH UI events

use crate::state::DetailSection;

/// UI messages for the SLEUTH application
#[derive(Debug, Clone)]
pub enum Message {
    // Tree navigator
    /// Toggle expand/collapse for a tree node
    TreeToggle(String),
    /// Select a path in the tree
    SelectPath(String),

    // Version navigation
    /// Change to a specific version by index
    VersionChange(usize),
    /// Go to next version
    NextVersion,
    /// Go to previous version
    PrevVersion,

    // View modes
    /// Toggle between Reading and Structural view
    ToggleViewMode,
    /// Toggle blame gutter visibility
    ToggleBlame,

    // Timeline
    /// Change timeline display style
    SetTimelineStyle(TimelineStyle),

    // Changes panel tabs
    /// Switch changes panel tab
    SetChangesTab(ChangesTab),

    // Overlays
    /// Toggle search overlay (⌘K)
    ToggleSearch,
    /// Toggle dataset loader (⌘O)
    ToggleLoader,
    /// Show blame detail modal for a path
    ShowBlameDetail(String),
    /// Toggle collapsible section in detail pane
    ToggleDetailSection(DetailSection),
    /// Update member vote filter for a roll call
    MemberVoteFilterChanged(usize, String),
    /// Close all overlays
    CloseOverlays,

    // Dataset operations
    /// Open file picker dialog
    OpenFilePicker,
    /// File selected from picker
    FileSelected(std::path::PathBuf),
    /// File picker cancelled
    FilePickerCancelled,
    /// Load a dataset from path
    LoadDataset(String),
    /// Dataset loaded successfully
    DatasetLoaded(Box<crate::state::Dataset>),
    /// Dataset load failed
    DatasetError(String),
    /// Update loader path input
    LoaderPathChanged(String),

    // Search
    /// Update search query
    SearchQueryChanged(String),
    /// Execute search
    SearchSubmit,

    // Keyboard
    /// Keyboard event
    KeyPressed(iced::keyboard::Key, iced::keyboard::Modifiers),

    /// No-op (used for ignored events)
    NoOp,
}

/// View mode for the reading pane
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ViewMode {
    /// Formatted reading view with prose
    #[default]
    Reading,
    /// Raw structural view showing element boundaries
    Structural,
}

/// Timeline display styles
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TimelineStyle {
    /// Scrubber slider
    #[default]
    Scrubber,
    /// Vertical list of versions
    List,
    /// Density visualization
    Density,
}

/// Tabs in the changes panel
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ChangesTab {
    /// Changes in current version
    #[default]
    ThisVersion,
    /// All paths changed
    AllPaths,
    /// Lifetime of selected element
    Lifetime,
}
