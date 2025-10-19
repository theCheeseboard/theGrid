use crate::chat::timeline_event::author_flyout::{AuthorFlyoutUserActionListener, author_flyout};
use crate::chat::timeline_event::room_message_event::CachedRoomMember;
use crate::mxc_image::{SizePolicy, mxc_image};
use contemporary::components::anchorer::WithAnchorer;
use contemporary::components::flyout::flyout;
use contemporary::styling::theme::Theme;
use gpui::{
    App, InteractiveElement, IntoElement, ParentElement, RenderOnce, StatefulInteractiveElement,
    Styled, Window, div, px, relative,
};
use matrix_sdk::Room;

#[derive(IntoElement)]
pub struct RoomMessageElement<T>
where
    T: IntoElement + 'static,
{
    pub author: Option<CachedRoomMember>,
    pub room: Room,
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
            Some(author) => {
                let author_clone = author.clone();
                david.child(
                    div()
                        .id("container")
                        .flex()
                        .gap(px(4.))
                        .child(
                            div().flex().flex_col().child(
                                div()
                                    .id("author-image")
                                    .cursor_pointer()
                                    .child(
                                        mxc_image(author.avatar())
                                            .size(px(40.))
                                            .m(px(2.))
                                            .size_policy(SizePolicy::Fit)
                                            .rounded(theme.border_radius),
                                    )
                                    .with_anchorer(move |david, bounds| {
                                        david.child(author_flyout(
                                            bounds,
                                            author_flyout_open,
                                            author,
                                            room,
                                            move |_, _, cx| {
                                                author_flyout_open_entity_2.write(cx, false);
                                            },
                                            self.on_user_action,
                                        ))
                                    })
                                    .on_click(move |_, _, cx| {
                                        author_flyout_open_entity.write(cx, true);
                                    }),
                            ),
                        )
                        .child(
                            div().id("content").flex().flex_col().child(
                                div().child(author_clone.display_name()).child(self.content),
                            ),
                        ),
                )
            }
        }
    }
}
