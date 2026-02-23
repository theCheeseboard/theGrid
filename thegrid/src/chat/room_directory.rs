use crate::chat::displayed_room::DisplayedRoom;
use cntp_i18n::tr;
use contemporary::components::constrainer::constrainer;
use contemporary::components::grandstand::grandstand;
use contemporary::styling::theme::Theme;
use gpui::{
    Context, Entity, InteractiveElement, IntoElement, ParentElement, Render, Styled, Window, div,
    px,
};
use matrix_sdk::OwnedServerName;
use matrix_sdk::ruma::OwnedRoomId;

mod directory_view;

pub struct RoomDirectory {
    server_name: OwnedServerName,
}

impl RoomDirectory {
    pub fn new(
        server_name: OwnedServerName,
        displayed_room: Entity<DisplayedRoom>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self { server_name }
    }
}

impl Render for RoomDirectory {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>();
        let server_name_string = self.server_name.to_string();

        div()
            .bg(theme.background)
            .w_full()
            .h_full()
            .flex()
            .flex_col()
            .child(
                grandstand("directory-grandstand")
                    .text(tr!(
                        "ROOM_DIRECTORY_TITLE",
                        "Room Directory of {{server}}",
                        server:quote = server_name_string
                    ))
                    .pt(px(36.)),
            )
            .child(
                constrainer("room-directory-constrainer")
                    .flex()
                    .flex_col()
                    .w_full()
                    .p(px(8.))
                    .gap(px(8.))
                    .child("Directory"),
            )
    }
}
