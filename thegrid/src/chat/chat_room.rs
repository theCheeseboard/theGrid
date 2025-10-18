use crate::auth::emoji_flyout::{EmojiFlyout, EmojiSelectedEvent};
use crate::chat::chat_input::{ChatInput, PasteRichEvent};
use crate::chat::displayed_room::DisplayedRoom;
use crate::chat::timeline_event::queued_event::QueuedEvent;
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
use contemporary::components::subtitle::subtitle;
use contemporary::components::text_field::TextField;
use contemporary::styling::theme::Theme;
use gpui::http_client::anyhow;
use gpui::prelude::FluentBuilder;
use gpui::private::anyhow;
use gpui::{
    App, AppContext, AsyncApp, ClipboardEntry, Context, Entity, ExternalPaths, InteractiveElement,
    IntoElement, ListAlignment, ListOffset, ListScrollEvent, ListState, ParentElement,
    PathPromptOptions, Point, Render, StatefulInteractiveElement, Styled, WeakEntity, Window,
    anchored, deferred, div, list, px, relative,
};
use gpui_tokio::Tokio;
use log::{error, info};
use matrix_sdk::attachment::AttachmentConfig;
use matrix_sdk::deserialized_responses::TimelineEvent;
use matrix_sdk::event_cache::{RoomEventCache, RoomPaginationStatus};
use matrix_sdk::room::RoomMember;
use matrix_sdk::ruma::api::client::receipt::create_receipt::v3::ReceiptType;
use matrix_sdk::ruma::events::fully_read::FullyReadEventContent;
use matrix_sdk::ruma::events::receipt::ReceiptThread;
use matrix_sdk::ruma::events::room::message::RoomMessageEventContent;
use matrix_sdk::ruma::{OwnedRoomId, OwnedUserId};
use matrix_sdk::send_queue::{LocalEcho, RoomSendQueueUpdate};
use mime2ext::mime2ext;
use std::fs::read;
use std::mem;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;
use thegrid::session::session_manager::SessionManager;
use thegrid::tokio_helper::TokioHelper;
use tokio::sync::broadcast::error::RecvError;

pub struct ChatRoom {
    room_id: OwnedRoomId,
    events: Vec<TimelineEvent>,
    queued: Vec<Entity<QueuedEvent>>,
    event_cache: Option<Entity<RoomEventCache>>,
    pagination_status: Entity<RoomPaginationStatus>,
    displayed_room: Entity<DisplayedRoom>,
    chat_input: Entity<ChatInput>,
    emoji_flyout: Option<Entity<EmojiFlyout>>,
    typing_users: Vec<RoomMember>,
    list_state: ListState,
    pending_attachments: Vec<PendingAttachment>,
}

struct PendingAttachment {
    filename: String,
    mime_type: String,
    data: anyhow::Result<Vec<u8>>,
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
            let paste_rich_listener = cx.listener(Self::paste_rich);
            let chat_input = cx.new(|cx| {
                let mut chat_input = ChatInput::new(cx);
                chat_input.on_enter_press(enter_press_listener);
                chat_input.on_text_changed(text_changed_listener);
                chat_input.on_paste_rich(paste_rich_listener);
                chat_input
            });

            let list_state = ListState::new(0, ListAlignment::Bottom, px(200.));
            list_state.set_scroll_handler(cx.listener(|this, event: &ListScrollEvent, _, cx| {
                if event.visible_range.end == this.events.len() {
                    this.send_read_receipt(cx);
                }
            }));

            let session_manager = cx.global::<SessionManager>();
            let client = session_manager.client().unwrap();
            let client = client.read(cx);

            let self_return = Self {
                room_id: room_id.clone(),
                events: Vec::new(),
                pagination_status: pagination_status.clone(),
                displayed_room,
                chat_input,
                emoji_flyout: None,
                typing_users: Vec::new(),
                event_cache: None,
                queued: Vec::new(),
                list_state,
                pending_attachments: Vec::new(),
            };

