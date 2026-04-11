use crate::chat::chat_input::{ChatInput, PasteRichEvent};
use crate::chat::chat_room::chat_bar::ChatBar;
use crate::chat::chat_room::timeline::Timeline;
use crate::chat::chat_room::timeline_view::event_filter::event_filter;
use crate::chat::displayed_room::DisplayedRoom;
use cntp_i18n::tr;
use gpui::http_client::anyhow;
use gpui::private::anyhow;
use gpui::{
    App, AppContext, AsyncApp, AsyncWindowContext, ClipboardEntry, Context, Entity,
    PathPromptOptions, WeakEntity, Window,
};
use log::error;
use matrix_sdk::room::RoomMember;
use matrix_sdk::ruma::api::client::room::aliases::v3::Response;
use matrix_sdk::ruma::events::room::canonical_alias::RoomCanonicalAliasEventContent;
use matrix_sdk::ruma::events::room::message::RoomMessageEventContent;
use matrix_sdk::ruma::events::tag::Tags;
use matrix_sdk::ruma::events::{room, Mentions, MessageLikeEventType};
use matrix_sdk::ruma::{api, OwnedRoomAliasId, OwnedRoomId, UserId};
use matrix_sdk::{Error, HttpError, Room};
use matrix_sdk_ui::timeline::{AttachmentConfig, AttachmentSource, EventTimelineItem, RoomExt};
use mime2ext::mime2ext;
use std::fs::read;
use std::mem;
use std::path::PathBuf;
use thegrid_common::room::active_call_participants::track_active_call_participants;
use thegrid_common::session::session_manager::SessionManager;
use thegrid_common::tokio_helper::TokioHelper;

pub struct OpenRoom {
    pub room: Option<Room>,
    pub current_user: Option<RoomMember>,
    pub displayed_room: Entity<DisplayedRoom>,
    pub pending_attachments: Vec<PendingAttachment>,
    pub typing_users: Vec<RoomMember>,
    pub active_call_users: Entity<Vec<RoomMember>>,
    pub chat_input: Entity<ChatInput>,
    pub room_id: OwnedRoomId,
    pub chat_bar: Entity<ChatBar>,
    pub timeline: Option<Entity<Timeline>>,
    pub tags: Tags,
    pub pending_reply: Option<EventTimelineItem>,
    local_aliases: Vec<OwnedRoomAliasId>,
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
        let weak_this = cx.weak_entity();
        let chat_bar = cx.new(|cx| ChatBar::new(this_entity, cx));

        let enter_press_listener = cx.listener(|this: &mut Self, _, window, cx| {
            this.send_pending_message(window, cx);
        });
        let escape_press_listener = cx.listener(|this, _, window, cx| {
            this.escape_press(window, cx);
        });
        let text_changed_listener = cx.listener(|this: &mut Self, _, window, cx| {
            this.text_changed(window, cx);
        });
        let paste_rich_listener = cx.listener(Self::paste_rich);
        let chat_input = cx.new(|cx| {
            let mut chat_input = ChatInput::new(weak_this, cx);
            chat_input.on_enter_press(enter_press_listener);
            chat_input.on_escape_press(escape_press_listener);
            chat_input.on_text_changed(text_changed_listener);
            chat_input.on_paste_rich(paste_rich_listener);
            chat_input
        });

