use crate::chat::chat_room::open_room::OpenRoom;
use crate::chat::chat_room::timeline_view::author_flyout::{
    author_flyout, AuthorFlyoutUserActionEvent, AuthorFlyoutUserActionListener,
};
use crate::chat::chat_room::timeline_view::membership_change_item::membership_change_item;
use crate::chat::chat_room::timeline_view::message_error_item::message_error_item;
use crate::chat::chat_room::timeline_view::profile_change_item::profile_change_item;
use crate::chat::chat_room::timeline_view::room_head::room_head;
use crate::chat::chat_room::timeline_view::rtc_notification_item::rtc_notification_item;
use crate::chat::chat_room::timeline_view::state_event_item::state_event_item;
use crate::chat::chat_room::timeline_view::timeline_message_item::timeline_message_item;
use crate::chat::displayed_room::DisplayedRoom;
use chrono::{DateTime, Local};
use cntp_i18n::tr;
use contemporary::components::anchorer::WithAnchorer;
use contemporary::components::context_menu::{ContextMenuExt, ContextMenuItem};
use contemporary::styling::theme::{variable_transparent, Theme, VariableColor};
use gpui::prelude::FluentBuilder;
use gpui::{
    div, px, App, AsyncApp, ElementId, Entity, InteractiveElement, IntoElement,
    ParentElement, RenderOnce, StatefulInteractiveElement, Styled, WeakEntity, Window,
};
use matrix_sdk::room::RoomMember;
use matrix_sdk_ui::timeline::{
    EventTimelineItem, TimelineDetails, TimelineItem as MatrixUiTimelineItem, TimelineItemContent,
    TimelineItemKind, VirtualTimelineItem,
};
use std::rc::Rc;
use std::sync::Arc;
use thegrid_common::mxc_image::{mxc_image, SizePolicy};
use thegrid_common::tokio_helper::TokioHelper;

#[derive(IntoElement)]
pub struct TimelineItem {
    timeline_item: Arc<MatrixUiTimelineItem>,
    previous_timeline_item: Option<Arc<MatrixUiTimelineItem>>,
    open_room: Entity<OpenRoom>,
    displayed_room: Entity<DisplayedRoom>,
    on_user_action: Rc<Box<AuthorFlyoutUserActionListener>>,
}

pub fn timeline_item(
    item: Arc<MatrixUiTimelineItem>,
    previous_timeline_item: Option<Arc<MatrixUiTimelineItem>>,
    open_room: Entity<OpenRoom>,
    displayed_room: Entity<DisplayedRoom>,
    on_user_action: impl Fn(&AuthorFlyoutUserActionEvent, &mut Window, &mut App) + 'static,
) -> TimelineItem {
    TimelineItem {
        timeline_item: item,
        previous_timeline_item,
        open_room,
        displayed_room,
        on_user_action: Rc::new(Box::new(on_user_action)),
    }
}

enum TimelineRowType {
    MessageWithAuthor,
    MessageWithoutAuthor,
    State,
}

