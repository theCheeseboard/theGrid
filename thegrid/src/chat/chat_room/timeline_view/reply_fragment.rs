use crate::chat::chat_room::open_room::OpenRoom;
use crate::chat::chat_room::timeline_view::author_flyout::AuthorFlyoutUserActionListener;
use crate::chat::chat_room::timeline_view::timeline_message_item::msgtype_to_message_line;
use crate::chat::displayed_room::DisplayedRoom;
use cntp_i18n::tr;
use contemporary::styling::theme::{ThemeStorage, VariableColor};
use gpui::{App, Entity, IntoElement, ParentElement, RenderOnce, Styled, Window, div};
use matrix_sdk::ruma::OwnedUserId;
use matrix_sdk_ui::timeline::{
    InReplyToDetails, MsgLikeKind, Profile, TimelineDetails, TimelineItemContent,
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
    }
}

impl RenderOnce for ReplyFragment {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        div()
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
                                    self.room,
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
    }
}
