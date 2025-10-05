use cntp_i18n::tr;
use contemporary::components::button::button;
use contemporary::components::grandstand::grandstand;
use contemporary::components::icon::icon;
use contemporary::components::layer::layer;
use contemporary::components::text_field::TextField;
use gpui::http_client::anyhow;
use gpui::private::anyhow::Error;
use gpui::{
    App, AppContext, AsyncApp, Context, ElementId, Entity, InteractiveElement, IntoElement,
    ListAlignment, ListState, ParentElement, Render, RenderOnce, Styled, WeakEntity, Window, div,
    list, px,
};
use gpui_tokio::Tokio;
use matrix_sdk::deserialized_responses::{TimelineEvent, TimelineEventKind};
use matrix_sdk::event_cache::{EventCacheDropHandles, RoomEventCache, RoomEventCacheUpdate};
use matrix_sdk::linked_chunk::relational::IndexableItem;
use matrix_sdk::ruma::OwnedRoomId;
use matrix_sdk::ruma::events::AnyTimelineEvent;
use matrix_sdk::ruma::events::room::message::RoomMessageEventContent;
use std::sync::Arc;
use thegrid::session::session_manager::SessionManager;

pub struct ChatRoom {
    room_id: OwnedRoomId,
    events: Vec<TimelineEvent>,
}

impl ChatRoom {
    pub fn new(room_id: OwnedRoomId, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let session_manager = cx.global::<SessionManager>();
            let client = session_manager.client().unwrap();
            let client = client.read(cx);

            let Some(room) = client.get_room(&room_id) else {
                return Self {
                    room_id,
                    events: Vec::new(),
                };
            };

            cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                let event_cache = Tokio::spawn(cx, async move {
                    room.event_cache().await.map_err(|e| anyhow!(e))
                })
                .unwrap()
                .await
                .unwrap();

                if let Ok((event_cache, drop_handles)) = event_cache {
                    event_cache
                        .pagination()
                        .run_backwards_until(100)
                        .await
                        .unwrap();

                    let (events, mut subscriber) = event_cache.subscribe().await;

                    this.update(cx, |this, cx| {
                        this.events = events;
                        cx.notify();
                    })
                    .unwrap();

                    loop {
                        let update = subscriber.recv().await.unwrap();
                        let events = event_cache.events().await;
                        this.update(cx, |this, cx| {
                            this.events = events;
                            cx.notify();
                        })
                        .unwrap();
                    }
                }
            })
            .detach();

            Self {
                room_id,
                events: Vec::new(),
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
                        let id = event.id().to_string();

                        let event = match &event.kind {
                            TimelineEventKind::Decrypted(decrypted) => {
                                match decrypted.event.deserialize() {
                                    Ok(event) => Ok(event),
                                    Err(_) => Err(anyhow!("Unknown Error")),
                                }
                            }
                            TimelineEventKind::UnableToDecrypt { .. } => {
                                Err(anyhow!("Unable to decrypt"))
                            }
                            TimelineEventKind::PlainText { event } => match event.deserialize() {
                                Ok(event) => Ok(event.into_full_event(room_id.clone())),
                                Err(_) => Err(anyhow!("Unknown Error")),
                            },
                        };

                        div()
                            .id(ElementId::Name(id.into()))
                            .child(match event {
                                Ok(AnyTimelineEvent::MessageLike(message_like)) => {
                                    match message_like.original_content() {
                                        None => div()
                                            .child(tr!(
                                                "MESSAGE_REDACTED",
                                                "Kapoof. Gone. Sorry, but you were too late."
                                            ))
                                            .into_any_element(),
                                        Some(content) => {
                                            // let Some(content) =
                                            div().into_any_element()
                                        }
                                    }
                                }
                                Ok(AnyTimelineEvent::State(state)) => {
                                    div().child(format!("{:?}", state)).into_any_element()
                                }
                                Err(e) => div()
                                    .child(tr!(
                                        "MESSAGE_DECRYPTION_FAILURE",
                                        "Unable to decrypt this message. Check your verification \
                                        status and try again later."
                                    ))
                                    .into_any_element(),
                            })
                            .into_any_element()
                    })
                    .flex()
                    .flex_col()
                    .h_full(),
                ),
            )
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