        let active_call_users = track_active_call_participants(room_id.clone(), cx);

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
            active_call_users,
            chat_input,
            timeline: None,
            tags: Default::default(),
            pending_reply: None,
            local_aliases: Vec::new(),
        };

        let Some(room) = client.get_room(&room_id) else {
            return self_return;
        };
        self_return.room = Some(room.clone());

        self_return.setup_acquire_own_user(cx);

        let room_clone = room.clone();
        cx.spawn(
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                let timeline = cx
                    .spawn_tokio(async move {
                        room_clone
                            .timeline_builder()
                            .event_filter(event_filter)
                            .build()
                            .await
                    })
                    .await;

                let Ok(timeline) = timeline else {
                    return;
                };

                let _ = weak_this
                    .update(cx, |this, cx| {
                        let timeline_entity = cx.new(|cx| Timeline::new(timeline, cx));
                        this.timeline = Some(timeline_entity.clone());
                        cx.notify();

                        this.paginate_backwards(cx);
                    })
                    .is_err();
            },
        )
        .detach();

        cx.spawn(
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                let tags = cx.spawn_tokio(async move { room.tags().await }).await;
                if let Ok(Some(tags)) = tags {
                    let _ = weak_this.update(cx, |this, cx| {
                        this.tags = tags;
                        cx.notify()
                    });
                }
            },
        )
        .detach();

        self_return.update_local_aliases(cx);

        self_return
    }

    pub fn paginate_backwards(&mut self, cx: &mut Context<Self>) {
        let weak_this = cx.weak_entity();
        if let Some(timeline) = self.timeline.as_ref() {
            timeline.update(cx, |timeline, cx| {
                if timeline.pagination_pending || timeline.pagination_at_top {
                    return;
                }

                timeline.pagination_pending = true;

                let timeline = timeline.inner.clone();
                cx.spawn(
                    async move |weak_timeline: WeakEntity<Timeline>, cx: &mut AsyncApp| {
                        let pagination_at_top = cx
                            .spawn_tokio(async move { timeline.paginate_backwards(50).await })
                            .await
                            .unwrap_or_else(|e| {
                                error!("Failed to paginate backwards: {}", e);
                                false
                            });
                        let _ = weak_timeline.update(cx, |timeline, cx| {
                            timeline.pagination_at_top = pagination_at_top;
                            timeline.pagination_pending = false;
                            cx.notify();
                        });
                        let _ = weak_this.update(cx, |this, cx| {
                            cx.notify();
                        });
                    },
                )
                .detach();
            });
        }
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
                    match tokio::task::unconstrained(typing_notification.recv()).await {
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
                ClipboardEntry::ExternalPaths(_) => {
                    // TODO: What is this? Do we paste a file?
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
        let can_send_message = self.current_user.as_ref().is_some_and(|current_user| {
            current_user.can_send_message(MessageLikeEventType::Message)
        });
        if !can_send_message {
            return;
        }

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
        let pending_reply = self.pending_reply.take();

        cx.on_next_frame(window, move |_, _, cx| {
            let message = chat_input.read(cx).text();
            if message.is_empty() && attachments.is_empty() {
                return;
            }

            let content = if message.is_empty() {
                None
            } else {
                Some(enrich_message(message))
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
                    if let Some(pending_reply) = pending_reply {
                        let _ = cx
                            .spawn_tokio(async move {
                                timeline
                                    .send_reply(
                                        content.into(),
                                        pending_reply.event_id().unwrap().to_owned(),
                                    )
                                    .await
                            })
                            .await;
                    } else {
                        let _ = cx
                            .spawn_tokio(async move { timeline.send(content.into()).await })
                            .await;
                    }
                }
            })
            .detach();

            chat_input.update(cx, |message_field, _| message_field.reset())
        });

        cx.notify();
    }

    pub fn escape_press(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.set_pending_reply(None, cx);
    }

    pub fn set_pending_reply(&mut self, event: Option<EventTimelineItem>, cx: &mut Context<Self>) {
        if event
            .as_ref()
            .is_none_or(|event| event.event_id().is_some())
        {
            self.pending_reply = event;
            cx.notify();
        }
    }

    pub fn remove_pending_attachment(&mut self, index: usize, cx: &mut Context<Self>) {
        self.pending_attachments.remove(index);
        cx.notify()
    }

    pub fn pagination_pending(&self, cx: &App) -> bool {
        match self.timeline.as_ref() {
            None => false,
            Some(timeline) => timeline.read(cx).pagination_pending,
        }
    }

    pub fn local_aliases(&self) -> Vec<OwnedRoomAliasId> {
        self.local_aliases.clone()
    }

    pub fn publish_local_alias(
        &mut self,
        alias: OwnedRoomAliasId,
        callback: impl FnOnce(&Result<(), HttpError>, &mut Window, &mut App) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let session_manager = cx.global::<SessionManager>();
        let client = session_manager.client().unwrap().read(cx).clone();
        let room_id = self.room_id.clone();

        cx.spawn_in(
            window,
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncWindowContext| {
                let result = cx
                    .spawn_tokio({
                        let alias = alias.clone();
                        async move { client.create_room_alias(&alias, &room_id).await }
                    })
                    .await;

                cx.update(|window, cx| {
                    let _ = weak_this.update(cx, |this, cx| {
                        this.local_aliases.push(alias);
                        cx.notify();

                        this.update_local_aliases(cx);
                    });
                    callback(&result, window, cx);
                })
            },
        )
        .detach();
    }

    pub fn unpublish_local_alias(
        &mut self,
        alias: OwnedRoomAliasId,
        callback: impl FnOnce(&Result<(), Error>, &mut Window, &mut App) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let session_manager = cx.global::<SessionManager>();
        let client = session_manager.client().unwrap().read(cx).clone();

        if !self.local_aliases.contains(&alias) {
            window.defer(cx, |window, cx| {
                callback(&Ok(()), window, cx);
            });
            return;
        }

        // If the alias is currently published, unpublish it first.
        let room = self.room.clone().unwrap();
        let canonical_alias = room.canonical_alias();
        let mut alt_aliases = room.alt_aliases();

        let state_event = if canonical_alias
            .as_ref()
            .is_some_and(|canonical_alias| canonical_alias == &alias)
            || alt_aliases.contains(&alias)
        {
            let mut event = RoomCanonicalAliasEventContent::new();
            event.alias = if canonical_alias
                .as_ref()
                .is_some_and(|canonical_alias| canonical_alias == &alias)
            {
                None
            } else {
                canonical_alias
            };
            alt_aliases.retain(|a| a != &alias);
            event.alt_aliases = alt_aliases;

            Some(event)
        } else {
            None
        };

        cx.spawn_in(
            window,
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncWindowContext| {
                if let Some(state_event) = state_event {
                    if let Err(e) = cx
                        .spawn_tokio(async move { room.send_state_event(state_event).await })
                        .await
                    {
                        let _ = cx.update(|window, cx| {
                            callback(&Err(e), window, cx);
                        });
                        return;
                    }
                }

                let result = cx
                    .spawn_tokio({
                        let alias = alias.clone();
                        async move { client.remove_room_alias(&alias).await }
                    })
                    .await;

                let _ = cx.update(|window, cx| {
                    let _ = weak_this.update(cx, |this, cx| {
                        this.local_aliases.retain(|a| a != &alias);
                        cx.notify();

                        this.update_local_aliases(cx);
                    });
                    callback(&result.map_err(|e| Error::Http(Box::new(e))), window, cx);
                });
            },
        )
        .detach();
    }

    pub fn publish_public_aliases(
        &mut self,
        canonical_alias: Option<OwnedRoomAliasId>,
        alt_aliases: Vec<OwnedRoomAliasId>,
        callback: impl FnOnce(
            &Result<api::client::state::send_state_event::v3::Response, Error>,
            &mut Window,
            &mut App,
        ) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let room = self.room.clone().unwrap();

        let mut event = RoomCanonicalAliasEventContent::new();
        event.alias = canonical_alias;
        event.alt_aliases = alt_aliases;

        cx.spawn_in(
            window,
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncWindowContext| {
                let result = cx
                    .spawn_tokio(async move { room.send_state_event(event).await })
                    .await;

                cx.update(|window, cx| {
                    callback(&result, window, cx);
                })
            },
        )
        .detach();
    }

    fn update_local_aliases(&mut self, cx: &mut Context<Self>) {
        let session_manager = cx.global::<SessionManager>();
        let client = session_manager.client().unwrap().read(cx).clone();

        let room_id = self.room_id.clone();

        cx.spawn(
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| match cx
                .spawn_tokio(async move {
                    client
                        .send(api::client::room::aliases::v3::Request::new(room_id))
                        .await
                })
                .await
            {
                Ok(response) => {
                    let _ = weak_this.update(cx, |this, cx| {
                        this.local_aliases = response.aliases;
                        cx.notify();
                    });
                }
                Err(e) => {
                    error!("Failed to get room aliases: {}", e);
                    return;
                }
            },
        )
        .detach();
    }
}

pub fn enrich_message(message: &str) -> RoomMessageEventContent {
    let original_message = message.to_string();
    let mut sent_message = String::new();
    let mut mentions = Mentions::new();

    let mut last_end = 0;
    for part in original_message.split_whitespace() {
        let start = part.as_ptr() as usize - original_message.as_ptr() as usize;
        let previous_whitespace = &original_message[last_end..start];
        if !previous_whitespace.is_empty() {
            sent_message.push_str(previous_whitespace);
        }
        last_end = start + part.len();

        if part == "@room" {
            mentions.room = true;
            sent_message.push_str(part);
        } else if let Ok(user_id) = UserId::parse(part) {
            sent_message.push_str(&format!("[{}]({})", part, user_id.matrix_to_uri()));
            mentions.user_ids.extend([user_id]);
        } else {
            sent_message.push_str(part);
        }
    }

    RoomMessageEventContent::text_markdown(sent_message).add_mentions(mentions)
}
