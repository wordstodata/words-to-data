//! Detail/attribution pane (right side)

use iced::widget::{button, column, container, row, scrollable, text, text_input};
use iced::{Color, Element, Length, Padding};

use words_to_data::annotation::{AnnotationStatus, ChangeAnnotation};
use words_to_data::congress::{HouseRollCall, Member, Party, VotePosition};
use words_to_data::uslm::AmendingAction;

use crate::message::Message;
use crate::state::{AppState, DetailSection};
use crate::theme::colors;

/// Helper trait for annotation display
pub trait AnnotationDisplay {
    fn operation_display(&self) -> &'static str;
}

impl AnnotationDisplay for ChangeAnnotation {
    fn operation_display(&self) -> &'static str {
        match self.operation {
            AmendingAction::Amend => "amended",
            AmendingAction::Add => "added",
            AmendingAction::Delete => "deleted",
            AmendingAction::Insert => "inserted",
            AmendingAction::Redesignate => "redesignated",
            AmendingAction::Repeal => "repealed",
            AmendingAction::Move => "moved",
            AmendingAction::Strike => "struck",
            AmendingAction::StrikeAndInsert => "struck and inserted",
        }
    }
}

/// Map party to display color
fn party_color(party: &Party) -> Color {
    match party {
        Party::Republican => colors::PARTY_R,
        Party::Democrat => colors::PARTY_D,
        Party::Independent | Party::Other(_) => colors::PARTY_I,
    }
}

/// Format member display: "Name (P-ST-DD)" or "Name (P-ST)"
fn format_member_display(member: &Member) -> String {
    let party_abbr = match &member.party {
        Party::Republican => "R",
        Party::Democrat => "D",
        Party::Independent => "I",
        Party::Other(s) => s.as_str(),
    };
    if let Some(dist) = member.district {
        format!(
            "{} ({}-{}-{:02})",
            member.name, party_abbr, member.state, dist
        )
    } else {
        format!("{} ({}-{})", member.name, party_abbr, member.state)
    }
}

/// Collapsible section header with arrow
fn section_header<'a>(
    title: String,
    is_expanded: bool,
    on_toggle: Message,
) -> Element<'a, Message> {
    let arrow = if is_expanded { "▼" } else { "▶" };
    button(
        row![
            text(arrow).size(10).color(colors::TEXT_SECONDARY),
            text(title).size(12).color(colors::TEXT_PRIMARY),
        ]
        .spacing(6),
    )
    .on_press(on_toggle)
    .padding([4, 0])
    .style(|_, _| button::Style {
        background: None,
        text_color: colors::TEXT_PRIMARY,
        ..Default::default()
    })
    .into()
}

impl AppState {
    /// Right panel: detail pane (360px)
    /// Shows blame detail when selected, otherwise hint
    pub fn view_changes_pane(&self) -> Element<Message> {
        let content = if let Some(ref path) = self.blame_detail_path {
            self.render_blame_detail(path)
        } else {
            // Hint when nothing selected
            column![
                text("Attribution").size(16).color(colors::TEXT_PRIMARY),
                text("Click a bill label in the reading view to see annotation details")
                    .size(12)
                    .color(colors::TEXT_SECONDARY),
            ]
            .spacing(8)
            .into()
        };

        container(scrollable(content).height(Length::Fill))
            .width(Length::Fixed(360.0))
            .height(Length::Fill)
            .padding(12)
            .style(|_| container::Style {
                background: Some(colors::PAPER_DARK.into()),
                ..Default::default()
            })
            .into()
    }