            let Some(room) = client.get_room(&room_id) else {
                return self_return;
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
            let room_clone = room.clone();
            cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                let event_cache = cx
                    .spawn_tokio(async move { room_clone.event_cache().await })
                    .await;

                if let Ok((event_cache, _)) = event_cache {
                    if this
                        .update(cx, |this, cx| {
                            let event_cache_entity = cx.new(|_| event_cache.clone());
                            this.event_cache = Some(event_cache_entity)
                        })
                        .is_err()
                    {
                        return;
                    }

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

            let room_clone = room.clone();
            cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                let room = room_clone.clone();
                let send_queue = room.send_queue();
                let Ok((queue, mut rx)) = cx
                    .spawn_tokio(async move { send_queue.subscribe().await })
                    .await
                else {
                    return;
                };

                let room = room_clone.clone();
                let _ = this.update(cx, |this, cx| {
                    this.queued = queue
                        .into_iter()
                        .map(|event| cx.new(|cx| QueuedEvent::new(event, room.clone(), cx)))
                        .collect();
                    cx.notify();
                });

                let room = room_clone.clone();
                loop {
                    let Ok(update) = rx.recv().await else {
                        return;
                    };

                    if this
                        .update(cx, |this, cx| match update {
                            RoomSendQueueUpdate::NewLocalEvent(event) => this
                                .queued
                                .push(cx.new(|cx| QueuedEvent::new(event, room.clone(), cx))),
                            RoomSendQueueUpdate::CancelledLocalEvent { transaction_id } => {
                                this.queued.retain(|event| {
                                    event.read(cx).transaction_id() != transaction_id
                                });
                            }
                            RoomSendQueueUpdate::ReplacedLocalEvent {
                                transaction_id,
                                new_content,
                            } => {
                                for queue_item in &this.queued {
                                    if queue_item.read(cx).transaction_id() == transaction_id {
                                        queue_item.update(cx, |queue_item, cx| {
                                            queue_item.notify_content_replaced(new_content, cx);
                                        });
                                        return;
                                    }
                                }
                            }
                            RoomSendQueueUpdate::SendError {
                                transaction_id,
                                is_recoverable,
                                ..
                            } => {
                                for queue_item in &this.queued {
                                    if queue_item.read(cx).transaction_id() == transaction_id {
                                        queue_item.update(cx, |queue_item, cx| {
                                            queue_item.notify_send_error(is_recoverable, cx);
                                        });
                                        return;
                                    }
                                }
                            }
                            RoomSendQueueUpdate::RetryEvent { transaction_id } => {
                                for queue_item in &this.queued {
                                    if queue_item.read(cx).transaction_id() == transaction_id {
                                        queue_item.update(cx, |queue_item, cx| {
                                            queue_item.notify_unwedged(cx);
                                            cx.notify()
                                        });
                                        return;
                                    }
                                }
                            }
                            RoomSendQueueUpdate::SentEvent {
                                transaction_id,
                                event_id,
                            } => {
                                this.queued.retain(|event| {
                                    event.read(cx).transaction_id() != transaction_id
                                });
                            }
                            RoomSendQueueUpdate::MediaUpload {
                                related_to,
                                file,
                                index,
                                progress,
                            } => {
                                for queue_item in &this.queued {
                                    if queue_item.read(cx).transaction_id() == related_to {
                                        queue_item.update(cx, |queue_item, cx| {
                                            queue_item
                                                .notify_upload_progress(file, index, progress, cx);
                                            cx.notify()
                                        });
                                        return;
                                    }
                                }
                            }
                        })
                        .is_err()
                    {
                        return;
                    }
                }
            })
            .detach();

