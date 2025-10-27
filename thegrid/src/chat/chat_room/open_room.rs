use crate::auth::emoji_flyout::EmojiFlyout;
use crate::chat::chat_input::{ChatInput, PasteRichEvent};
use crate::chat::chat_room::chat_bar::ChatBar;
use crate::chat::chat_room::timeline::Timeline;
use crate::chat::chat_room::user_action_dialogs::UserActionDialogs;
use crate::chat::displayed_room::DisplayedRoom;
use crate::chat::timeline_event::queued_event::QueuedEvent;
use cntp_i18n::tr;
use gpui::http_client::anyhow;
use gpui::private::anyhow;
use gpui::{
    AppContext, AsyncApp, ClipboardEntry, Context, Entity, ListAlignment, ListScrollEvent,
    ListState, PathPromptOptions, WeakEntity, Window, px,
};
use log::{error, info};
use matrix_sdk::Room;
use matrix_sdk::deserialized_responses::TimelineEvent;
use matrix_sdk::event_cache::{RoomEventCache, RoomPaginationStatus};
use matrix_sdk::room::RoomMember;
use matrix_sdk::ruma::OwnedRoomId;
use matrix_sdk::ruma::api::client::receipt::create_receipt::v3::ReceiptType;
use matrix_sdk::ruma::events::receipt::ReceiptThread;
use matrix_sdk::ruma::events::room::message::RoomMessageEventContent;
use matrix_sdk::send_queue::RoomSendQueueUpdate;
use matrix_sdk_ui::Timeline as MatrixUiTimeline;
use matrix_sdk_ui::timeline::{AttachmentConfig, AttachmentSource, Error, RoomExt};
use mime2ext::mime2ext;
use std::fs::read;
use std::mem;
use std::path::PathBuf;
use thegrid::session::session_manager::SessionManager;
use thegrid::thegrid_error::TheGridError;
use thegrid::tokio_helper::TokioHelper;

pub struct OpenRoom {
    pub room: Option<Room>,
    pub current_user: Option<RoomMember>,
    pub displayed_room: Entity<DisplayedRoom>,
    pub pending_attachments: Vec<PendingAttachment>,
    pub typing_users: Vec<RoomMember>,
    pub chat_input: Entity<ChatInput>,
    pub room_id: OwnedRoomId,
    pub events: Vec<TimelineEvent>,
    pub queued: Vec<Entity<QueuedEvent>>,
    pub event_cache: Option<Entity<RoomEventCache>>,
    pub pagination_status: RoomPaginationStatus,
    pub chat_bar: Entity<ChatBar>,
    pub timeline: Option<Entity<Timeline>>,
}

pub struct PendingAttachment {
    pub filename: String,
    pub mime_type: String,
    pub data: anyhow::Result<Vec<u8>>,
}

impl OpenRoom {
    pub fn new(
        room_id: OwnedRoomId,
        displayed_room: Entity<DisplayedRoom>,
        cx: &mut Context<Self>,
    ) -> Self {
        let this_entity = cx.entity();
        let chat_bar = cx.new(|cx| ChatBar::new(this_entity, cx));

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

        let session_manager = cx.global::<SessionManager>();
        let client = session_manager.client().unwrap();
        let client = client.read(cx);

        let mut self_return = Self {
            room: None,
            room_id: room_id.clone(),
            events: Vec::new(),
            pagination_status: RoomPaginationStatus::Idle {
                hit_timeline_start: false,
            },
            displayed_room,
            event_cache: None,
            queued: Vec::new(),
            pending_attachments: Vec::new(),
            chat_bar,
            current_user: None,
            typing_users: Vec::new(),
            chat_input,
            timeline: None,
        };

        let Some(room) = client.get_room(&room_id) else {
            return self_return;
        };
        self_return.room = Some(room.clone());

        self_return.setup_acquire_own_user(cx);
        self_return.setup_event_cache(cx);
        self_return.setup_send_queue(cx);

        cx.spawn(
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                let timeline = cx.spawn_tokio(async move { room.timeline().await }).await;

                if let Ok(timeline) = timeline {
                    let _ = weak_this.update(cx, |this, cx| {
                        let timeline_entity = cx.new(|cx| Timeline::new(timeline, cx));
                        this.timeline = Some(timeline_entity.clone());
                        cx.notify()
                    });
                }
            },
        )
        .detach();

