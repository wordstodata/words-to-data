//! Reading pane - main content view with diff highlighting and blame

use iced::widget::{
    Column, button, column, container, rich_text, row, scrollable, span, text, tooltip,
};
use iced::{Element, Length, Padding};

use words_to_data::annotation::AnnotationStatus;
use words_to_data::diff::TextChangeType;
use words_to_data::uslm::{TextContentField, USLMElement};

use crate::message::Message;
use crate::state::AppState;
use crate::theme::colors;
use crate::views::detail::AnnotationDisplay;

impl AppState {
    /// Center panel: reading view (flexible)
    pub fn view_reading_pane(&self) -> Element<'_, Message> {
        let content: Element<'_, Message> = if let Some(ref dataset) = self.dataset {
            if let Some(ref path) = self.selected_path {
                if let Some(version) = dataset.storage().versions.get(self.selected_version_index) {
                    if let Some(element) = version.element.find(path) {
                        // Flatten tree into single column
                        let mut blocks: Vec<Element<'_, Message>> = Vec::new();
                        self.collect_element_blocks(element, 0, &mut blocks);
                        Column::with_children(blocks).spacing(4).into()
                    } else {
                        text("Element not found").into()
                    }
                } else {
                    text("No version").into()
                }
            } else {
                text("Select an element from the tree").into()
            }
        } else {
            text("Load a dataset to begin").into()
        };

        container(scrollable(content).height(Length::Fill))
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(16)
            .style(|_| container::Style {
                background: Some(colors::PAPER.into()),
                ..Default::default()
            })
            .into()
    }

    /// Collect all element blocks into flat vec with indentation
    fn collect_element_blocks<'a>(
        &'a self,
        element: &'a USLMElement,
        depth: usize,
        out: &mut Vec<Element<'a, Message>>,
    ) {
        let indent = depth as f32 * 16.0;

        // Render this element's fields directly into out
        self.render_single_element(element, out, indent);

        // Recurse children
        for child in &element.children {
            self.collect_element_blocks(child, depth + 1, out);
        }

        // Continuation after children (same indent as parent)
        if let Some(ref continuation) = element.data.continuation {
            let cont_block = text(continuation.to_string())
                .size(14.0)
                .color(colors::TEXT_PRIMARY);
            out.push(
                container(cont_block)
                    .padding(Padding::default().left(indent))
                    .into(),
            );
        }
    }

    /// Render single element fields with optional blame gutter
    /// Returns elements to push directly to flat output vec
    fn render_single_element<'a>(
        &'a self,
        element: &'a USLMElement,
        out: &mut Vec<Element<'a, Message>>,
        indent: f32,
    ) {
        let path = &element.data.path;
        let diff = self.get_diff_for_path(path);
        let has_changes = diff.is_some_and(|d| !d.changes.is_empty());

        // Helper to render a field with word-level diff highlighting
        let render_field =
            |field: TextContentField, field_text: &'a str, size: f32| -> Element<'a, Message> {
                if let Some(diff) = diff {
                    if let Some(change) = diff.changes.iter().find(|c| c.field_name == field) {
                        let spans: Vec<iced::widget::text::Span<'a, (), iced::Font>> = change
                            .changes
                            .iter()
                            .map(|tc| {
                                let s = span::<(), iced::Font>(&tc.value);
                                match tc.tag {
                                    TextChangeType::Insert => {
                                        s.color(colors::INSERT_FG).background(colors::INSERT_BG)
                                    }
                                    TextChangeType::Delete => {
                                        s.color(colors::DELETE_FG).background(colors::DELETE_BG)
                                    }
                                    TextChangeType::Equal => s.color(colors::TEXT_PRIMARY),
                                }
                            })
                            .collect();
                        return rich_text(spans).size(size).into();
                    }
                }
                text(field_text)
                    .size(size)
                    .color(colors::TEXT_PRIMARY)
                    .into()
            };

        // Collect fields
        let mut fields: Vec<Element<'a, Message>> = Vec::new();
        if let Some(ref heading) = element.data.heading {
            fields.push(render_field(TextContentField::Heading, heading, 20.0));
        }
        if let Some(ref chapeau) = element.data.chapeau {
            fields.push(render_field(TextContentField::Chapeau, chapeau, 14.0));
        }
        if let Some(ref body) = element.data.content {
            fields.push(render_field(TextContentField::Content, body, 14.0));
        }
        if let Some(ref proviso) = element.data.proviso {
            fields.push(render_field(TextContentField::Proviso, proviso, 13.0));
        }

        if fields.is_empty() {
            return;
        }

        // Build blame widget if needed
        let blame_widget: Option<Element<'a, Message>> = if has_changes && self.show_blame {
            self.get_blame_for_path(path).map(|(bill_id, color)| {
                // Party color stripe
                let stripe =
                    container(text(""))
                        .width(4.0)
                        .height(Length::Shrink)
                        .style(move |_| container::Style {
                            background: Some(color.into()),
                            ..Default::default()
                        });

                // Bill label
                let label = text(bill_id.clone()).size(10).color(colors::TEXT_SECONDARY);
                let blame_col = column![stripe, label].spacing(2);

                // Clickable blame button
                let path_clone = path.to_string();
                let blame_btn = button(blame_col)
                    .padding(2)
                    .style(move |_, status| {
                        let bg = match status {
                            button::Status::Hovered => Some(colors::HOVER.into()),
                            _ => None,
                        };
                        button::Style {
                            background: bg,
                            ..Default::default()
                        }
                    })
                    .on_press(Message::ShowBlameDetail(path_clone));

                // Tooltip
                let tooltip_content = self.build_blame_tooltip(path);
                tooltip(blame_btn, tooltip_content, tooltip::Position::Bottom)
                    .gap(4)
                    .style(|_| container::Style {
                        background: Some(colors::PAPER.into()),
                        border: iced::Border {
                            radius: 4.0.into(),
                            width: 1.0,
                            color: colors::PAPER_BORDER,
                        },
                        shadow: iced::Shadow {
                            color: iced::Color::from_rgba(0.0, 0.0, 0.0, 0.15),
                            offset: iced::Vector::new(0.0, 2.0),
                            blur_radius: 8.0,
                        },
                        ..Default::default()
                    })
                    .into()
            })
        } else {
            None
        };

        // Push fields directly - no nested column
        for (i, field) in fields.into_iter().enumerate() {
            let elem: Element<'a, Message> = if i == 0 {
                if let Some(ref _blame) = blame_widget {
                    // First field gets blame widget in row
                    // Re-build blame widget (can't move out of Option in loop)
                    if let Some((bill_id, color)) = self.get_blame_for_path(path) {
                        let stripe = container(text("")).width(4.0).height(Length::Shrink).style(
                            move |_| container::Style {
                                background: Some(color.into()),
                                ..Default::default()
                            },
                        );
                        let label = text(bill_id.clone()).size(10).color(colors::TEXT_SECONDARY);
                        let blame_col = column![stripe, label].spacing(2);
                        let path_clone = path.to_string();
                        let blame_btn = button(blame_col)
                            .padding(2)
                            .style(move |_, status| {
                                let bg = match status {
                                    button::Status::Hovered => Some(colors::HOVER.into()),
                                    _ => None,
                                };
                                button::Style {
                                    background: bg,
                                    ..Default::default()
                                }
                            })
                            .on_press(Message::ShowBlameDetail(path_clone));
                        let tooltip_content = self.build_blame_tooltip(path);
                        let blame_with_tooltip =
                            tooltip(blame_btn, tooltip_content, tooltip::Position::Bottom)
                                .gap(4)
                                .style(|_| container::Style {
                                    background: Some(colors::PAPER.into()),
                                    border: iced::Border {
                                        radius: 4.0.into(),
                                        width: 1.0,
                                        color: colors::PAPER_BORDER,
                                    },
                                    shadow: iced::Shadow {
                                        color: iced::Color::from_rgba(0.0, 0.0, 0.0, 0.15),
                                        offset: iced::Vector::new(0.0, 2.0),
                                        blur_radius: 8.0,
                                    },
                                    ..Default::default()
                                });
                        row![blame_with_tooltip, field].spacing(8).into()
                    } else {
                        field
                    }
                } else {
                    field
                }
            } else {
                field
            };

            out.push(
                container(elem)
                    .padding(Padding::default().left(indent))
                    .into(),
            );
        }
    }

    /// Build tooltip content for blame hover
    pub fn build_blame_tooltip<'a>(&'a self, path: &str) -> Element<'a, Message> {
        let Some(ref dataset) = self.dataset else {
            return text("No data").size(11).into();
        };

        let annotations = dataset.annotations_for_path(path).unwrap_or_default();
        let Some(ann) = annotations.first() else {
            return text("No annotation").size(11).into();
        };

        let mut content = column![].spacing(4).padding(8);

        // Bill and operation
        let header = text(format!(
            "{} - {}",
            ann.source_bill.bill_id,
            ann.operation_display()
        ))
        .size(12)
        .color(colors::TEXT_PRIMARY);
        content = content.push(header);

        // Status
        let status_text = match ann.metadata.status {
            AnnotationStatus::Pending => "Pending",
            AnnotationStatus::Verified => "Verified",
            AnnotationStatus::Disputed => "Disputed",
            AnnotationStatus::Rejected => "Rejected",
        };
        let status = text(status_text).size(10).color(colors::TEXT_SECONDARY);
        content = content.push(status);

        // Truncated causative text preview
        if !ann.source_bill.causative_text.is_empty() {
            let preview: String = ann.source_bill.causative_text.chars().take(100).collect();
            let preview = if ann.source_bill.causative_text.len() > 100 {
                format!("{}...", preview)
            } else {
                preview
            };
            let causative = text(preview).size(10).color(colors::TEXT_SECONDARY);
            content = content.push(causative);
        }

        // Click hint
        let hint = text("Click for details")
            .size(9)
            .color(colors::TEXT_SECONDARY);
        content = content.push(hint);

        container(content).max_width(300.0).into()
    }
}
