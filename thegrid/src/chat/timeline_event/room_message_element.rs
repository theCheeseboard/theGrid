use crate::chat::timeline_event::room_message_event::CachedRoomMember;
use crate::mxc_image::{SizePolicy, mxc_image};
use contemporary::styling::theme::Theme;
use gpui::{
    App, InteractiveElement, IntoElement, ParentElement, RenderOnce, Styled, Window, div, px,
    relative,
};

#[derive(IntoElement)]
pub struct RoomMessageElement<T>
where
    T: IntoElement + 'static,
{
    pub author: Option<CachedRoomMember>,
    pub content: T,
}

impl<T: gpui::IntoElement> RenderOnce for RoomMessageElement<T> {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.global::<Theme>();
        let david = div()
            .id("room-message")
            .flex()
            .m(px(2.))
            .max_w(relative(100.));

        match self.author {
            None => david
                .flex()
                .gap(px(4.))
                .child(div().w(px(40.)).mx(px(2.)))
                .child(div().child(self.content)),
            Some(author) => david.child(
                div()
                    .id("container")
                    .flex()
                    .gap(px(4.))
                    .child(
                        mxc_image(author.avatar())
                            .size(px(40.))
                            .m(px(2.))
                            .size_policy(SizePolicy::Fit)
                            .rounded(theme.border_radius),
                    )
                    .child(
                        div()
                            .id("content")
                            .flex()
                            .flex_col()
                            .child(div().child(author.display_name()).child(self.content)),
                    ),
            ),
        }
    }
}
