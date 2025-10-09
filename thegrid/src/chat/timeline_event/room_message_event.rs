use crate::mxc_image::{SizePolicy, mxc_image};
use contemporary::styling::theme::Theme;
use gpui::{App, IntoElement, ParentElement, RenderOnce, Styled, Window, div, px, relative, rgba};
use matrix_sdk::ruma::events::room::message::{MessageType, RoomMessageEventContent};
use matrix_sdk::ruma::events::{
    AnyTimelineEvent, MessageLikeEventContent, OriginalMessageLikeEvent,
};

#[derive(IntoElement)]
pub struct RoomMessageEvent<T>
where
    T: RoomMessageEventRenderable + 'static,
{
    event: OriginalMessageLikeEvent<T>,
    timeline_event: AnyTimelineEvent,
}

pub fn room_message_event<T>(
    event: OriginalMessageLikeEvent<T>,
    timeline_event: AnyTimelineEvent,
) -> RoomMessageEvent<T>
where
    T: RoomMessageEventRenderable,
{
    RoomMessageEvent {
        event,
        timeline_event,
    }
}

impl<T> RenderOnce for RoomMessageEvent<T>
where
    T: RoomMessageEventRenderable + 'static,
{
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.global::<Theme>();

        let author = self.timeline_event.sender();

        div()
            .flex()
            .m(px(2.))
            .child(div().w(px(200.)).child(author.to_string()))
            .child(
                div()
                    .p(px(2.))
                    .bg(rgba(0x00C8FF10))
                    .rounded(theme.border_radius)
                    .max_w(relative(0.8))
                    .child(self.event.content.speech_box_content()),
            )
    }
}

trait RoomMessageEventRenderable: MessageLikeEventContent {
    fn speech_box_content(&self) -> impl IntoElement;
}

impl RoomMessageEventRenderable for RoomMessageEventContent {
    fn speech_box_content(&self) -> impl IntoElement {
        div().child(match &self.msgtype {
            MessageType::Emote(emote) => div().child(emote.body.clone()).into_any_element(),
            MessageType::Image(image) => mxc_image(image.source.clone())
                .min_w(px(100.))
                .min_h(px(30.))
                .size_policy(SizePolicy::Constrain(500., 500.))
                .into_any_element(),
            MessageType::Text(text) => text.body.clone().into_any_element(),
            MessageType::VerificationRequest(verification_request) => {
                "Key Verification Request".into_any_element()
            }
            _ => "Unknown Message".into_any_element(),
        })
    }
}
