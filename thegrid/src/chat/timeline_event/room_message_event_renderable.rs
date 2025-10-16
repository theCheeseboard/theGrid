use crate::mxc_image::{SizePolicy, mxc_image};
use contemporary::styling::theme::Theme;
use gpui::{IntoElement, ParentElement, Styled, div, px, relative, rgba};
use matrix_sdk::ruma::events::room::message::{MessageType, Relation, RoomMessageEventContent};
use matrix_sdk::ruma::events::{
    AnyMessageLikeEvent, AnyMessageLikeEventContent, MessageLikeEventContent,
};

pub trait RoomMessageEventRenderable: MessageLikeEventContent {
    fn message_line(&self, theme: &Theme) -> impl IntoElement;
    fn should_render(&self) -> bool;
}

impl RoomMessageEventRenderable for RoomMessageEventContent {
    fn message_line(&self, theme: &Theme) -> impl IntoElement {
        div().child(msgtype_to_message_line(&self.msgtype, theme))
    }

    fn should_render(&self) -> bool {
        self.relates_to
            .as_ref()
            .map(|relates_to| match relates_to {
                Relation::Reply { .. } => true,
                Relation::Replacement(_) => false,
                _ => true,
            })
            .unwrap_or(true)
    }
}

impl RoomMessageEventRenderable for AnyMessageLikeEventContent {
    fn message_line(&self, theme: &Theme) -> impl IntoElement {
        match self {
            AnyMessageLikeEventContent::RoomMessage(msg) => div()
                .child(msgtype_to_message_line(&msg.msgtype, theme))
                .into_any_element(),
            _ => div().into_any_element(),
        }
    }

    fn should_render(&self) -> bool {
        true
    }
}

pub fn msgtype_to_message_line(msgtype: &MessageType, theme: &Theme) -> impl IntoElement {
    match msgtype {
        MessageType::Emote(emote) => div().child(emote.body.clone()).into_any_element(),
        MessageType::Image(image) => div()
            .child(
                mxc_image(image.source.clone())
                    .min_w(px(100.))
                    .min_h(px(30.))
                    .size_policy(SizePolicy::Constrain(500., 500.)),
            )
            .into_any_element(),
        MessageType::Text(text) => div()
            .p(px(2.))
            .bg(rgba(0x00C8FF10))
            .rounded(theme.border_radius)
            .max_w(relative(0.8))
            .child(text.body.clone())
            .into_any_element(),
        MessageType::VerificationRequest(verification_request) => {
            "Key Verification Request".into_any_element()
        }
        _ => "Unknown Message".into_any_element(),
    }
}