            self_return
        })
    }

    pub fn send_pending_message(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let chat_input = self.chat_input.clone();
        let room_id = self.room_id.clone();
        let attachments = mem::take(&mut self.pending_attachments);

        cx.on_next_frame(window, move |_, _, cx| {
            let message = chat_input.read(cx).text();
            if message.is_empty() && attachments.is_empty() {
                return;
            }

            let content = if message.is_empty() {
                None
            } else {
                Some(RoomMessageEventContent::text_plain(message.to_string()))
            };

            let session_manager = cx.global::<SessionManager>();
            let client = session_manager.client().unwrap().read(cx);
            let room = client.get_room(&room_id).unwrap();
            let send_queue = room.send_queue();

            cx.spawn(async move |_, cx: &mut AsyncApp| {
                for attachment in attachments.into_iter() {
                    if let Ok(data) = attachment.data {
                        let send_queue = send_queue.clone();
                        let _ = cx
                            .spawn_tokio(async move {
                                send_queue
                                    .send_attachment(
                                        attachment.filename,
                                        attachment.mime_type.parse().unwrap(),
                                        data,
                                        Default::default(),
                                    )
                                    .await
                            })
                            .await;
                    }
                }

                if let Some(content) = content {
                    let _ = cx
                        .spawn_tokio(async move { send_queue.send(content.into()).await })
                        .await;
                }
            })
            .detach();

            chat_input.update(cx, |message_field, _| message_field.reset())
        });

        cx.notify();
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

    fn send_read_receipt(&mut self, cx: &mut Context<Self>) {
        let room_id = self.room_id.clone();
        let session_manager = cx.global::<SessionManager>();
        let client = session_manager.client().unwrap().read(cx);
        let room = client.get_room(&room_id).unwrap();

        let Some(latest_event) = self.events.last() else {
            return;
        };
        let Some(latest_event_id) = latest_event.event_id() else {
            return;
        };

        cx.spawn(async move |_, cx: &mut AsyncApp| {
            let _ = cx
                .spawn_tokio(async move {
                    room.send_single_receipt(
                        ReceiptType::Read,
                        ReceiptThread::Unthreaded,
                        latest_event_id,
                    )
                    .await
                })
                .await;
        })
        .detach();
    }

    fn paste_rich(&mut self, event: &PasteRichEvent, _: &mut Window, cx: &mut Context<Self>) {
        for entry in event.clipboard_item.entries() {
            match entry {
                ClipboardEntry::String(_) => {
                    // noop
                }
                ClipboardEntry::Image(image) => {
                    let suggested_extension = mime2ext(image.format.mime_type());

                    self.pending_attachments.push(PendingAttachment {
                        filename: match suggested_extension {
                            None => "image".into(),
                            Some(suggested_extension) => {
                                format!("image.{suggested_extension}")
                            }
                        },
                        mime_type: image.format.mime_type().into(),
                        data: Ok(image.bytes.clone()),
                    });
                }
            }
        }

        cx.notify();
    }

    fn render_attachments(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>();
        div()
            .absolute()
            .left_0()
            .top_0()
            .size_full()
            .flex()
            .items_end()
            .justify_end()
            .child(
                div()
                    .id("attachment-list")
                    .rounded(theme.border_radius)
                    .bg(theme.background)
                    .border(px(1.))
                    .border_color(theme.border_color)
                    .occlude()
                    .m(px(8.))
                    .p(px(4.))
                    .gap(px(4.))
                    .w(px(400.))
                    .max_h(relative(0.7))
                    .overflow_y_scroll()
                    .child(subtitle(tr!("ATTACHMENTS_TITLE", "Attachments")))
                    .child(self.pending_attachments.iter().enumerate().fold(
                        div().flex().flex_col().gap(px(4.)),
                        |david, (i, attachment)| {
                            david.child(
                                div().id(i).child(
                                    layer()
                                        .flex()
                                        .flex_col()
                                        .p(px(2.))
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .child(attachment.filename.clone())
                                                .child(div().flex_grow())
                                                .child(
                                                    button("delete-button")
                                                        .flat()
                                                        .child(icon("edit-delete".into()))
                                                        .on_click(cx.listener(
                                                            move |this, _, _, cx| {
                                                                this.pending_attachments.remove(i);
                                                                cx.notify()
                                                            },
                                                        )),
                                                ),
                                        )
                                        .when(attachment.data.is_err(), |david| {
                                            david.child(
                                                admonition()
                                                    .severity(AdmonitionSeverity::Error)
                                                    .title(tr!("ATTACH_ERROR", "Attachment Error"))
                                                    .child(
                                                        attachment
                                                            .data
                                                            .as_ref()
                                                            .unwrap_err()
                                                            .to_string(),
                                                    ),
                                            )
                                        }),
                                ),
                            )
                        },
                    )),
            )
    }

    fn show_attach_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            multiple: true,
            files: true,
            directories: false,
            prompt: Some(tr!("ATTACH_PROMPT", "Attach").into()),
        });

        cx.spawn(
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                if let Some(path) = prompt.await.ok().and_then(|result| result.ok()).flatten() {
                    weak_this
                        .update(cx, |this, cx| {
                            for path in path {
                                this.attach_from_disk(path, cx);
                            }
                            cx.notify()
                        })
                        .unwrap();
                };
            },
        )
        .detach();
    }

    fn attach_from_disk(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let file_contents = read(&path);

        self.pending_attachments.push(PendingAttachment {
            filename: path.file_name().unwrap().to_string_lossy().to_string(),
            mime_type: "application/octet-stream".into(),
            data: file_contents.map_err(|e| anyhow!(e)),
        });
    }
}