impl TimelineItem {
    pub fn render_event_timeline_item(
        &self,
        event: &EventTimelineItem,
        window: &mut Window,
        cx: &mut App,
    ) -> impl IntoElement {
        let author_flyout_open_entity = window.use_state(cx, |_, _| false);
        let author_flyout_open_entity_2 = author_flyout_open_entity.clone();

        let open_room = &self.open_room;

        let author = event.sender();
        let previous_event_author =
            self.previous_timeline_item
                .as_ref()
                .and_then(|item| match item.kind() {
                    TimelineItemKind::Event(e) => {
                        if is_state_event(e.content()) {
                            None
                        } else {
                            Some(e.sender().to_owned())
                        }
                    }
                    TimelineItemKind::Virtual(_) => None,
                });

        let row_type = if is_state_event(event.content()) {
            TimelineRowType::State
        } else if let Some(previous_event_author) = previous_event_author
            && *author == previous_event_author
        {
            TimelineRowType::MessageWithoutAuthor
        } else {
            TimelineRowType::MessageWithAuthor
        };

        let theme = cx.global::<Theme>().clone();

        let mut context_menu = vec![];

        let event_content = div()
            .flex()
            .flex_col()
            .child(match event.content() {
                TimelineItemContent::MsgLike(msg) => {
                    let sender = event.sender().to_string();
                    context_menu.push(
                        ContextMenuItem::separator()
                            .label(tr!(
                                "MESSAGE_CONTEXT_MENU_TITLE",
                                "For message from {{user}}",
                                user:quote = sender
                            ))
                            .build(),
                    );
                    context_menu.push(
                        ContextMenuItem::menu_item()
                            .label(tr!("MESSAGE_REPLY", "Reply"))
                            .icon("mail-reply-sender")
                            .when(!event.can_be_replied_to(), |david| david.disabled())
                            .on_triggered({
                                let open_room = open_room.clone();
                                let event = event.clone();
                                move |_, window, cx| {
                                    open_room.update(cx, |open_room, cx| {
                                        open_room.set_pending_reply(Some(event.clone()), cx);
                                    });
                                }
                            })
                            .build(),
                    );
                    timeline_message_item(msg.clone()).into_any_element()
                }
                TimelineItemContent::MembershipChange(membership_change) => {
                    membership_change_item(membership_change.clone()).into_any_element()
                }
                TimelineItemContent::ProfileChange(profile_change) => {
                    profile_change_item(profile_change.clone()).into_any_element()
                }
                TimelineItemContent::RtcNotification => {
                    rtc_notification_item(event.clone()).into_any_element()
                }
                TimelineItemContent::OtherState(other_state) => state_event_item(
                    other_state.clone(),
                    event.sender_profile().clone(),
                    event.sender().to_owned(),
                )
                .into_any_element(),
                TimelineItemContent::FailedToParseMessageLike { .. }
                | TimelineItemContent::FailedToParseState { .. } => {
                    message_error_item("exception", tr!("MESSAGE_CORRUPT", "Corrupt Message"), cx)
                        .into_any_element()
                }
                _ => message_error_item("dialog-warning", tr!("MESSAGE_UNSUPPORTED"), cx)
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

        let hovered = window.use_keyed_state(
            ElementId::NamedChild(
                Box::new(ElementId::Name("hovered".into())),
                event
                    .event_id()
                    .map(|event_id| event_id.to_string())
                    .unwrap_or_default()
                    .into(),
            ),
            cx,
            |_, _| false,
        );

        let mut background = variable_transparent();
        if event.is_highlighted() {
            background = theme.warning_accent_color.blend(background);
        }
        if *hovered.read(cx) {
            let mut layer = theme.layer_background;
            layer.a /= 2.;
            background = layer.blend(background)
        }

        match row_type {
            TimelineRowType::MessageWithAuthor => {
                let sender = event.sender().to_owned();
                let sender_profile = event.sender_profile();
                let room = self.open_room.clone();
                let cached_member = window.use_state::<Option<RoomMember>>(cx, |_, cx| {
                    let room = room.read(cx);
                    let room = room.room.clone().unwrap();

                    cx.spawn(
                        async move |weak_this: WeakEntity<Option<RoomMember>>,
                                    cx: &mut AsyncApp| {
                            let Ok(member) = cx
                                .spawn_tokio(async move { room.get_member(&sender).await })
                                .await
                            else {
                                return;
                            };

                            let _ = weak_this.update(cx, |this, cx| {
                                *this = member;
                                cx.notify();
                            });
                        },
                    )
                    .detach();

                    None
                });
                let author_flyout_open = *author_flyout_open_entity.read(cx);
                let on_user_action = self.on_user_action.clone();
                let displayed_room = self.displayed_room.clone();

                div()
                    .id("container")
                    .when(event.is_local_echo(), |david| david.opacity(0.7))
                    .bg(background)
                    .on_hover({
                        let hovered = hovered.clone();
                        move |is_hover, _, cx| {
                            hovered.write(cx, *is_hover);
                        }
                    })
                    .flex()
                    .flex_grow()
                    .w_full()
                    .overflow_hidden()
                    .gap(px(8.))
                    .pr(px(8.))
                    .child(
                        div().flex().flex_col().min_w(px(40.)).m(px(2.)).child(
                            div()
                                .id("author-image")
                                .cursor_pointer()
                                .child(
                                    mxc_image(match sender_profile {
                                        TimelineDetails::Ready(profile) => {
                                            profile.avatar_url.clone()
                                        }
                                        _ => None,
                                    })
                                    .fallback_image(event.sender())
                                    .size(px(40.))
                                    .size_policy(SizePolicy::Fit)
                                    .rounded(theme.border_radius),
                                )
                                .with_anchorer(move |david, bounds, _, _| {
                                    david.child(author_flyout(
                                        bounds,
                                        author_flyout_open,
                                        cached_member,
                                        room,
                                        displayed_room,
                                        move |_, _, cx| {
                                            author_flyout_open_entity_2.write(cx, false);
                                        },
                                        move |event, window, cx| {
                                            on_user_action.clone()(event, window, cx)
                                        },
                                    ))
                                })
                                .on_click(move |_, _, cx| {
                                    author_flyout_open_entity.write(cx, true);
                                }),
                        ),
                    )
                    .child(
                        div()
                            .id("content")
                            .flex_grow()
                            .flex()
                            .flex_col()
                            .overflow_hidden()
                            .child(
                                div()
                                    .child(
                                        match sender_profile {
                                            TimelineDetails::Ready(profile) => profile
                                                .display_name
                                                .clone()
                                                .or(Some(event.sender().to_string())),
                                            _ => None,
                                        }
                                        .unwrap_or_default(),
                                    )
                                    .child(event_content),
                            ),
                    )
                    .when(!context_menu.is_empty(), |david| {
                        david.with_context_menu(context_menu)
                    })
                    .into_any_element()
            }
            TimelineRowType::MessageWithoutAuthor => div()
                .id("container")
                .when(event.is_local_echo(), |david| david.opacity(0.7))
                .bg(background)
                .on_hover({
                    let hovered = hovered.clone();
                    move |is_hover, _, cx| {
                        hovered.write(cx, *is_hover);
                    }
                })
                .flex()
                .overflow_hidden()
                .w_full()
                .gap(px(8.))
                .pr(px(8.))
                .child(div().min_w(px(40.)).mx(px(2.)))
                .child(
                    div()
                        .w_full()
                        .max_w_full()
                        .overflow_hidden()
                        .child(event_content),
                )
                .when(!context_menu.is_empty(), |david| {
                    david.with_context_menu(context_menu)
                })
                .into_any_element(),
            TimelineRowType::State => div()
                .w_full()
                .overflow_hidden()
                .pr(px(8.))
                .child(event_content)
                .into_any_element(),
        }
    }
}

impl RenderOnce for TimelineItem {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        match self.timeline_item.kind() {
            TimelineItemKind::Event(event) => self
                .render_event_timeline_item(event, window, cx)
                .into_any_element(),
            TimelineItemKind::Virtual(VirtualTimelineItem::DateDivider(date)) => {
                let theme = cx.global::<Theme>();

                let date = DateTime::from_timestamp_secs(date.as_secs().into())
                    .unwrap()
                    .with_timezone(&Local);

                div()
                    .flex()
                    .w_full()
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
                room_head(self.open_room.read(cx).room_id.clone()).into_any_element()
            }
            _ => div().into_any_element(),
        }
    }
}

fn is_state_event(content: &TimelineItemContent) -> bool {
    match content {
        TimelineItemContent::MsgLike(_)
        | TimelineItemContent::FailedToParseMessageLike { .. }
        | TimelineItemContent::FailedToParseState { .. }
        | TimelineItemContent::CallInvite
        | TimelineItemContent::RtcNotification => false,
        TimelineItemContent::MembershipChange(_)
        | TimelineItemContent::ProfileChange(_)
        | TimelineItemContent::OtherState(_) => true,
    }
}
