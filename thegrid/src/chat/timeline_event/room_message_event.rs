use contemporary::styling::theme::Theme;
use gpui::{App, IntoElement, ParentElement, RenderOnce, Styled, Window, div, px, relative, rgba};
use matrix_sdk::ruma::events::room::message::RoomMessageEventContent;
use matrix_sdk::ruma::events::{MessageLikeEventContent, OriginalMessageLikeEvent};

#[derive(IntoElement)]
pub struct RoomMessageEvent<T>
where
    T: RoomMessageEventRenderable + 'static,
{
    event: OriginalMessageLikeEvent<T>,
}

pub fn room_message_event<T>(event: OriginalMessageLikeEvent<T>) -> RoomMessageEvent<T>
where
    T: RoomMessageEventRenderable,
{
    RoomMessageEvent { event }
}

impl<T> RenderOnce for RoomMessageEvent<T>
where
    T: RoomMessageEventRenderable + 'static,
{
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.global::<Theme>();
        div().flex().m(px(2.)).child(
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
    fn speech_box_content(self) -> impl IntoElement;
}

impl RoomMessageEventRenderable for RoomMessageEventContent {
    fn speech_box_content(self) -> impl IntoElement {
        div().child(self.body().to_string())
    }
}