        self_return
    }

    pub fn send_read_receipt(&mut self, cx: &mut Context<Self>) {
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

    fn setup_acquire_own_user(&mut self, cx: &mut Context<Self>) {
        let room = self.room.clone().unwrap();
        cx.spawn(
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                let room_clone = room.clone();

                if let Ok(Some(us)) = cx
                    .spawn_tokio(async move { room.get_member(room.own_user_id()).await })
                    .await
                {
                    let _ = weak_this.update(cx, |this, cx| {
                        this.room = Some(room_clone);
                        this.current_user = Some(us);
                        this.setup_typing_users_listener(cx);
                        cx.notify()
                    });
                }
            },
        )
        .detach();
    }

    fn setup_event_cache(&mut self, cx: &mut Context<Self>) {
        let room = self.room.clone().unwrap();
        cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let event_cache = cx
                .spawn_tokio(async move { room.event_cache().await })
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
                let this_clone = this.clone();
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
                                    .ok_or(TheGridError::new("Event Cache Closed"))
                            })
                            .await
                        else {
                            TheGridError::new("Event Cache Closed");
                            return;
                        };

                        if this_clone
                            .update(cx, |this, cx| {
                                this.pagination_status = room_pagination_status;
                            })
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
    }

    fn setup_send_queue(&mut self, cx: &mut Context<Self>) {
        let room_clone = self.room.clone().unwrap();
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
                let this_entity = cx.entity();
                this.queued = queue
                    .into_iter()
                    .map(|event| cx.new(|cx| QueuedEvent::new(event, this_entity.clone(), cx)))
                    .collect();
                cx.notify();
            });

            let room = room_clone.clone();
            loop {
                let Ok(update) = rx.recv().await else {
                    return;
                };

                if this
                    .update(cx, |this, cx| {
                        let this_entity = cx.entity();
                        match update {
                            RoomSendQueueUpdate::NewLocalEvent(event) => this
                                .queued
                                .push(cx.new(|cx| QueuedEvent::new(event, this_entity, cx))),
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
                        }
                    })
                    .is_err()
                {
                    return;
                }
            }
        })
        .detach();
    }

    fn setup_typing_users_listener(&mut self, cx: &mut Context<Self>) {
        let room = self.room.clone().unwrap();
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
                                let member = room_clone.get_member(&user).await.unwrap().unwrap();
                                typing_users.push(member);
                            }
                            if weak_this
                                .update(cx, |this, cx| {
                                    this.typing_users = typing_users;
                                    cx.notify();
                                })
                                .is_err()
                            {
                                drop(typing_notification_guard);
                                return;
                            };
                        }
                        Err(_) => {
                            drop(typing_notification_guard);
                            return;
                        }
                    }
                }
            },
        )
        .detach();
    }

    pub fn text_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let room = self.room.clone().unwrap();
        cx.on_next_frame(window, move |_, window, cx| {
            cx.spawn(async move |_, cx: &mut AsyncApp| {
                let _ = cx
                    .spawn_tokio(async move { room.typing_notice(true).await })
                    .await;
            })
            .detach();
        });
    }

    pub fn show_attach_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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

    pub fn attach_from_disk(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let file_contents = read(&path);

        self.pending_attachments.push(PendingAttachment {
            filename: path.file_name().unwrap().to_string_lossy().to_string(),
            mime_type: "application/octet-stream".into(),
            data: file_contents.map_err(|e| anyhow!(e)),
        });
    }

    pub fn send_pending_message(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let chat_input = self.chat_input.clone();
        let attachments = mem::take(&mut self.pending_attachments);

        let timeline = self.timeline.clone().unwrap().read(cx).inner.clone();

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
            
            cx.spawn(async move |_, cx: &mut AsyncApp| {
                for attachment in attachments.into_iter() {
                    if let Ok(data) = attachment.data {
                        let timeline = timeline.clone();
                        let _ = cx
                            .spawn_tokio(async move {
                                timeline.send_attachment(
                                    AttachmentSource::Data {
                                        filename: attachment.filename,
                                        bytes: data
                                    },
                                    attachment.mime_type.parse().unwrap(),
                                    AttachmentConfig::default(),
                                ).await
                            })
                            .await;
                    }
                }

                if let Some(content) = content {
                    let _ = cx
                        .spawn_tokio(async move { timeline.send(content.into()).await })
                        .await;
                }
            })
            .detach();

            chat_input.update(cx, |message_field, _| message_field.reset())
        });

        cx.notify();
    }

    pub fn remove_pending_attachment(&mut self, index: usize, cx: &mut Context<Self>) {
        self.pending_attachments.remove(index);
        cx.notify()
    }
}
