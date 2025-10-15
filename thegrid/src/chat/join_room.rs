use crate::chat::join_room::create_room_popover::CreateRoomPopover;
use cntp_i18n::tr;
use contemporary::components::button::button;
use contemporary::components::constrainer::constrainer;
use contemporary::components::dialog_box::{StandardButton, dialog_box};
use contemporary::components::grandstand::grandstand;
use contemporary::components::icon_text::icon_text;
use contemporary::components::layer::layer;
use contemporary::components::subtitle::subtitle;
use contemporary::styling::theme::Theme;
use gpui::{
    Context, ElementId, Entity, InteractiveElement, IntoElement, ParentElement, Render, Styled,
    Window, div, px,
};
use std::rc::Rc;
use thegrid::admonition::{AdmonitionSeverity, admonition};

pub mod create_room_popover;

pub struct JoinRoom {
    create_room_popover: Entity<CreateRoomPopover>,
}

impl JoinRoom {
    pub fn new(cx: &mut Context<Self>, create_room_popover: Entity<CreateRoomPopover>) -> Self {
        Self {
            create_room_popover,
        }
    }
}

impl Render for JoinRoom {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>();

        div()
            .bg(theme.background)
            .w_full()
            .h_full()
            .flex()
            .flex_col()
            .child(
                grandstand("join-room-grandstand")
                    .text(tr!("JOIN_ROOM", "Create or Join Room"))
                    .pt(px(36.)),
            )
            .child(
                constrainer("join-room-constrainer")
                    .flex()
                    .flex_col()
                    .w_full()
                    .p(px(8.))
                    .gap(px(8.))
                    .child(
                        layer()
                            .flex()
                            .flex_col()
                            .p(px(8.))
                            .w_full()
                            .child(subtitle(tr!("CREATE_ROOM_OPTIONS", "Create Room")))
                            .child({
                                button("create-room")
                                    .child(icon_text("list-add".into(), tr!("CREATE_ROOM").into()))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.create_room_popover.update(
                                            cx,
                                            |create_room_popover, cx| {
                                                create_room_popover.open(cx);
                                                cx.notify();
                                            },
                                        )
                                    }))
                            }),
                    ),
            )
    }
}
