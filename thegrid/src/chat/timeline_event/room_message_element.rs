use crate::chat::chat_room::open_room::OpenRoom;
use crate::chat::timeline_event::author_flyout::{AuthorFlyoutUserActionListener, author_flyout};
use crate::chat::timeline_event::room_message_event::CachedRoomMember;
use crate::mxc_image::{SizePolicy, mxc_image};
use contemporary::components::anchorer::WithAnchorer;
use contemporary::components::flyout::flyout;
use contemporary::styling::theme::Theme;
use gpui::{
    App, Entity, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    StatefulInteractiveElement, Styled, Window, div, px, relative, rgb,
};
use matrix_sdk::Room;

#[derive(IntoElement)]
pub struct RoomMessageElement<T>
where
    T: IntoElement + 'static,
{
    pub author: Option<CachedRoomMember>,
    pub room: Entity<OpenRoom>,
    pub content: T,
    pub on_user_action: Box<AuthorFlyoutUserActionListener>,
}

impl<T: gpui::IntoElement> RenderOnce for RoomMessageElement<T> {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let author_flyout_open_entity = window.use_state(cx, |_, _| false);
        let author_flyout_open = *author_flyout_open_entity.read(cx);
        let author_flyout_open_entity_2 = author_flyout_open_entity.clone();
        let room = self.room;

        let theme = cx.global::<Theme>();
        let david = div().id("room-message").flex().m(px(2.)).max_w_full();

        match self.author {
            None => david.child(
                div()
                    .flex()
                    .w_full()
                    .max_w_full()
                    .gap(px(8.))
                    .child(div().min_w(px(40.)).mx(px(2.)))
                    .child(div().w_full().max_w_full().child(self.content)),
            ),
            Some(author) => {
                let author_clone = author.clone();
                david.child(
                    div()
                        .id("container")
                        .flex()
                        .flex_grow()
                        .gap(px(8.))
                        .child(
                            div().flex().flex_col().min_w(px(40.)).m(px(2.)).child(
                                div().id("author-image").cursor_pointer().child(
                                    mxc_image(author.avatar())
                                        .size(px(40.))
                                        .size_policy(SizePolicy::Fit)
                                        .rounded(theme.border_radius),
                                ),
                            ),
                        )
                        .child(
                            div().id("content").flex_grow().flex().flex_col().child(
                                div().child(author_clone.display_name()).child(self.content),
                            ),
                        ),
                )
            }
        }
    }
}
