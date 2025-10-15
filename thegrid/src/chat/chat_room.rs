use crate::auth::emoji_flyout::{EmojiFlyout, EmojiSelectedEvent};
use crate::chat::chat_input::ChatInput;
use crate::chat::displayed_room::DisplayedRoom;
use crate::chat::timeline_event::room_head::room_head;
use crate::chat::timeline_event::timeline_event;
use cntp_i18n::{tr, trn};
use contemporary::components::admonition::{AdmonitionSeverity, admonition};
use contemporary::components::button::button;
use contemporary::components::grandstand::grandstand;
use contemporary::components::icon::icon;
use contemporary::components::icon_text::icon_text;
use contemporary::components::layer::layer;
use contemporary::components::spinner::spinner;
use contemporary::components::text_field::TextField;
use gpui::http_client::anyhow;
use gpui::prelude::FluentBuilder;
use gpui::{
    App, AppContext, AsyncApp, Context, Entity, InteractiveElement, IntoElement, ListAlignment,
    ListOffset, ListState, ParentElement, Point, Render, Styled, WeakEntity, Window, anchored,
    deferred, div, list, px,
};
use gpui_tokio::Tokio;
use log::{error, info};
use matrix_sdk::deserialized_responses::TimelineEvent;
use matrix_sdk::event_cache::RoomPaginationStatus;
use matrix_sdk::room::RoomMember;
use matrix_sdk::ruma::events::room::message::RoomMessageEventContent;
use matrix_sdk::ruma::{OwnedRoomId, OwnedUserId};
use std::rc::Rc;
use std::time::Duration;
use thegrid::session::session_manager::SessionManager;
use thegrid::tokio_helper::TokioHelper;
use tokio::sync::broadcast::error::RecvError;

pub struct ChatRoom {
    room_id: OwnedRoomId,
    events: Vec<TimelineEvent>,
    pagination_status: Entity<RoomPaginationStatus>,
    displayed_room: Entity<DisplayedRoom>,
    chat_input: Entity<ChatInput>,
    emoji_flyout: Option<Entity<EmojiFlyout>>,
    typing_users: Vec<RoomMember>,
}

