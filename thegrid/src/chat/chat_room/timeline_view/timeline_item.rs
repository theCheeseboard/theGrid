use crate::auth::emoji_flyout::EmojiFlyout;
use crate::chat::chat_input::{ChatInput, End};
use crate::chat::chat_room::chat_bar::ChatBar;
use crate::chat::chat_room::open_room::{OpenRoom, OpenRoomFocus};
use crate::chat::chat_room::timeline_view::author_flyout::{
    AuthorFlyoutUserActionEvent, AuthorFlyoutUserActionListener, author_flyout,
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
use contemporary::components::button::button;
use contemporary::components::context_menu::{ContextMenuExt, ContextMenuItem};
use contemporary::components::flyout::flyout;
use contemporary::components::icon::icon;
use contemporary::components::layer::layer;
use contemporary::components::tooltip::simple_tooltip;
use contemporary::styling::theme::{Theme, VariableColor, variable_transparent};
use gpui::prelude::FluentBuilder;
use gpui::{
    App, AsyncApp, ElementId, Entity, Focusable, InteractiveElement, IntoElement, ParentElement,
    RenderOnce, StatefulInteractiveElement, Styled, WeakEntity, Window, deferred, div, px,
};
use matrix_sdk::room::RoomMember;
use matrix_sdk::room::edit::EditedContent;
use matrix_sdk::ruma::events::MessageLikeEventType;
use matrix_sdk::ruma::events::room::message::{
    MessageType, RoomMessageEventContentWithoutRelation,
};
use matrix_sdk_ui::timeline::{
    EventTimelineItem, MsgLikeKind, TimelineDetails, TimelineFocus,
    TimelineItem as MatrixUiTimelineItem, TimelineItemContent, TimelineItemKind,
    VirtualTimelineItem,
};
use std::rc::Rc;
use std::sync::Arc;
use thegrid_common::mxc_image::{SizePolicy, mxc_image};
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

        let editing = window.use_state(cx, |_, _| false);

        let open_room = &self.open_room;
        let open_room_read = open_room.read(cx);
        let current_user = open_room_read.current_user.clone();
        let focused_event_id = match &open_room_read.current_focus {
            OpenRoomFocus {
                timeline_focus: TimelineFocus::Event { target, .. },
                ..
            } => Some(target.clone()),
            _ => None,
        };

        let emoji_flyout_visible = window.use_state(cx, |_, _| false);
        let emoji_flyout = window.use_state(cx, {
            let emoji_flyout_visible = emoji_flyout_visible.clone();
            let open_room = open_room.clone();
            let event = event.clone();
            move |_, cx| {
                let mut emoji_flyout = EmojiFlyout::new(cx);
                emoji_flyout.set_emoji_selected_listener({
                    let emoji_flyout_visible = emoji_flyout_visible.clone();
                    let open_room = open_room.clone();
                    move |emoji_selected_event, _, cx| {
                        emoji_flyout_visible.write(cx, false);
                        open_room.update(cx, |open_room, cx| {
                            open_room.toggle_reaction_on_event(
                                &event,
                                emoji_selected_event.emoji.clone(),
                                cx,
                            )
                        })
                    }
                });
                emoji_flyout
            }
        });

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
                            .label(tr!("MESSAGE_REACT", "Add Reaction"))
                            .when(
                                current_user.as_ref().is_some_and(|user| {
                                    !user.can_send_message(MessageLikeEventType::Reaction)
                                }),
                                |david| david.disabled(),
                            )
                            .on_triggered({
                                let emoji_flyout_visible = emoji_flyout_visible.clone();
                                move |_, _, cx| emoji_flyout_visible.write(cx, true)
                            })
                            .build(),
                    );
                    context_menu.push(
                        ContextMenuItem::menu_item()
                            .label(tr!("MESSAGE_REPLY", "Reply"))
                            .icon("mail-reply-sender")
                            .when(
                                !event.can_be_replied_to()
                                    || current_user.as_ref().is_some_and(|user| {
                                        !user.can_send_message(MessageLikeEventType::RoomMessage)
                                    }),
                                |david| david.disabled(),
                            )
                            .on_triggered({
                                let open_room = open_room.clone();
                                let event = event.clone();
                                move |_, _, cx| {
                                    open_room.update(cx, |open_room, cx| {
                                        open_room.set_pending_reply(Some(event.clone()), cx);
                                    });
                                }
                            })
                            .build(),
                    );
                    if current_user.as_ref().is_some_and(|user| {
                        (user.can_redact_own() && event.is_own()) || user.can_redact_other()
                    }) && !matches!(msg.kind, MsgLikeKind::Redacted)
                    {
                        context_menu.push(
                            ContextMenuItem::menu_item()
                                .label(tr!("MESSAGE_REDACT", "Remove"))
                                .icon("edit-delete")
                                .on_triggered({
                                    let open_room = open_room.clone();
                                    let event = event.clone();
                                    move |_, _, cx| {
                                        open_room.update(cx, |open_room, cx| {
                                            open_room.redact_event(&event.clone(), cx);
                                        });
                                    }
                                })
                                .build(),
                        );
                    }
                    if event.is_own() && event.is_editable() {
                        context_menu.push(
                            ContextMenuItem::menu_item()
                                .label(tr!("MESSAGE_EDIT", "Edit"))
                                .icon("edit-rename")
                                .on_triggered({
                                    let editing = editing.clone();
                                    move |_, _, cx| editing.write(cx, true)
                                })
                                .build(),
                        );
                    }

                    if *editing.read(cx)
                        && let MsgLikeKind::Message(message) = &msg.kind
                    {
                        let initial_content = message.body();
                        let edit = window.use_state(cx, |_, _| initial_content.to_string());
                        let complete_edit = {
                            let open_room = open_room.clone();
                            let event = event.clone();
                            let edit = edit.clone();
                            let editing = editing.clone();
                            let message = message.clone();
                            move |cx: &mut App| {
                                open_room.update(cx, |open_room, cx| {
                                    open_room.edit_event(
                                        &event.clone(),
                                        EditedContent::RoomMessage(
                                            match message.msgtype() {
                                                MessageType::Notice(_) => {
                                                    RoomMessageEventContentWithoutRelation::
                                                        notice_markdown(edit.read(cx))
                                                }
                                                MessageType::Text(_) => {
                                                    RoomMessageEventContentWithoutRelation::
                                                        text_markdown(edit.read(cx))
                                                }
                                                _ => RoomMessageEventContentWithoutRelation::
                                                        new(message.msgtype().clone()),
                                            }
                                        ),
                                        cx,
                                    );
                                });
                                editing.write(cx, false);
                            }
                        };
                        let editor = window.use_state(cx, |window, cx| {
                            let mut chat_input = ChatInput::new(open_room.downgrade(), cx);
                            chat_input.set_text(initial_content);
                            chat_input.on_enter_press({
                                let complete_edit = complete_edit.clone();
                                move |_, _, cx| complete_edit(cx)
                            });
                            chat_input.on_escape_press({
                                let editing = editing.clone();
                                move |_, _, cx| editing.write(cx, false)
                            });
                            chat_input.on_text_changed(cx.listener({
                                let edit = edit.clone();
                                move |chat_input: &mut ChatInput, _, _, cx| {
                                    edit.write(cx, chat_input.text().to_string());
                                }
                            }));
                            chat_input.end(&End, window, cx);
                            chat_input.focus_handle(cx).focus(window, cx);
                            chat_input
                        });
                        layer()
                            .flex()
                            .flex_col()
                            .p(px(4.))
                            .child(deferred(editor))
                            .child(
                                div()
                                    .flex()
                                    .gap(px(4.))
                                    .child(div().flex_grow())
                                    .child(
                                        button("edit-cancel-button")
                                            .child(icon("dialog-cancel"))
                                            .flat()
                                            .on_click({
                                                let editing = editing.clone();
                                                move |_, _, cx| editing.write(cx, false)
                                            })
                                            .tooltip(simple_tooltip(tr!(
                                                "EDIT_CANCEL",
                                                "Cancel Editing"
                                            ))),
                                    )
                                    .child(
                                        button("edit-send-button")
                                            .child(icon("dialog-ok"))
                                            .on_click({
                                                let complete_edit = complete_edit.clone();
                                                move |_, _, cx| complete_edit(cx)
                                            })
                                            .tooltip(simple_tooltip(tr!(
                                                "EDIT_SUBMIT",
                                                "Submit Edit"
                                            ))),
                                    ),
                            )
                            .into_any_element()
                    } else {
                        timeline_message_item(
                            msg.clone(),
                            event.clone(),
                            self.open_room.clone(),
                            self.displayed_room.clone(),
                            self.on_user_action.clone(),
                        )
                        .into_any_element()
                    }
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
                Arc::new(ElementId::Name("hovered".into())),
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
        if focused_event_id.is_some_and(|focused_event_id| {
            event
                .event_id()
                .is_some_and(|event_id| focused_event_id == event_id)
        }) {
            background = theme.info_accent_color.blend(background);
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
                                    .fixed_square(px(40.))
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
                    .with_anchorer({
                        let emoji_flyout_visible = emoji_flyout_visible.clone();
                        let emoji_flyout = emoji_flyout.clone();
                        move |david, bounds, _, cx| {
                            david.child(
                                flyout(bounds)
                                    .render_as_deferred(true)
                                    .visible(*emoji_flyout_visible.read(cx))
                                    .on_close(move |_, _, cx| emoji_flyout_visible.write(cx, false))
                                    .child(emoji_flyout),
                            )
                        }
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
                .with_anchorer({
                    let emoji_flyout_visible = emoji_flyout_visible.clone();
                    let emoji_flyout = emoji_flyout.clone();
                    move |david, bounds, _, cx| {
                        david.child(
                            flyout(bounds)
                                .render_as_deferred(true)
                                .visible(*emoji_flyout_visible.read(cx))
                                .on_close(move |_, _, cx| emoji_flyout_visible.write(cx, false))
                                .child(emoji_flyout),
                        )
                    }
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
