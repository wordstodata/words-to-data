//! Version navigation bar

use iced::widget::{button, container, row, text};
use iced::{Element, Length};

use crate::message::Message;
use crate::state::AppState;
use crate::theme::colors;

impl AppState {
    /// Bottom bar: version arrows and info
    pub fn view_timeline(&self) -> Element<Message> {
        let Some(ref dataset) = self.dataset else {
            return container(text("")).height(Length::Fixed(36.0)).into();
        };

        let version_count = dataset.storage().versions.len();
        if version_count == 0 {
            return container(text("No versions"))
                .height(Length::Fixed(36.0))
                .into();
        }

        let current_date = dataset
            .storage()
            .versions
            .get(self.selected_version_index)
            .map(|v| v.date.as_str())
            .unwrap_or("--");

        let version_label = dataset
            .storage()
            .versions
            .get(self.selected_version_index)
            .and_then(|v| v.label.as_deref())
            .unwrap_or("");

        let info = if version_label.is_empty() {
            text(format!(
                "{} ({}/{})",
                current_date,
                self.selected_version_index + 1,
                version_count
            ))
        } else {
            text(format!(
                "{} - {} ({}/{})",
                current_date,
                version_label,
                self.selected_version_index + 1,
                version_count
            ))
        }
        .size(12)
        .color(colors::TEXT_SECONDARY);

        let prev_btn = button(text("◀").size(12))
            .padding([4, 8])
            .on_press(Message::PrevVersion);

        let next_btn = button(text("▶").size(12))
            .padding([4, 8])
            .on_press(Message::NextVersion);

        let content = row![prev_btn, info, next_btn]
            .spacing(12)
            .align_y(iced::Alignment::Center);

        container(content)
            .width(Length::Fill)
            .height(Length::Fixed(36.0))
            .padding(4)
            .center_x(Length::Fill)
            .style(|_| container::Style {
                background: Some(colors::PAPER_DARK.into()),
                ..Default::default()
            })
            .into()
    }
}
