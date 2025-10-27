use crate::chat::chat_room::timeline_view::membership_change_item::membership_change_item;
use crate::chat::chat_room::timeline_view::room_head::room_head;
use crate::chat::chat_room::timeline_view::timeline_message_item::timeline_message_item;
use crate::chat::timeline_event::author_flyout::author_flyout;
use crate::mxc_image::{SizePolicy, mxc_image};
use chrono::{DateTime, Local};
use cntp_i18n::tr;
use contemporary::components::anchorer::WithAnchorer;
use contemporary::styling::theme::{Theme, VariableColor};
use gpui::prelude::FluentBuilder;
use gpui::{
    App, InteractiveElement, IntoElement, ParentElement, RenderOnce, StatefulInteractiveElement,
    Styled, Window, div, px,
};
use matrix_sdk::ruma::OwnedRoomId;
use matrix_sdk_ui::timeline::{
    EventTimelineItem, Profile, TimelineDetails, TimelineItem as MatrixUiTimelineItem,
    TimelineItemContent, TimelineItemKind, VirtualTimelineItem,
};
use std::sync::Arc;

#[derive(IntoElement)]
pub struct TimelineItem {
    timeline_item: Arc<MatrixUiTimelineItem>,
    previous_timeline_item: Option<Arc<MatrixUiTimelineItem>>,
    room_id: OwnedRoomId,
}

pub fn timeline_item(
    item: Arc<MatrixUiTimelineItem>,
    previous_timeline_item: Option<Arc<MatrixUiTimelineItem>>,
    room_id: OwnedRoomId,
) -> TimelineItem {
    TimelineItem {
        timeline_item: item,
        previous_timeline_item,
        room_id,
    }
}

impl TimelineItem {
    pub fn render_event_timeline_item(
        &self,
        event: &EventTimelineItem,
        cx: &mut App,
    ) -> impl IntoElement {
        let author = event.sender();
        let previous_event_author =
            self.previous_timeline_item
                .as_ref()
                .and_then(|item| match item.kind() {
                    TimelineItemKind::Event(e) => Some(e.sender().to_owned()),
                    TimelineItemKind::Virtual(_) => None,
                });

        let show_author_eligible = match event.content() {
            TimelineItemContent::MsgLike(_) => true,
            TimelineItemContent::MembershipChange(_) => false,
            TimelineItemContent::ProfileChange(_) => false,
            TimelineItemContent::OtherState(_) => false,
            TimelineItemContent::FailedToParseMessageLike { .. } => true,
            TimelineItemContent::FailedToParseState { .. } => true,
            TimelineItemContent::CallInvite => true,
            TimelineItemContent::CallNotify => true,
        };
        let show_author = if let Some(previous_event_author) = previous_event_author
            && *author == previous_event_author
        {
            false
        } else {
            show_author_eligible
        };

        let theme = cx.global::<Theme>();

        let event_content = div()
            .flex()
            .flex_col()
            .child(match event.content() {
                TimelineItemContent::MsgLike(msg) => {
                    timeline_message_item(msg.clone()).into_any_element()
                }
                TimelineItemContent::MembershipChange(membership_change) => {
                    membership_change_item(membership_change.clone()).into_any_element()
                }
                _ => div()
                    .child(tr!("MESSAGE_UNSUPPORTED", "Unsupported Message"))
                    .into_any_element(),
            })
            .when(event.latest_edit_json().is_some(), |david| {
                david.child(
                    div()
                        .flex()
                        .text_color(theme.foreground.disabled())
                        .text_size(theme.system_font_size * 0.8)
                        // TODO: RTL?
                        .child("⬑ ")
                        .child(tr!("EDITED_MESSAGE_INDICATOR", "(edited)")),
                )
            });

        if show_author {
            let sender_profile = event.sender_profile();
            div()
                .id("container")
                .when(event.is_local_echo(), |david| david.opacity(0.7))
                .flex()
                .flex_grow()
                .gap(px(8.))
                .child(
                    div().flex().flex_col().min_w(px(40.)).m(px(2.)).child(
                        div().id("author-image").cursor_pointer().child(
                            mxc_image(match sender_profile {
                                TimelineDetails::Ready(profile) => profile.avatar_url.clone(),
                                _ => None,
                            })
                            .size(px(40.))
                            .size_policy(SizePolicy::Fit)
                            .rounded(theme.border_radius),
                        ), // .with_anchorer(move |david, bounds| {
                           //     david.child(author_flyout(
                           //         bounds,
                           //         author_flyout_open,
                           //         author,
                           //         room,
                           //         move |_, _, cx| {
                           //             author_flyout_open_entity_2.write(cx, false);
                           //         },
                           //         self.on_user_action,
                           //     ))
                           // })
                           // .on_click(move |_, _, cx| {
                           //     author_flyout_open_entity.write(cx, true);
                           // }),
                    ),
                )
                .child(
                    div().id("content").flex_grow().flex().flex_col().child(
                        div()
                            .child(
                                match sender_profile {
                                    TimelineDetails::Ready(profile) => profile.display_name.clone(),
                                    _ => None,
                                }
                                .unwrap_or_default(),
                            )
                            .child(event_content),
                    ),
                )
                .into_any_element()
        } else {
            div()
                .when(event.is_local_echo(), |david| david.opacity(0.7))
                .flex()
                .w_full()
                .max_w_full()
                .gap(px(8.))
                .child(div().min_w(px(40.)).mx(px(2.)))
                .child(div().w_full().max_w_full().child(event_content))
                .into_any_element()
        }
    }
}

impl RenderOnce for TimelineItem {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        match self.timeline_item.kind() {
            TimelineItemKind::Event(event) => self
                .render_event_timeline_item(event, cx)
                .into_any_element(),
            TimelineItemKind::Virtual(VirtualTimelineItem::DateDivider(date)) => {
                let theme = cx.global::<Theme>();

                let date = DateTime::from_timestamp_secs(date.as_secs().into())
                    .unwrap()
                    .with_timezone(&Local);

                div()
                    .flex()
                    .gap(px(8.))
                    .items_center()
                    .child(div().h(px(1.)).bg(theme.border_color).flex_grow())
                    .child(
                        div()
                            .child(tr!(
                                "DATE_DIVIDER",
                                "{{date}}",
                                date:date("YMD", length="medium")=date
                            ))
                            .text_color(theme.border_color),
                    )
                    .child(div().h(px(1.)).bg(theme.border_color).flex_grow())
                    .into_any_element()
            }
            TimelineItemKind::Virtual(VirtualTimelineItem::TimelineStart) => {
                room_head(self.room_id.clone()).into_any_element()
            }
            _ => div().into_any_element(),
        }
    }
}
