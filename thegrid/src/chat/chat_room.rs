use crate::chat::displayed_room::DisplayedRoom;
use crate::chat::main_chat_surface::{ChangeRoomEvent, ChangeRoomHandler};
use crate::chat::timeline_event::timeline_event;
use cntp_i18n::tr;
use contemporary::components::button::button;
use contemporary::components::grandstand::grandstand;
use contemporary::components::icon::icon;
use contemporary::components::icon_text::icon_text;
use contemporary::components::layer::layer;
use contemporary::components::text_field::TextField;
use gpui::http_client::anyhow;
use gpui::prelude::FluentBuilder;
use gpui::{
    App, AppContext, AsyncApp, Context, Entity, IntoElement, ListAlignment, ListOffset, ListState,
    ParentElement, Render, Styled, WeakEntity, Window, div, list, px,
};
use gpui_tokio::Tokio;
use matrix_sdk::deserialized_responses::TimelineEvent;
use matrix_sdk::ruma::OwnedRoomId;
use matrix_sdk::ruma::events::room::message::RoomMessageEventContent;
use std::rc::Rc;
use thegrid::admonition::{AdmonitionSeverity, admonition};
use thegrid::session::session_manager::SessionManager;

pub struct ChatRoom {
    room_id: OwnedRoomId,
    events: Vec<TimelineEvent>,
    on_change_room: Option<Rc<Box<ChangeRoomHandler>>>,
}

impl ChatRoom {
    pub fn new(
        room_id: OwnedRoomId,
        on_change_room: impl Fn(&ChangeRoomEvent, &mut Window, &mut App) + 'static,
        cx: &mut App,
    ) -> Entity<Self> {
        cx.new(|cx| {
            let session_manager = cx.global::<SessionManager>();
            let client = session_manager.client().unwrap();
            let client = client.read(cx);

            let Some(room) = client.get_room(&room_id) else {
                return Self {
                    room_id,
                    events: Vec::new(),
                    on_change_room: Some(Rc::new(Box::new(on_change_room))),
                };
            };

            cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                let event_cache = Tokio::spawn(cx, async move {
                    room.event_cache().await.map_err(|e| anyhow!(e))
                })
                .unwrap()
                .await
                .unwrap();

                if let Ok((event_cache, _)) = event_cache {
                    if event_cache
                        .pagination()
                        .run_backwards_until(100)
                        .await
                        .is_err()
                    {
                        return;
                    }

                    let (events, mut subscriber) = event_cache.subscribe().await;

                    if this
                        .update(cx, |this, cx| {
                            this.events = events;
                            cx.notify();
                        })
                        .is_err()
                    {
                        return;
                    };

                    loop {
                        subscriber.recv().await.unwrap();
                        let events = event_cache.events().await;
                        if this
                            .update(cx, |this, cx| {
                                this.events = events;
                                cx.notify();
                            })
                            .is_err()
                        {
                            return;
                        };
                    }
                }
            })
            .detach();

            Self {
                room_id,
                events: Vec::new(),
                on_change_room: Some(Rc::new(Box::new(on_change_room))),
            }
        })
    }
}

impl Render for ChatRoom {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let message_field = window.use_state(cx, |window, cx| {
            let text_field = TextField::new(cx, "message-field", "".into(), "".into());
            text_field.update(cx, |text_field, cx| {
                text_field.borderless(true);
            });
            text_field
        });
        let root_list_state =
            window.use_state(cx, |_, _| ListState::new(0, ListAlignment::Top, px(200.)));
        let message_field = message_field.read(cx);
        let root_list_state = root_list_state.read(cx);

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
        let events = self.events.clone();
        let room_id = self.room_id.clone();
        if events.len() != root_list_state.item_count() {
            root_list_state.reset(events.len());
            root_list_state.scroll_to(ListOffset {
                item_ix: events.len(),
                offset_in_item: px(0.),
            })
        }

        div()
            .flex()
            .flex_col()
            .size_full()
            .child(
                grandstand("main-area-grandstand")
                    .text(room.name().unwrap_or_default())
                    .pt(px(36.)),
            )
            .child(
                div().flex_grow().child(
                    list(root_list_state.clone(), move |i, _, cx| {
                        let event = events[i].clone();
                        let previous_event = if i == 0 {
                            None
                        } else {
                            events.get(i - 1).cloned()
                        };

                        timeline_event(event, previous_event, room_id.clone()).into_any_element()
                    })
                    .flex()
                    .flex_col()
                    .h_full(),
                ),
            )
            .when_else(
                room.is_tombstoned(),
                |david| {
                    let tombstone_content = room.tombstone_content().unwrap();

                    david.child(
                        div().p(px(2.)).child(
                            admonition()
                                .severity(AdmonitionSeverity::Info)
                                .title(tr!("ROOM_TOMBSTONED_TITLE", "This room has been replaced"))
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap(px(4.))
                                        .child(tr!(
                                            "ROOM_TOMBSTONED_TEXT",
                                            "Join the new room to keep the conversation going."
                                        ))
                                        .child(
                                            div().flex().child(div().flex_grow()).child(
                                                button("view-replaced-room-button")
                                                    .child(icon_text(
                                                        "arrow-right".into(),
                                                        tr!(
                                                            "ROOM_TOMBSTONED_NAVIGATE",
                                                            "Go to new room"
                                                        )
                                                        .into(),
                                                    ))
                                                    .on_click(cx.listener(
                                                        move |this, _, window, cx| {
                                                            if let Some(change_room_handler) =
                                                                &this.on_change_room
                                                            {
                                                                let event = ChangeRoomEvent {
                                                                    new_room: DisplayedRoom::Room(
                                                                        tombstone_content
                                                                            .replacement_room
                                                                            .clone(),
                                                                    ),
                                                                };
                                                                change_room_handler(
                                                                    &event, window, cx,
                                                                );
                                                            }
                                                        },
                                                    )),
                                            ),
                                        ),
                                ),
                        ),
                    )
                },
                |david| {
                    david.child(
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
                                        let content = RoomMessageEventContent::text_plain(
                                            message.to_string(),
                                        );
                                        let room_clone = room_clone.clone();

                                        cx.spawn(async move |cx| {
                                            Tokio::spawn_result(cx, async move {
                                                room_clone
                                                    .send(content)
                                                    .await
                                                    .map_err(|e| anyhow!(e))
                                            })
                                            .unwrap()
                                            .await;
                                        })
                                        .detach();
                                    }),
                            ),
                    )
                },
            )
    }
}
