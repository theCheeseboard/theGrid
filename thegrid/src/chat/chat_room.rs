use cntp_i18n::tr;
use contemporary::components::button::button;
use contemporary::components::grandstand::grandstand;
use contemporary::components::icon::icon;
use contemporary::components::layer::layer;
use contemporary::components::text_field::TextField;
use gpui::http_client::anyhow;
use gpui::{App, IntoElement, ParentElement, RenderOnce, Styled, Window, div, px};
use gpui_tokio::Tokio;
use matrix_sdk::ruma::OwnedRoomId;
use matrix_sdk::ruma::events::room::message::RoomMessageEventContent;
use thegrid::session::session_manager::SessionManager;

#[derive(IntoElement)]
pub struct ChatRoom {
    room_id: OwnedRoomId,
}

pub fn chat_room(room_id: OwnedRoomId) -> ChatRoom {
    ChatRoom { room_id }
}

impl RenderOnce for ChatRoom {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let message_field = window.use_state(cx, |window, cx| {
            let text_field = TextField::new(cx, "message-field", "".into(), "".into());
            text_field.update(cx, |text_field, cx| {
                text_field.borderless(true);
            });
            text_field
        });
        let message_field = message_field.read(cx);

        let session_manager = cx.global::<SessionManager>();
        let Some(client) = session_manager.client() else {
            return div();
        };

        let client = client.read(cx);

        let Some(room) = client.get_room(&self.room_id) else {
            return div().flex().flex_col().size_full().child(
                grandstand("main-area-grandstand")
                    .text(tr!("UNKNOWN_ROOM", "Unknown Room"))
                    .pt(px(36.)),
            );
        };

        let message_field_clone = message_field.clone();
        let room_clone = room.clone();

        div()
            .flex()
            .flex_col()
            .size_full()
            .child(
                grandstand("main-area-grandstand")
                    .text(room.name().unwrap_or_default())
                    .pt(px(36.)),
            )
            .child(div().flex_grow())
            .child(
                layer()
                    .p(px(2.))
                    .gap(px(2.))
                    .flex()
                    .child(message_field.clone().into_any_element())
                    .child(
                        button("send_button")
                            .child(icon("mail-send".into()))
                            .on_click(move |_, _, cx| {
                                let message = message_field_clone.read(cx).current_text(cx);
                                let content =
                                    RoomMessageEventContent::text_plain(message.to_string());
                                let room_clone = room_clone.clone();

                                cx.spawn(async move |cx| {
                                    Tokio::spawn_result(cx, async move {
                                        room_clone.send(content).await.map_err(|e| anyhow!(e))
                                    })
                                    .unwrap()
                                    .await;
                                })
                                .detach();
                            }),
                    ),
            )
    }
}