    /// Render blame detail content for right pane
    pub fn render_blame_detail(&self, path: &str) -> Element<Message> {
        let Some(ref dataset) = self.dataset else {
            return text("No dataset").size(12).into();
        };

        let annotations = dataset.annotations_for_path(path).unwrap_or_default();
        let Some(ann) = annotations.first() else {
            return text("No annotation for this path")
                .size(12)
                .color(colors::TEXT_SECONDARY)
                .into();
        };

        let mut content = column![].spacing(12);

        // Header
        let header = text(format!(
            "{} — {}",
            ann.source_bill.bill_id,
            ann.operation_display()
        ))
        .size(16)
        .color(colors::TEXT_PRIMARY);
        content = content.push(header);

        // Status
        let status_text = match ann.metadata.status {
            AnnotationStatus::Pending => "Pending review",
            AnnotationStatus::Verified => "Verified",
            AnnotationStatus::Disputed => "Disputed",
            AnnotationStatus::Rejected => "Rejected",
        };
        let status_color = match ann.metadata.status {
            AnnotationStatus::Verified => colors::INSERT_FG,
            AnnotationStatus::Rejected => colors::DELETE_FG,
            AnnotationStatus::Disputed => colors::BADGE_CHANGED,
            AnnotationStatus::Pending => colors::TEXT_SECONDARY,
        };
        content = content.push(text(status_text).size(11).color(status_color));

        // Causative text
        if !ann.source_bill.causative_text.is_empty() {
            let label = text("Causative Text")
                .size(11)
                .color(colors::TEXT_SECONDARY);
            let body = container(
                text(ann.source_bill.causative_text.to_string())
                    .size(12)
                    .color(colors::TEXT_PRIMARY),
            )
            .padding(8)
            .style(|_| container::Style {
                background: Some(colors::PAPER.into()),
                border: iced::Border {
                    radius: 4.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            });
            content = content.push(column![label, body].spacing(4));
        }

        // Confidence
        if let Some(conf) = ann.metadata.confidence {
            content = content.push(
                text(format!("Confidence: {:.0}%", conf * 100.0))
                    .size(11)
                    .color(colors::TEXT_SECONDARY),
            );
        }

        // Annotator
        content = content.push(
            text(format!("Annotator: {}", ann.metadata.annotator))
                .size(11)
                .color(colors::TEXT_SECONDARY),
        );

        // Reasoning
        if let Some(ref reasoning) = ann.metadata.reasoning {
            let label = text("Reasoning").size(11).color(colors::TEXT_SECONDARY);
            let body = container(
                text(reasoning.to_string())
                    .size(12)
                    .color(colors::TEXT_PRIMARY),
            )
            .padding(8)
            .style(|_| container::Style {
                background: Some(colors::PAPER.into()),
                border: iced::Border {
                    radius: 4.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            });
            content = content.push(column![label, body].spacing(4));
        }

        // Notes
        if let Some(ref notes) = ann.metadata.notes {
            let label = text("Notes").size(11).color(colors::TEXT_SECONDARY);
            let body = container(text(notes.to_string()).size(12).color(colors::TEXT_PRIMARY))
                .padding(8)
                .style(|_| container::Style {
                    background: Some(colors::PAPER.into()),
                    border: iced::Border {
                        radius: 4.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                });
            content = content.push(column![label, body].spacing(4));
        }

        // Divider before voting info
        content = content.push(
            container(text(""))
                .width(Length::Fill)
                .height(Length::Fixed(1.0))
                .style(|_| container::Style {
                    background: Some(colors::PAPER_BORDER.into()),
                    ..Default::default()
                }),
        );

        let bill_id = &ann.source_bill.bill_id;

        // Sponsor section
        if let Some(sponsor_info) = dataset.get_sponsor_info(bill_id).ok().flatten() {
            content = content.push(self.render_sponsor_section(dataset, &sponsor_info));
            content = content.push(self.render_cosponsors_section(dataset, &sponsor_info));
        }

        // Roll call votes section
        content = content.push(self.render_votes_section(dataset, bill_id));

        content.into()
    }

    /// Render sponsor info
    fn render_sponsor_section<'a>(
        &'a self,
        dataset: &crate::state::Dataset,
        sponsor_info: &words_to_data::congress::SponsorInfo,
    ) -> Element<'a, Message> {
        let mut col = column![text("Sponsor").size(11).color(colors::TEXT_SECONDARY)].spacing(4);

        if let Some(member) = dataset.get_member(&sponsor_info.sponsor).ok().flatten() {
            col = col.push(
                text(format_member_display(&member))
                    .size(12)
                    .color(party_color(&member.party)),
            );
        } else {
            col = col.push(
                text(sponsor_info.sponsor.to_string())
                    .size(12)
                    .color(colors::TEXT_SECONDARY),
            );
        }

        col.into()
    }

    /// Render collapsible cosponsors list
    fn render_cosponsors_section<'a>(
        &'a self,
        dataset: &crate::state::Dataset,
        sponsor_info: &words_to_data::congress::SponsorInfo,
    ) -> Element<'a, Message> {
        let count = sponsor_info.cosponsors.len();
        if count == 0 {
            return text("No cosponsors")
                .size(11)
                .color(colors::TEXT_SECONDARY)
                .into();
        }

