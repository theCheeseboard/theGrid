use cntp_i18n::tr;
use contemporary::styling::theme::Theme;
use gpui::{App, IntoElement, ParentElement, RenderOnce, Styled, Window, div, px};
use matrix_sdk::ruma::OwnedRoomId;
use thegrid_common::mxc_image::{SizePolicy, mxc_image};
use thegrid_common::session::session_manager::SessionManager;

#[derive(IntoElement)]
pub struct RoomHead {
    room: OwnedRoomId,
}

pub fn room_head(room: OwnedRoomId) -> RoomHead {
    RoomHead { room }
}

impl RenderOnce for RoomHead {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.global::<Theme>();
        let session_manager = cx.global::<SessionManager>();
        let rooms = session_manager.rooms().read(cx);

        let room = rooms.room(&self.room).unwrap().read(cx);

        div()
            .flex()
            .gap(px(4.))
            .child(div().w(px(40.)).mx(px(2.)))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(div().h(px(200.)))
                    .child(
                        mxc_image(room.inner.avatar_url())
                            .size(px(128.))
                            .size_policy(SizePolicy::Fit),
                    )
                    .child(
                        div().text_size(theme.heading_font_size).child(
                            room.inner
                                .cached_display_name()
                                .map(|name| name.to_string())
                                .or_else(|| room.inner.name())
                                .unwrap_or_default(),
                        ),
                    )
                    .child(tr!(
                        "ROOM_HEAD_TEXT",
                        "Welcome to the beginning of the room."
                    )),
            )
    }
}
