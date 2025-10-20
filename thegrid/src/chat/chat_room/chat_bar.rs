use crate::auth::emoji_flyout::EmojiFlyout;
use crate::chat::chat_input::{ChatInput, PasteRichEvent};
use crate::chat::chat_room::PendingAttachment;
use crate::chat::displayed_room::DisplayedRoom;
use cntp_i18n::{tr, trn};
use contemporary::components::admonition::{AdmonitionSeverity, admonition};
use contemporary::components::button::button;
use contemporary::components::icon::icon;
use contemporary::components::icon_text::icon_text;
use contemporary::components::layer::layer;
use gpui::http_client::anyhow;
use gpui::prelude::FluentBuilder;
use gpui::{
    AppContext, AsyncApp, ClipboardEntry, Context, Entity, InteractiveElement, IntoElement,
    ParentElement, PathPromptOptions, Point, Render, Styled, WeakEntity, Window, anchored,
    deferred, div, px,
};
use matrix_sdk::Room;
use matrix_sdk::room::RoomMember;
use matrix_sdk::ruma::OwnedRoomId;
use matrix_sdk::ruma::events::room::message::RoomMessageEventContent;
use mime2ext::mime2ext;
use std::fs::read;
use std::mem;
use std::path::PathBuf;
use thegrid::session::session_manager::SessionManager;
use thegrid::tokio_helper::TokioHelper;

pub struct ChatBar {
    room: Option<Room>,
    current_user: Option<RoomMember>,
    displayed_room: Entity<DisplayedRoom>,
    chat_input: Entity<ChatInput>,
    pending_attachments: Entity<Vec<PendingAttachment>>,
    emoji_flyout: Option<Entity<EmojiFlyout>>,
    typing_users: Vec<RoomMember>,
}

impl ChatBar {
    pub fn new(
        room_id: OwnedRoomId,
        displayed_room: Entity<DisplayedRoom>,
        pending_attachments: Entity<Vec<PendingAttachment>>,
        cx: &mut Context<Self>,
    ) -> Self {
        let session_manager = cx.global::<SessionManager>();
        let client = session_manager.client().unwrap().read(cx).clone();

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

        cx.spawn(
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                let Some(room) = client.get_room(&room_id) else {
                    return;
                };

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

        Self {
            room: None,
            current_user: None,
            displayed_room,
            pending_attachments,
            chat_input,
            emoji_flyout: None,
            typing_users: Vec::new(),
        }
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

    fn paste_rich(&mut self, event: &PasteRichEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.pending_attachments
            .update(cx, |pending_attachments, cx| {
                for entry in event.clipboard_item.entries() {
                    match entry {
                        ClipboardEntry::String(_) => {
                            // noop
                        }
                        ClipboardEntry::Image(image) => {
                            let suggested_extension = mime2ext(image.format.mime_type());

                            pending_attachments.push(PendingAttachment {
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
            })
    }

    pub fn attach_from_disk(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let file_contents = read(&path);

        self.pending_attachments
            .update(cx, |pending_attachments, _| {
                pending_attachments.push(PendingAttachment {
                    filename: path.file_name().unwrap().to_string_lossy().to_string(),
                    mime_type: "application/octet-stream".into(),
                    data: file_contents.map_err(|e| anyhow!(e)),
                });
            })
    }

    pub fn send_pending_message(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let chat_input = self.chat_input.clone();
        let room = self.room.clone().unwrap();
        let attachments = self
            .pending_attachments
            .update(cx, |pending_attachments, _| mem::take(pending_attachments));

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
}

impl Render for ChatBar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(room) = self.room.as_ref() else {
            return div();
        };

        let window_size = window.viewport_size();
        let inset = window.client_inset().unwrap_or_else(|| px(0.));

        let typing_users = &self.typing_users;

        div().when_else(
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
                        layer()
                            .m(px(2.))
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
            },
        )
    }
}
