use crate::chat::chat_room::open_room::{OpenRoom, OpenRoomFocus, OpenRoomFocusReason};
use crate::chat::chat_room::timeline_view::author_flyout::AuthorFlyoutUserActionListener;
use crate::chat::chat_room::timeline_view::timeline_message_item::msgtype_to_message_line;
use crate::chat::displayed_room::DisplayedRoom;
use cntp_i18n::tr;
use contemporary::styling::theme::{ThemeStorage, VariableColor};
use gpui::{
    App, Entity, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder,
};
use matrix_sdk::ruma::{OwnedEventId, OwnedUserId};
use matrix_sdk_ui::timeline::{
    InReplyToDetails, MsgLikeKind, Profile, TimelineDetails, TimelineEventFocusThreadMode,
    TimelineFocus, TimelineItemContent,
};
use std::rc::Rc;

#[derive(IntoElement)]
pub struct ReplyFragment {
    content: Option<TimelineItemContent>,
    sender_profile: Option<TimelineDetails<Profile>>,
    sender: Option<OwnedUserId>,
    room: Entity<OpenRoom>,
    displayed_room: Entity<DisplayedRoom>,
    on_user_action: Rc<Box<AuthorFlyoutUserActionListener>>,
    event_id: Option<OwnedEventId>,
}

pub fn reply_fragment(
    content: TimelineItemContent,
    sender_profile: TimelineDetails<Profile>,
    sender: OwnedUserId,
    room: Entity<OpenRoom>,
    displayed_room: Entity<DisplayedRoom>,
    on_user_action: Rc<Box<AuthorFlyoutUserActionListener>>,
) -> ReplyFragment {
    ReplyFragment {
        content: Some(content),
        sender_profile: Some(sender_profile),
        sender: Some(sender),
        room,
        displayed_room,
        on_user_action,
        event_id: None,
    }
}

pub fn reply_fragment_in_reply_to(
    details: InReplyToDetails,
    room: Entity<OpenRoom>,
    displayed_room: Entity<DisplayedRoom>,
    on_user_action: Rc<Box<AuthorFlyoutUserActionListener>>,
) -> ReplyFragment {
    let (content, sender_profile, sender) = if let TimelineDetails::Ready(reply) = details.event {
        (
            Some(reply.content),
            Some(reply.sender_profile),
            Some(reply.sender),
        )
    } else {
        (None, None, None)
    };

    ReplyFragment {
        content,
        sender_profile,
        sender,
        room,
        displayed_room,
        on_user_action,
        event_id: Some(details.event_id),
    }
}

impl RenderOnce for ReplyFragment {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        div()
            .id("reply_fragment")
            .flex()
            .text_color(theme.foreground.disabled())
            .text_size(theme.system_font_size * 0.8)
            // TODO: RTL?
            .child("⬐ ")
            .child({
                match self.content {
                    Some(TimelineItemContent::MsgLike(msg_like)) => match msg_like.kind {
                        MsgLikeKind::Message(message) => Some(
                            div()
                                .flex()
                                .child(msgtype_to_message_line(
                                    message.msgtype(),
                                    self.sender.unwrap(),
                                    self.sender_profile.unwrap(),
                                    true,
                                    self.room.clone(),
                                    self.displayed_room,
                                    self.on_user_action,
                                    window,
                                    cx,
                                ))
                                .into_any_element(),
                        ),
                        _ => None,
                    },
                    _ => None,
                }
                .unwrap_or_else(|| {
                    tr!("REPLY_UNAVAILABLE", "Reply message could not be loaded").into_any_element()
                })
            })
            .when_some(self.event_id, |david, event_id| {
                david.cursor_pointer().on_click({
                    let open_room = self.room.clone();
                    let event_id = event_id.clone();
                    move |_, _, cx| {
                        open_room.update(cx, {
                            let event_id = event_id.clone();
                            move |open_room, cx| {
                                open_room.focus_timeline(
                                    OpenRoomFocus {
                                        timeline_focus: TimelineFocus::Event {
                                            target: event_id,
                                            num_context_events: 0,
                                            thread_mode: TimelineEventFocusThreadMode::Automatic {
                                                hide_threaded_events: false,
                                            },
                                        },
                                        reason: OpenRoomFocusReason::Reply,
                                    },
                                    cx,
                                );
                            }
                        })
                    }
                })
            })
    }
}
