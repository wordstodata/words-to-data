//! Message handling and state updates

use iced::Task;
use iced::keyboard::{Key, key::Named};

use words_to_data::dataset::{Dataset, Format};

use crate::message::{Message, ViewMode};
use crate::state::AppState;

impl AppState {
    /// Update state in response to a message
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::TreeToggle(path) => {
                if self.tree_expanded.contains(&path) {
                    self.tree_expanded.remove(&path);
                } else {
                    self.tree_expanded.insert(path);
                }
                Task::none()
            }
            Message::SelectPath(path) => {
                self.selected_path = Some(path);
                Task::none()
            }
            Message::VersionChange(idx) => {
                if let Some(ref dataset) = self.dataset {
                    if idx < dataset.versions.len() {
                        self.selected_version_index = idx;
                        self.recompute_diff();
                    }
                }
                Task::none()
            }
            Message::NextVersion => {
                if let Some(ref dataset) = self.dataset {
                    if self.selected_version_index + 1 < dataset.versions.len() {
                        self.selected_version_index += 1;
                        self.recompute_diff();
                    }
                }
                Task::none()
            }
            Message::PrevVersion => {
                if self.selected_version_index > 0 {
                    self.selected_version_index -= 1;
                    self.recompute_diff();
                }
                Task::none()
            }
            Message::ToggleViewMode => {
                self.view_mode = match self.view_mode {
                    ViewMode::Reading => ViewMode::Structural,
                    ViewMode::Structural => ViewMode::Reading,
                };
                Task::none()
            }
            Message::ToggleBlame => {
                self.show_blame = !self.show_blame;
                Task::none()
            }
            Message::SetTimelineStyle(style) => {
                self.timeline_style = style;
                Task::none()
            }
            Message::SetChangesTab(_) => Task::none(), // Unused
            Message::ToggleSearch => {
                self.show_search = !self.show_search;
                if self.show_search {
                    self.show_loader = false;
                }
                Task::none()
            }
            Message::ToggleLoader => {
                self.show_loader = !self.show_loader;
                if self.show_loader {
                    self.show_search = false;
                }
                Task::none()
            }
            Message::ShowBlameDetail(path) => {
                self.blame_detail_path = Some(path);
                self.detail_expanded.clear();
                self.member_vote_filters.clear();
                self.show_search = false;
                self.show_loader = false;
                Task::none()
            }
            Message::ToggleDetailSection(section) => {
                if self.detail_expanded.contains(&section) {
                    self.detail_expanded.remove(&section);
                } else {
                    self.detail_expanded.insert(section);
                }
                Task::none()
            }
            Message::MemberVoteFilterChanged(idx, query) => {
                if query.is_empty() {
                    self.member_vote_filters.remove(&idx);
                } else {
                    self.member_vote_filters.insert(idx, query);
                }
                Task::none()
            }
            Message::CloseOverlays => {
                self.show_search = false;
                self.show_loader = false;
                self.blame_detail_path = None;
                Task::none()
            }
            Message::OpenFilePicker => Task::future(async {
                rfd::AsyncFileDialog::new()
                    .add_filter("JSON", &["json"])
                    .set_title("Open Dataset")
                    .pick_file()
                    .await
            })
            .map(|handle| match handle {
                Some(h) => Message::FileSelected(h.path().to_path_buf()),
                None => Message::FilePickerCancelled,
            }),
            Message::FileSelected(path) => {
                self.show_loader = false;
                let path_str = path.to_string_lossy().to_string();
                match Dataset::load(&path_str, Format::Compact) {
                    Ok(dataset) => Task::done(Message::DatasetLoaded(Box::new(dataset))),
                    Err(e) => Task::done(Message::DatasetError(e.to_string())),
                }
            }
            Message::FilePickerCancelled => Task::none(),
            Message::LoadDataset(path) => {
                self.show_loader = false;
                match Dataset::load(&path, Format::Compact) {
                    Ok(dataset) => Task::done(Message::DatasetLoaded(Box::new(dataset))),
                    Err(e) => Task::done(Message::DatasetError(e.to_string())),
                }
            }
            Message::DatasetLoaded(dataset) => {
                // Auto-expand root and first level
                if let Some(version) = dataset.versions.first() {
                    let root_path = version.element.data.path.clone();
                    self.tree_expanded.insert(root_path.to_string());
                }
                self.dataset = Some(*dataset);
                self.selected_version_index = 0;
                self.selected_path = None;
                self.recompute_diff();
                Task::none()
            }
            Message::DatasetError(err) => {
                eprintln!("Dataset error: {}", err);
                Task::none()
            }
            Message::SearchQueryChanged(query) => {
                self.search_query = query;
                Task::none()
            }
            Message::SearchSubmit => {
                // TODO: Implement search
                Task::none()
            }
            Message::LoaderPathChanged(path) => {
                self.loader_path = path;
                Task::none()
            }
            Message::KeyPressed(key, modifiers) => {
                // Handle keyboard shortcuts
                if modifiers.command() {
                    match key.as_ref() {
                        Key::Character("k") => {
                            self.show_search = !self.show_search;
                            self.show_loader = false;
                        }
                        Key::Character("o") => {
                            self.show_loader = !self.show_loader;
                            self.show_search = false;
                        }
                        _ => {}
                    }
                }
                // Escape closes overlays
                if key == Key::Named(Named::Escape) {
                    self.show_search = false;
                    self.show_loader = false;
                    self.blame_detail_path = None;
                }
                Task::none()
            }
            Message::NoOp => Task::none(),
        }
    }
}
