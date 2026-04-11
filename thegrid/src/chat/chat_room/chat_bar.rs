use crate::auth::emoji_flyout::EmojiFlyout;
use crate::chat::chat_room::open_room::OpenRoom;
use crate::chat::chat_room::timeline_view::reply_fragment::reply_fragment;
use crate::chat::displayed_room::DisplayedRoom;
use cntp_i18n::{tr, trn};
use contemporary::components::admonition::{admonition, AdmonitionSeverity};
use contemporary::components::button::button;
use contemporary::components::icon::icon;
use contemporary::components::icon_text::icon_text;
use contemporary::components::layer::layer;
use contemporary::components::toast::Toast;
use contemporary::styling::theme::{ThemeStorage, VariableColor};
use gpui::prelude::FluentBuilder;
use gpui::{
    anchored, deferred, div, px, AppContext, AsyncApp, AsyncWindowContext,
    Context, Entity, InteractiveElement, IntoElement, ParentElement, Point, Render, Styled, WeakEntity, Window,
};
use matrix_sdk::ruma::events::room::tombstone::RoomTombstoneEventContent;
use matrix_sdk::ruma::events::MessageLikeEventType;
use matrix_sdk::RoomState;
use matrix_sdk_ui::timeline::TimelineItemContent;
use thegrid_common::session::room_cache::RoomJoinEvent;
use thegrid_common::session::session_manager::SessionManager;
use thegrid_common::tokio_helper::TokioHelper;

pub struct ChatBar {
    open_room: Entity<OpenRoom>,
    emoji_flyout: Option<Entity<EmojiFlyout>>,
}

impl ChatBar {
    pub fn new(open_room: Entity<OpenRoom>, cx: &mut Context<Self>) -> Self {
        Self {
            open_room,
            emoji_flyout: None,
        }
    }

    pub fn render_tombstone_content(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let open_room = self.open_room.read(cx);
        let room = open_room.room.as_ref().unwrap().clone();
        let replacement_room = room
            .tombstone_content()
            .map(|content| content.replacement_room);

        let session_manager = cx.global::<SessionManager>();
        let room_manager = session_manager.rooms().read(cx);

        let joining = room_manager.joining_room(room.room_id().to_owned());

        let enter_tombstoned_room = cx.listener({
            let replacement_room = replacement_room.clone();
            move |this, _, window, cx| {
                let session_manager = cx.global::<SessionManager>();
                let room_manager = session_manager.rooms().read(cx);

                let replacement_room = replacement_room.clone().unwrap();

                let joined_room = room_manager.room(&replacement_room).and_then(|room| {
                    let room = &room.read(cx).inner;
                    if room.state() == RoomState::Joined {
                        Some(room)
                    } else {
                        None
                    }
                });

                if joined_room.is_some() {
                    this.open_room
                        .read(cx)
                        .displayed_room
                        .clone()
                        .write(cx, DisplayedRoom::Room(replacement_room));
                } else {
                    // Join the tombstoned room
                    let callback = cx.listener({
                        let replacement_room = replacement_room.clone();
                        let room = room.clone();
                        move |this, event: &RoomJoinEvent, window, cx| {
                            if let Err(e) = &event.result {
                                Toast::new()
                                    .title(tr!("TOMBSTONE_JOIN_ERROR_TITLE").as_ref())
                                    .body(
                                        tr!(
                                            "TOMBSTONE_JOIN_ERROR_TEXT",
                                            room = replacement_room.to_string()
                                        )
                                        .as_ref(),
                                    )
                                    .severity(AdmonitionSeverity::Error)
                                    .post(window, cx);
                                return;
                            }

                            // Go to the replacement room
                            this.open_room
                                .read(cx)
                                .displayed_room
                                .clone()
                                .write(cx, DisplayedRoom::Room(replacement_room.clone()));

                            // Attempt to leave the old room in the background
                            let room = room.clone();
                            cx.spawn(
                                async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                                    let _ = cx.spawn_tokio(async move { room.leave().await }).await;
                                },
                            )
                            .detach();
                        }
                    });

                    // Go via the sender of the tombstone event
                    let room = room.clone();
                    cx.spawn_in(
                        window,
                        async move |weak_this: WeakEntity<Self>, cx: &mut AsyncWindowContext| {
                            let sender_via = cx
                                .spawn_tokio(async move {
                                    room.get_state_event_static::<RoomTombstoneEventContent>()
                                        .await
                                })
                                .await
                                .ok()
                                .flatten()
                                .and_then(|tombstone_event| tombstone_event.deserialize().ok())
                                .map(|tombstone_event| {
                                    tombstone_event.sender().server_name().to_owned()
                                });
                            let _ = cx.update(move |window, cx| {
                                let _ = weak_this.update(cx, move |this, cx| {
                                    let Some(sender_via) = sender_via else {
                                        Toast::new()
                                            .title(
                                                tr!(
                                                    "TOMBSTONE_JOIN_ERROR_TITLE",
                                                    "Unable to join the replacement room"
                                                )
                                                .as_ref(),
                                            )
                                            .body(
                                                tr!(
                                                    "TOMBSTONE_JOIN_ERROR_TEXT",
                                                    "Unable to join {{room}}",
                                                    room = replacement_room.to_string()
                                                )
                                                .as_ref(),
                                            )
                                            .severity(AdmonitionSeverity::Error)
                                            .post(window, cx);
                                        return;
                                    };

                                    let session_manager = cx.global::<SessionManager>();
                                    session_manager.rooms().update(cx, |room_manager, cx| {
                                        room_manager.join_room(
                                            replacement_room,
                                            false,
                                            vec![sender_via],
                                            callback,
                                            window,
                                            cx,
                                        )
                                    });
                                });
                            });
                        },
                    )
                    .detach();
                }
            }
        });