impl Render for ChatRoom {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
        if events.len() + self.queued.len() + 1 != self.list_state.item_count() {
            self.list_state.reset(events.len() + self.queued.len() + 1);
            self.list_state.scroll_to(ListOffset {
                item_ix: events.len() + 1,
                offset_in_item: px(0.),
            })
        }

        let pagination_status = *self.pagination_status.read(cx);

        let window_size = window.viewport_size();
        let inset = window.client_inset().unwrap_or_else(|| px(0.));

        let typing_users = &self.typing_users;
        let room_id = self.room_id.clone();

        let theme = cx.global::<Theme>();

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
                div()
                    .flex_grow()
                    .child(
                        list(
                            self.list_state.clone(),
                            cx.processor(move |this, i, _, cx| {
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
                                } else if i < events.len() + 1 {
                                    let event: &TimelineEvent = &events[i - 1];
                                    let event = event.clone();
                                    let previous_event = if i == 1 {
                                        None
                                    } else {
                                        events.get(i - 2).cloned()
                                    };

                                    let event_cache = this.event_cache.clone().unwrap();

                                    timeline_event(
                                        event,
                                        previous_event,
                                        event_cache,
                                        room_clone.clone(),
                                    )
                                    .into_any_element()
                                } else {
                                    let event: &Entity<QueuedEvent> =
                                        &this.queued[i - events.len() - 1];
                                    let previous_event = if i == 1 {
                                        None
                                    } else {
                                        events.get(i - 2).cloned()
                                    };

                                    event.update(cx, |event, cx| {
                                        event.previous_event = previous_event;
                                    });

                                    event.clone().into_any_element()
                                }
                            }),
                        )
                        .flex()
                        .flex_col()
                        .h_full(),
                    )
                    .when(!self.pending_attachments.is_empty(), |david| {
                        david.child(self.render_attachments(window, cx))
                    }),
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
                                .child(
                                    button("attach_button")
                                        .child(icon("mail-attachment".into()))
                                        .flat()
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.show_attach_dialog(window, cx)
                                        })),
                                )
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
            .child(
                div()
                    .absolute()
                    .left_0()
                    .top_0()
                    .size_full()
                    .on_drop(cx.listener(|this, event: &ExternalPaths, _, cx| {
                        for path in event.paths() {
                            this.attach_from_disk(path.clone(), cx);
                        }
                    })),
            )
    }
}
