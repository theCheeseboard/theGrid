use crate::LivekitCall;
use crate::call_manager::LivekitCallManager;
use cntp_i18n::tr;
use contemporary::components::button::button;
use contemporary::components::grandstand::grandstand;
use contemporary::components::icon::icon;
use contemporary::components::icon_text::icon_text;
use contemporary::components::layer::layer;
use contemporary::styling::theme::ThemeStorage;
use gpui::{Context, Entity, IntoElement, ParentElement, Render, Styled, Window, div, px, rgb};
use matrix_sdk::ruma::OwnedRoomId;
use std::rc::Rc;
use thegrid_common::session::session_manager::SessionManager;
use thegrid_common::surfaces::SurfaceChangeHandler;

pub struct CallPage {
    call: Entity<LivekitCall>,
    room_id: OwnedRoomId,
    on_surface_change: Rc<Box<SurfaceChangeHandler>>,
}

impl CallPage {
    pub fn new(
        room_id: OwnedRoomId,
        on_surface_change: Rc<Box<SurfaceChangeHandler>>,
        cx: &mut Context<Self>,
    ) -> Self {
        let call_manager = cx.global::<LivekitCallManager>();
        let call = call_manager
            .calls()
            .iter()
            .find(|call| call.read(cx).room == room_id)
            .unwrap()
            .clone();

        Self {
            call,
            room_id,
            on_surface_change,
        }
    }
}

impl Render for CallPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let session_manager = cx.global::<SessionManager>();
        let room = session_manager
            .rooms()
            .read(cx)
            .room(&self.room_id)
            .unwrap()
            .read(cx);
        let room_name = room.display_name().clone();

        let theme = cx.theme();

        div()
            .size_full()
            .bg(rgb(0x000000))
            .flex()
            .flex_col()
            .flex_grow()
            .child(
                grandstand("call-join")
                    .text(room_name)
                    .pt(px(36.))
                    .on_back_click(cx.listener(move |this, _, window, cx| {
                        (this.on_surface_change)(
                            &thegrid_common::surfaces::SurfaceChangeEvent {
                                change: thegrid_common::surfaces::SurfaceChange::Pop,
                            },
                            window,
                            cx,
                        )
                    })),
            )
            .child(
                div()
                    .flex_grow()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .flex_grow()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child("active call or something idk"),
                    )
                    .child(
                        div().flex().justify_center().p(px(16.)).child(
                            layer()
                                .border(px(1.))
                                .border_color(theme.border_color)
                                .p(px(8.))
                                .flex()
                                .child(
                                    button("hangup-call")
                                        .destructive()
                                        .child(icon("call-stop".into()).size(24.))
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.call.update(cx, |call, cx| {
                                                call.end_call(cx);
                                            })
                                        })),
                                ),
                        ),
                    ),
            )
    }
}
