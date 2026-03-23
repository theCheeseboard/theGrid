use crate::chat::chat_room::timeline_view::timeline_message_item::msgtype_to_message_line;
use cntp_i18n::tr;
use contemporary::styling::theme::{ThemeStorage, VariableColor};
use gpui::{div, App, IntoElement, ParentElement, RenderOnce, Styled, Window};
use matrix_sdk_ui::timeline::{
    InReplyToDetails, MsgLikeKind, TimelineDetails, TimelineItemContent,
};

#[derive(IntoElement)]
pub struct ReplyFragment {
    content: Option<TimelineItemContent>,
}

pub fn reply_fragment(content: TimelineItemContent) -> ReplyFragment {
    ReplyFragment {
        content: Some(content),
    }
}

pub fn reply_fragment_in_reply_to(details: InReplyToDetails) -> ReplyFragment {
    ReplyFragment {
        content: if let TimelineDetails::Ready(reply) = details.event {
            Some(reply.content)
        } else {
            None
        },
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
                                .child(msgtype_to_message_line(message.msgtype(), true, window, cx))
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
