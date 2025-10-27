use crate::auth::emoji_flyout::EmojiFlyout;
use crate::chat::chat_input::{ChatInput, PasteRichEvent};
use crate::chat::chat_room::chat_bar::ChatBar;
use crate::chat::chat_room::timeline::Timeline;
use crate::chat::chat_room::user_action_dialogs::UserActionDialogs;
use crate::chat::displayed_room::DisplayedRoom;
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
            displayed_room,
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
                                timeline
                                    .send_attachment(
                                        AttachmentSource::Data {
                                            filename: attachment.filename,
                                            bytes: data,
                                        },
                                        attachment.mime_type.parse().unwrap(),
                                        AttachmentConfig::default(),
                                    )
                                    .await
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