impl ChatRoom {
    pub fn new(
        room_id: OwnedRoomId,
        displayed_room: Entity<DisplayedRoom>,
        cx: &mut App,
    ) -> Entity<Self> {
        cx.new(|cx| {
            let pagination_status = cx.new(|_| RoomPaginationStatus::Idle {
                hit_timeline_start: false,
            });

            let enter_press_listener = cx.listener(|this: &mut Self, _, window, cx| {
                this.send_pending_message(window, cx);
            });
            let text_changed_listener = cx.listener(|this: &mut Self, _, window, cx| {
                this.text_changed(window, cx);
            });
            let chat_input = cx.new(|cx| {
                let mut chat_input = ChatInput::new(cx);
                chat_input.on_enter_press(enter_press_listener);
                chat_input.on_text_changed(text_changed_listener);
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
                    displayed_room,
                    chat_input,
                    emoji_flyout: None,
                    typing_users: Vec::new(),
                };
            };

            let (typing_notification_guard, mut typing_notification) =
                room.subscribe_to_typing_notifications();
            let room_clone = room.clone();
            cx.spawn(
                async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                    loop {
                        match typing_notification.recv().await {
                            Ok(notification) => {
                                let mut typing_users = Vec::new();
                                for user in notification {
                                    let member =
                                        room_clone.get_member(&user).await.unwrap().unwrap();
                                    typing_users.push(member);
                                }
                                if weak_this
                                    .update(cx, |this, cx| {
                                        this.typing_users = typing_users;
                                        cx.notify();
                                    })
                                    .is_err()
                                {
                                    return;
                                };
                            }
                            Err(_) => {
                                return;
                            }
                        }
                    }
                },
            )
            .detach();

            let pagination_status_clone = pagination_status.clone();
            cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                let room_clone = room.clone();
                let event_cache = Tokio::spawn(cx, async move {
                    room_clone.event_cache().await.map_err(|e| anyhow!(e))
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

                // Manually drop this here to move it into the closure
                drop(typing_notification_guard);
            })
            .detach();

            Self {
                room_id,
                events: Vec::new(),
                pagination_status: pagination_status.clone(),
                displayed_room,
                chat_input,
                emoji_flyout: None,
                typing_users: Vec::new(),
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

    pub fn text_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let room_id = self.room_id.clone();
        cx.on_next_frame(window, move |_, window, cx| {
            let session_manager = cx.global::<SessionManager>();
            let client = session_manager.client().unwrap().read(cx);
            let room = client.get_room(&room_id).unwrap();

            cx.spawn(async move |_, cx: &mut AsyncApp| {
                let _ = cx
                    .spawn_tokio(async move { room.typing_notice(true).await })
                    .await;
            })
            .detach();
        });
    }
}

impl Render for ChatRoom {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let root_list_state = window.use_state(cx, |_, _| {
            ListState::new(0, ListAlignment::Bottom, px(200.))
        });
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
        if events.len() + 1 != root_list_state.item_count() {
            root_list_state.reset(events.len() + 1);
            root_list_state.scroll_to(ListOffset {
                item_ix: events.len() + 1,
                offset_in_item: px(0.),
            })
        }

        let pagination_status = self.pagination_status.read(cx).clone();

        let window_size = window.viewport_size();
        let inset = window.client_inset().unwrap_or_else(|| px(0.));

        let typing_users = &self.typing_users;
        let room_id = self.room_id.clone();

        div()
            .flex()
            .flex_col()
            .size_full()
            .child(
                grandstand("main-area-grandstand")
                    .text(
                        room.cached_display_name()
                            .map(|name| name.to_string())
                            .or_else(|| room.name())
                            .unwrap_or_default(),
                    )
                    .pt(px(36.)),
            )
            .child(
                div().flex_grow().child(
                    list(root_list_state.clone(), move |i, _, cx| {
                        if i == 0 {
                            match pagination_status {
                                RoomPaginationStatus::Idle { hit_timeline_start } => {
                                    if hit_timeline_start {
                                        room_head(room_id.clone()).into_any_element()
                                    } else {
                                        div().child("Not Paginating").into_any_element()
                                    }
                                }
                                RoomPaginationStatus::Paginating => div()
                                    .w_full()
                                    .flex()
                                    .py(px(12.))
                                    .items_center()
                                    .justify_center()
                                    .child(spinner())
                                    .into_any_element(),
                            }
                        } else {
                            let event = events[i - 1].clone();
                            let previous_event = if i == 1 {
                                None
                            } else {
                                events.get(i - 2).cloned()
                            };

                            timeline_event(event, previous_event, room_clone.clone())
                                .into_any_element()
                        }
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
                                                            this.displayed_room.write(
                                                                cx,
                                                                DisplayedRoom::Room(
                                                                    tombstone_content
                                                                        .replacement_room
                                                                        .clone(),
                                                                ),
                                                            );
                                                        },
                                                    )),
                                            ),
                                        ),
                                ),
                        ),
                    )
                },
                |david| {
                    david
                        .child(
                            div().flex().child(match typing_users.len() {
                                0 => "".to_string(),
                                1 => tr!(
                                    "TYPING_NOTIFICATION_ONE",
                                    "{{user}} is typing...",
                                    user = typing_users[0]
                                        .display_name()
                                        .unwrap_or_default()
                                        .to_string()
                                )
                                .into(),
                                2 => tr!(
                                    "TYPING_NOTIFICATION_TWO",
                                    "{{user}} and {{user2}} are typing...",
                                    user = typing_users[0]
                                        .display_name()
                                        .unwrap_or_default()
                                        .to_string(),
                                    user2 = typing_users[1]
                                        .display_name()
                                        .unwrap_or_default()
                                        .to_string()
                                )
                                .into(),
                                3 => tr!(
                                    "TYPING_NOTIFICATION_THREE",
                                    "{{user}}, {{user2}} and {{user3}} are typing...",
                                    user = typing_users[0]
                                        .display_name()
                                        .unwrap_or_default()
                                        .to_string(),
                                    user2 = typing_users[1]
                                        .display_name()
                                        .unwrap_or_default()
                                        .to_string(),
                                    user3 = typing_users[2]
                                        .display_name()
                                        .unwrap_or_default()
                                        .to_string()
                                )
                                .into(),
                                _ => trn!(
                                    "TYPING_NOTIFICATION",
                                    "{{count}} user is typing...",
                                    "{{count}} users are typing...",
                                    count = typing_users.len() as isize
                                )
                                .into(),
                            }),
                        )
                        .child(
                            layer()
                                .p(px(2.))
                                .gap(px(2.))
                                .flex()
                                .child(self.chat_input.clone().into_any_element())
                                .child(button("emoji").child("😀").flat().on_click(cx.listener(
                                    |this, _, _, cx| {
                                        let chat_input = this.chat_input.clone();
                                        this.emoji_flyout = Some(cx.new(|cx| {
                                            let mut emoji_flyout = EmojiFlyout::new(cx);
                                            emoji_flyout.set_emoji_selected_listener(
                                                move |event, window, cx| {
                                                    chat_input.update(cx, |chat_input, cx| {
                                                        chat_input.type_string(
                                                            &event.emoji,
                                                            window,
                                                            cx,
                                                        );
                                                    });
                                                },
                                            );
                                            emoji_flyout
                                        }));
                                        cx.notify()
                                    },
                                )))
                                .child(
                                    button("send_button")
                                        .child(icon("mail-send".into()))
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.send_pending_message(window, cx);
                                        })),
                                )
                                .when_some(self.emoji_flyout.clone(), |david, emoji_flyout| {
                                    david.child(deferred(
                                        anchored().position(Point::new(px(0.), px(0.))).child(
                                            div()
                                                .top_0()
                                                .left_0()
                                                .w(window_size.width - inset - inset)
                                                .h(window_size.height - inset - inset)
                                                .m(inset)
                                                .occlude()
                                                .on_any_mouse_down(cx.listener(
                                                    move |this, _, _, cx| {
                                                        this.emoji_flyout = None;
                                                        cx.notify()
                                                    },
                                                ))
                                                .child(
                                                    anchored()
                                                        .position(Point::new(
                                                            window_size.width,
                                                            window_size.height,
                                                        ))
                                                        .child(emoji_flyout.into_any_element()),
                                                ),
                                        ),
                                    ))
                                }),
                        )
                },
            )
    }
}