        div().p(px(2.)).child(
            admonition()
                .severity(AdmonitionSeverity::Info)
                .title(if replacement_room.is_some() {
                    tr!("ROOM_TOMBSTONED_TITLE", "This room has been replaced")
                } else {
                    tr!("ROOM_TERMINATED_TITLE", "This room has been terminated.")
                })
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(4.))
                        .child(if replacement_room.is_some() {
                            tr!(
                                "ROOM_TOMBSTONED_TEXT",
                                "Join the new room to keep the conversation going."
                            )
                        } else {
                            tr!("ROOM_TERMINATED_TEXT", "Thank you for your participation.")
                        })
                        .when(replacement_room.is_some(), |david| {
                            david.child(
                                div().flex().child(div().flex_grow()).child(
                                    button("view-replaced-room-button")
                                        .when(joining, |david| david.disabled())
                                        .child(icon_text(
                                            "arrow-right",
                                            tr!("ROOM_TOMBSTONED_NAVIGATE", "Go to new room"),
                                        ))
                                        .on_click(enter_tombstoned_room),
                                ),
                            )
                        }),
                ),
        )
    }
}

impl Render for ChatBar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let open_room = self.open_room.read(cx);
        let Some(room) = open_room.room.as_ref() else {
            return div().into_any_element();
        };

        if room.is_tombstoned() {
            return self.render_tombstone_content(cx).into_any_element();
        }
        let can_send_message = open_room.current_user.as_ref().is_some_and(|current_user| {
            current_user.can_send_message(MessageLikeEventType::Message)
        });

        let window_size = window.viewport_size();
        let inset = window.client_inset().unwrap_or_else(|| px(0.));

        let typing_users = &open_room.typing_users;

        let theme = cx.theme();

        div()
            .when_some(open_room.pending_reply.as_ref(), |david, pending_reply| {
                let TimelineItemContent::MsgLike(content) = pending_reply.content() else {
                    return david;
                };

                david.child(div().flex().child(div().w(px(32.))).child(reply_fragment(
                    pending_reply.content().clone(),
                    pending_reply.sender_profile().clone(),
                    pending_reply.sender().to_owned(),
                )))
            })
            .child(
                layer()
                    .m(px(2.))
                    .p(px(2.))
                    .gap(px(2.))
                    .flex()
                    .child(
                        button("attach_button")
                            .when(!can_send_message, |david| david.disabled())
                            .child(icon("mail-attachment"))
                            .flat()
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.open_room.update(cx, |open_room, cx| {
                                    open_room.show_attach_dialog(window, cx)
                                });
                            })),
                    )
                    .when_else(
                        can_send_message,
                        |david| david.child(open_room.chat_input.clone()),
                        |david| {
                            david.child(
                                div()
                                    .flex_grow()
                                    .self_center()
                                    .text_color(theme.foreground.disabled())
                                    .child(tr!(
                                        "CHAT_BAR_NO_SEND_PERMISSION",
                                        "You do not have permission to send messages in this room."
                                    )),
                            )
                        },
                    )
                    .child(
                        button("emoji")
                            .child("😀")
                            .flat()
                            .when(!can_send_message, |david| david.disabled())
                            .on_click(cx.listener(|this, _, _, cx| {
                                let chat_input = this.open_room.read(cx).chat_input.clone();
                                this.emoji_flyout = Some(cx.new(|cx| {
                                    let mut emoji_flyout = EmojiFlyout::new(cx);
                                    emoji_flyout.set_emoji_selected_listener(
                                        move |event, window, cx| {
                                            chat_input.update(cx, |chat_input, cx| {
                                                chat_input.type_string(&event.emoji, window, cx);
                                            });
                                        },
                                    );
                                    emoji_flyout
                                }));
                                cx.notify()
                            })),
                    )
                    .child(
                        button("send_button")
                            .child(icon("mail-send"))
                            .when(!can_send_message, |david| david.disabled())
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.open_room.update(cx, |open_room, cx| {
                                    open_room.send_pending_message(window, cx);
                                })
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
                                    .on_any_mouse_down(cx.listener(move |this, _, _, cx| {
                                        this.emoji_flyout = None;
                                        cx.notify()
                                    }))
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
            .into_any_element()
    }
}
