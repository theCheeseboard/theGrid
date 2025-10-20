use crate::auth::emoji_flyout::EmojiFlyout;
use crate::chat::chat_room::open_room::OpenRoom;
use crate::chat::displayed_room::DisplayedRoom;
use cntp_i18n::{tr, trn};
use contemporary::components::admonition::{AdmonitionSeverity, admonition};
use contemporary::components::button::button;
use contemporary::components::icon::icon;
use contemporary::components::icon_text::icon_text;
use contemporary::components::layer::layer;
use gpui::prelude::FluentBuilder;
use gpui::{
    AppContext, Context, Entity, InteractiveElement, IntoElement, ParentElement, Point, Render,
    Styled, Window, anchored, deferred, div, px,
};

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
}

impl Render for ChatBar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let open_room = self.open_room.read(cx);
        let Some(room) = open_room.room.as_ref() else {
            return div();
        };

        let window_size = window.viewport_size();
        let inset = window.client_inset().unwrap_or_else(|| px(0.));

        let typing_users = &open_room.typing_users;

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
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.open_room
                                                        .read(cx)
                                                        .displayed_room
                                                        .clone()
                                                        .write(
                                                            cx,
                                                            DisplayedRoom::Room(
                                                                tombstone_content
                                                                    .replacement_room
                                                                    .clone(),
                                                            ),
                                                        );
                                                })),
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
                                        this.open_room.update(cx, |open_room, cx| {
                                            open_room.show_attach_dialog(window, cx)
                                        });
                                    })),
                            )
                            .child(open_room.chat_input.clone())
                            .child(button("emoji").child("😀").flat().on_click(cx.listener(
                                |this, _, _, cx| {
                                    let chat_input = this.open_room.read(cx).chat_input.clone();
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
