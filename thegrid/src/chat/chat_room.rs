use crate::chat::chat_input::ChatInput;
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
use log::{error, info};
use matrix_sdk::deserialized_responses::TimelineEvent;
use matrix_sdk::event_cache::RoomPaginationStatus;
use matrix_sdk::ruma::OwnedRoomId;
use matrix_sdk::ruma::events::room::message::RoomMessageEventContent;
use std::rc::Rc;
use thegrid::admonition::{AdmonitionSeverity, admonition};
use thegrid::session::session_manager::SessionManager;
use thegrid::tokio_helper::TokioHelper;

pub struct ChatRoom {
    room_id: OwnedRoomId,
    events: Vec<TimelineEvent>,
    pagination_status: Entity<RoomPaginationStatus>,
    on_change_room: Option<Rc<Box<ChangeRoomHandler>>>,
    chat_input: Entity<ChatInput>,
}

impl ChatRoom {
    pub fn new(
        room_id: OwnedRoomId,
        on_change_room: impl Fn(&ChangeRoomEvent, &mut Window, &mut App) + 'static,
        cx: &mut App,
    ) -> Entity<Self> {
        cx.new(|cx| {
            let pagination_status = cx.new(|_| RoomPaginationStatus::Idle {
                hit_timeline_start: false,
            });

            let enter_press_listener = cx.listener(|this: &mut Self, _, window, cx| {
                this.send_pending_message(window, cx);
            });
            let chat_input = cx.new(|cx| {
                let mut chat_input = ChatInput::new(cx);
                chat_input.on_enter_press(enter_press_listener);
                chat_input
            });

            let session_manager = cx.global::<SessionManager>();
            let client = session_manager.client().unwrap();
            let client = client.read(cx);

            let Some(room) = client.get_room(&room_id) else {
                return Self {
                    room_id,
                    events: Vec::new(),
                    pagination_status: pagination_status.clone(),
                    on_change_room: Some(Rc::new(Box::new(on_change_room))),
                    chat_input,
                };
            };

            let pagination_status_clone = pagination_status.clone();
            cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                let event_cache = Tokio::spawn(cx, async move {
                    room.event_cache().await.map_err(|e| anyhow!(e))
                })
                .unwrap()
                .await
                .unwrap();

                if let Ok((event_cache, _)) = event_cache {
                    let event_cache_clone = event_cache.clone();
                    cx.spawn(async move |cx: &mut AsyncApp| {
                        cx.spawn_tokio(async move {
                            event_cache_clone
                                .pagination()
                                .run_backwards_until(100)
                                .await
                        })
                        .await
                    })
                    .detach();

                    let event_cache_clone = event_cache.clone();
                    cx.spawn(async move |cx: &mut AsyncApp| {
                        loop {
                            let event_cache_clone = event_cache_clone.clone();
                            let Ok(room_pagination_status) = cx
                                .spawn_tokio(async move {
                                    event_cache_clone
                                        .pagination()
                                        .status()
                                        .next()
                                        .await
                                        .ok_or(anyhow!("Event cache closed"))
                                })
                                .await
                            else {
                                return;
                            };

                            if pagination_status_clone
                                .write(cx, room_pagination_status)
                                .is_err()
                            {
                                return;
                            }
                        }
                    })
                    .detach();

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
                            info!("Event cache closed");
                            return;
                        };
                    }
                } else {
                    error!("Unable to get event cache for room")
                }
            })
            .detach();

            Self {
                room_id,
                events: Vec::new(),
                pagination_status: pagination_status.clone(),
                on_change_room: Some(Rc::new(Box::new(on_change_room))),
                chat_input,
            }
        })
    }

    pub fn send_pending_message(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let chat_input = self.chat_input.clone();
        let room_id = self.room_id.clone();

        cx.on_next_frame(window, move |_, window, cx| {
            let message = chat_input.read(cx).text();
            if message.is_empty() {
                return;
            }

            let content = RoomMessageEventContent::text_plain(message.to_string());

            let session_manager = cx.global::<SessionManager>();
            let client = session_manager.client().unwrap().read(cx);
            let room = client.get_room(&room_id).unwrap();

            cx.spawn(async move |_, cx: &mut AsyncApp| {
                let _ = cx
                    .spawn_tokio(async move { room.send(content).await })
                    .await;
            })
            .detach();

            chat_input.update(cx, |message_field, _| message_field.reset())
        });
    }
}

impl Render for ChatRoom {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let root_list_state =
            window.use_state(cx, |_, _| ListState::new(0, ListAlignment::Top, px(200.)));
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

        let room_clone = room.clone();
        let events = self.events.clone();
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

                        timeline_event(event, previous_event, room_clone.clone()).into_any_element()
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
                            .child(self.chat_input.clone().into_any_element())
                            .child(
                                button("send_button")
                                    .child(icon("mail-send".into()))
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.send_pending_message(window, cx);
                                    })),
                            ),
                    )
                },
            )
    }
}