        let is_expanded = self.detail_expanded.contains(&DetailSection::Cosponsors);
        let header = section_header(
            format!("Cosponsors ({})", count),
            is_expanded,
            Message::ToggleDetailSection(DetailSection::Cosponsors),
        );

        let mut col = column![header].spacing(4);

        if is_expanded {
            let mut list = column![].spacing(2).padding(Padding::ZERO.left(16));
            for cosponsor in &sponsor_info.cosponsors {
                if let Some(member) = dataset.get_member(&cosponsor.bioguide_id).ok().flatten() {
                    list = list.push(
                        text(format_member_display(&member))
                            .size(11)
                            .color(party_color(&member.party)),
                    );
                } else {
                    list = list.push(
                        text(cosponsor.bioguide_id.to_string())
                            .size(11)
                            .color(colors::TEXT_SECONDARY),
                    );
                }
            }
            col = col.push(list);
        }

        col.into()
    }

    /// Render roll call votes section
    fn render_votes_section<'a>(
        &'a self,
        dataset: &crate::state::Dataset,
        bill_id: &str,
    ) -> Element<'a, Message> {
        let mut col = column![
            text("Roll Call Votes")
                .size(11)
                .color(colors::TEXT_SECONDARY)
        ]
        .spacing(8);

        let Some(bill_votes) = dataset.get_bill_votes(bill_id).ok().flatten() else {
            return col
                .push(
                    text("No recorded votes")
                        .size(11)
                        .color(colors::TEXT_SECONDARY),
                )
                .into();
        };

        if bill_votes.roll_calls.is_empty() {
            return col
                .push(
                    text("No recorded votes")
                        .size(11)
                        .color(colors::TEXT_SECONDARY),
                )
                .into();
        }

        for (idx, roll_call) in bill_votes.roll_calls.iter().enumerate() {
            col = col.push(self.render_roll_call(dataset, roll_call, idx));
        }

        col.into()
    }

    /// Render single roll call vote
    fn render_roll_call<'a>(
        &'a self,
        dataset: &crate::state::Dataset,
        roll_call: &HouseRollCall,
        idx: usize,
    ) -> Element<'a, Message> {
        let vote_box = container(
            column![
                text(roll_call.question.to_string())
                    .size(12)
                    .color(colors::TEXT_PRIMARY),
                text(format!("{} — {}", roll_call.date, roll_call.result))
                    .size(11)
                    .color(colors::TEXT_SECONDARY),
                row![
                    text(format!("Yea: {}", roll_call.yea_count))
                        .size(11)
                        .color(colors::INSERT_FG),
                    text(format!("Nay: {}", roll_call.nay_count))
                        .size(11)
                        .color(colors::DELETE_FG),
                    text(format!("NV: {}", roll_call.not_voting_count))
                        .size(11)
                        .color(colors::TEXT_SECONDARY),
                    text(format!("Present: {}", roll_call.present_count))
                        .size(11)
                        .color(colors::TEXT_SECONDARY),
                ]
                .spacing(12),
                self.render_party_vote_bar(dataset, roll_call),
                self.render_member_votes_toggle(dataset, roll_call, idx),
            ]
            .spacing(4),
        )
        .padding(8)
        .style(|_| container::Style {
            background: Some(colors::PAPER.into()),
            border: iced::Border {
                radius: 4.0.into(),
                width: 1.0,
                color: colors::PAPER_BORDER,
            },
            ..Default::default()
        });

        vote_box.into()
    }

    /// Render party breakdown bar showing % yes by party
    fn render_party_vote_bar<'a>(
        &'a self,
        dataset: &crate::state::Dataset,
        roll_call: &HouseRollCall,
    ) -> Element<'a, Message> {
        // Count votes by party
        let mut r_yea = 0u32;
        let mut r_total = 0u32;
        let mut d_yea = 0u32;
        let mut d_total = 0u32;
        let mut i_yea = 0u32;
        let mut i_total = 0u32;

        for mv in &roll_call.member_votes {
            let party = dataset
                .get_member(&mv.bioguide_id)
                .ok()
                .flatten()
                .map(|m| m.party)
                .unwrap_or(Party::Other("?".into()));

            let is_yea = mv.position == VotePosition::Yea;

            match party {
                Party::Republican => {
                    r_total += 1;
                    if is_yea {
                        r_yea += 1;
                    }
                }
                Party::Democrat => {
                    d_total += 1;
                    if is_yea {
                        d_yea += 1;
                    }
                }
                Party::Independent | Party::Other(_) => {
                    i_total += 1;
                    if is_yea {
                        i_yea += 1;
                    }
                }
            }
        }

        let r_pct = if r_total > 0 {
            (r_yea as f32 / r_total as f32) * 100.0
        } else {
            0.0
        };
        let d_pct = if d_total > 0 {
            (d_yea as f32 / d_total as f32) * 100.0
        } else {
            0.0
        };
        let i_pct = if i_total > 0 {
            (i_yea as f32 / i_total as f32) * 100.0
        } else {
            0.0
        };

        let bar_height = 12.0;
        let bar_width = 80.0;

        let make_bar = |pct: f32, color: Color| -> Element<'a, Message> {
            let fill_width = (pct / 100.0) * bar_width;
            container(
                container(text(""))
                    .width(Length::Fixed(fill_width))
                    .height(Length::Fixed(bar_height))
                    .style(move |_| container::Style {
                        background: Some(color.into()),
                        ..Default::default()
                    }),
            )
            .width(Length::Fixed(bar_width))
            .height(Length::Fixed(bar_height))
            .style(|_| container::Style {
                background: Some(colors::PAPER_BORDER.into()),
                border: iced::Border {
                    radius: 2.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            })
            .into()
        };

        let mut bars = row![].spacing(8);

        if r_total > 0 {
            bars = bars.push(
                row![
                    text(format!("R {:.0}%", r_pct))
                        .size(10)
                        .color(colors::PARTY_R),
                    make_bar(r_pct, colors::PARTY_R),
                ]
                .spacing(4)
                .align_y(iced::Alignment::Center),
            );
        }

        if d_total > 0 {
            bars = bars.push(
                row![
                    text(format!("D {:.0}%", d_pct))
                        .size(10)
                        .color(colors::PARTY_D),
                    make_bar(d_pct, colors::PARTY_D),
                ]
                .spacing(4)
                .align_y(iced::Alignment::Center),
            );
        }

        if i_total > 0 {
            bars = bars.push(
                row![
                    text(format!("I {:.0}%", i_pct))
                        .size(10)
                        .color(colors::PARTY_I),
                    make_bar(i_pct, colors::PARTY_I),
                ]
                .spacing(4)
                .align_y(iced::Alignment::Center),
            );
        }

        bars.into()
    }

    /// Render collapsible member votes
    fn render_member_votes_toggle<'a>(
        &'a self,
        dataset: &crate::state::Dataset,
        roll_call: &HouseRollCall,
        idx: usize,
    ) -> Element<'a, Message> {
        let count = roll_call.member_votes.len();
        let is_expanded = self
            .detail_expanded
            .contains(&DetailSection::MemberVotes(idx));

        let header = section_header(
            format!("Member Votes ({})", count),
            is_expanded,
            Message::ToggleDetailSection(DetailSection::MemberVotes(idx)),
        );

        let mut col = column![header].spacing(4);

        if is_expanded {
            // Filter input
            let filter_query = self
                .member_vote_filters
                .get(&idx)
                .map(|s| s.as_str())
                .unwrap_or("");

            let filter_input = text_input("Filter by name...", filter_query)
                .on_input(move |s| Message::MemberVoteFilterChanged(idx, s))
                .size(11)
                .padding(4)
                .width(Length::Fill);

            col = col.push(filter_input);

            let query_lower = filter_query.to_lowercase();

            // Filter helper - check if member name matches query
            let matches_filter = |bioguide: &str| -> bool {
                if query_lower.is_empty() {
                    return true;
                }
                if let Some(member) = dataset.get_member(bioguide).ok().flatten() {
                    member.name.to_lowercase().contains(&query_lower)
                        || member.last_name.to_lowercase().contains(&query_lower)
                        || member.first_name.to_lowercase().contains(&query_lower)
                } else {
                    bioguide.to_lowercase().contains(&query_lower)
                }
            };

            // Group by position, applying filter
            let mut yeas: Vec<&str> = Vec::new();
            let mut nays: Vec<&str> = Vec::new();
            let mut other: Vec<(&str, VotePosition)> = Vec::new();

            for mv in &roll_call.member_votes {
                if !matches_filter(&mv.bioguide_id) {
                    continue;
                }
                match mv.position {
                    VotePosition::Yea => yeas.push(&mv.bioguide_id),
                    VotePosition::Nay => nays.push(&mv.bioguide_id),
                    _ => other.push((&mv.bioguide_id, mv.position)),
                }
            }

            let filtered_count = yeas.len() + nays.len() + other.len();
            if !query_lower.is_empty() && filtered_count < count {
                col = col.push(
                    text(format!("Showing {} of {}", filtered_count, count))
                        .size(10)
                        .color(colors::TEXT_SECONDARY),
                );
            }

            let mut groups = column![].spacing(8).padding(Padding::ZERO.left(8));

            // Yeas
            if !yeas.is_empty() {
                let mut yea_col = column![
                    text(format!("Yea ({})", yeas.len()))
                        .size(11)
                        .color(colors::INSERT_FG)
                ]
                .spacing(2);
                for bioguide in yeas.iter().take(50) {
                    yea_col =
                        yea_col.push(self.format_member_vote(dataset, bioguide, &Party::Democrat));
                }
                if yeas.len() > 50 {
                    yea_col = yea_col.push(
                        text(format!("... and {} more", yeas.len() - 50))
                            .size(10)
                            .color(colors::TEXT_SECONDARY),
                    );
                }
                groups = groups.push(yea_col);
            }

            // Nays
            if !nays.is_empty() {
                let mut nay_col = column![
                    text(format!("Nay ({})", nays.len()))
                        .size(11)
                        .color(colors::DELETE_FG)
                ]
                .spacing(2);
                for bioguide in nays.iter().take(50) {
                    nay_col = nay_col.push(self.format_member_vote(
                        dataset,
                        bioguide,
                        &Party::Republican,
                    ));
                }
                if nays.len() > 50 {
                    nay_col = nay_col.push(
                        text(format!("... and {} more", nays.len() - 50))
                            .size(10)
                            .color(colors::TEXT_SECONDARY),
                    );
                }
                groups = groups.push(nay_col);
            }

            // Other (NV, Present)
            if !other.is_empty() {
                let mut other_col = column![
                    text(format!("Other ({})", other.len()))
                        .size(11)
                        .color(colors::TEXT_SECONDARY)
                ]
                .spacing(2);
                for (bioguide, pos) in other.iter().take(20) {
                    let pos_str = match pos {
                        VotePosition::NotVoting => "NV",
                        VotePosition::Present => "P",
                        _ => "?",
                    };
                    if let Some(member) = dataset.get_member(bioguide).ok().flatten() {
                        other_col = other_col.push(
                            text(format!("{} [{}]", format_member_display(&member), pos_str))
                                .size(10)
                                .color(colors::TEXT_SECONDARY),
                        );
                    } else {
                        other_col = other_col.push(
                            text(format!("{} [{}]", bioguide, pos_str))
                                .size(10)
                                .color(colors::TEXT_SECONDARY),
                        );
                    }
                }
                if other.len() > 20 {
                    other_col = other_col.push(
                        text(format!("... and {} more", other.len() - 20))
                            .size(10)
                            .color(colors::TEXT_SECONDARY),
                    );
                }
                groups = groups.push(other_col);
            }

            if filtered_count == 0 && !query_lower.is_empty() {
                groups = groups.push(
                    text("No matching members")
                        .size(10)
                        .color(colors::TEXT_SECONDARY),
                );
            }

            col = col.push(groups);
        }

        col.into()
    }

    /// Format a single member vote line
    fn format_member_vote<'a>(
        &'a self,
        dataset: &crate::state::Dataset,
        bioguide: &str,
        _fallback_party: &Party,
    ) -> Element<'a, Message> {
        if let Some(member) = dataset.get_member(bioguide).ok().flatten() {
            text(format_member_display(&member))
                .size(10)
                .color(party_color(&member.party))
                .into()
        } else {
            text(bioguide.to_string())
                .size(10)
                .color(colors::TEXT_SECONDARY)
                .into()
        }
    }
}
