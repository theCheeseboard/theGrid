use crate::call_manager::LivekitCallManager;
use crate::{CallMember, CallState, LivekitCall, StreamState};
use cntp_i18n::tr;
use contemporary::components::admonition::admonition;
use contemporary::components::button::button;
use contemporary::components::grandstand::grandstand;
use contemporary::components::icon::icon;
use contemporary::components::icon_text::icon_text;
use contemporary::components::layer::layer;
use contemporary::components::spinner::spinner;
use contemporary::styling::theme::ThemeStorage;
use gpui::prelude::FluentBuilder;
use gpui::{
    App, Context, Entity, IntoElement, ParentElement, Render, RenderOnce, Styled, Window, div, px,
    rgb,
};
use matrix_sdk::ruma::OwnedRoomId;
use std::rc::Rc;
use thegrid_common::mxc_image::{SizePolicy, mxc_image};
use thegrid_common::session::session_manager::SessionManager;
use thegrid_common::surfaces::{SurfaceChange, SurfaceChangeEvent, SurfaceChangeHandler};

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
        let call = self.call.read(cx);
        let call_members = call.call_members().read(cx);

        let call_manager = cx.global::<LivekitCallManager>();
        let mute = call_manager.mute();
        let deaf = call_manager.deaf();

        let (rows, cols) = match call_members.len() {
            1 => (1, 1),
            2 => (1, 2),
            3 => (2, 2),
            4 => (2, 2),
            5 => (3, 2),
            6 => (3, 2),
            7 => (3, 3),
            8 => (3, 3),
            9 => (3, 3),
            10 => (3, 4),
            11 => (3, 4),
            12 => (3, 4),
            // TODO: What if there are more than 16 people?
            _ => (4, 4),
        };

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
                            &SurfaceChangeEvent {
                                change: SurfaceChange::Pop,
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
                    .child(match call.state {
                        CallState::Connecting => div()
                            .flex_grow()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(spinner().size(px(32.))),
                        CallState::Active { .. } => call_members.iter().fold(
                            div()
                                .flex_grow()
                                .grid()
                                .grid_rows(rows)
                                .grid_cols(cols)
                                .m(px(16.))
                                .gap(px(16.)),
                            |david, call_member| {
                                david.child(CallMemberDisplay {
                                    call_member: call_member.clone(),
                                })
                            },
                        ),
                        CallState::Ended => div().flex_grow(),
                        CallState::Error(error) => div()
                            .flex_grow()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                admonition()
                                    .title(tr!(
                                        "CALL_CONNECTION_ERROR",
                                        "Unable to connect the call"
                                    ))
                                    .child(error.to_string()),
                            ),
                    })
                    .child(
                        div().flex().justify_center().p(px(16.)).child(
                            layer()
                                .border(px(1.))
                                .border_color(theme.border_color)
                                .p(px(8.))
                                .gap(px(8.))
                                .flex()
                                .child(
                                    div()
                                        .flex()
                                        .bg(theme.button_background)
                                        .rounded(theme.border_radius)
                                        .child(
                                            button("deaf")
                                                .p(px(16.))
                                                .child(
                                                    icon(
                                                        if *deaf.read(cx) {
                                                            "headphones"
                                                        } else {
                                                            "headphones"
                                                        }
                                                        .into(),
                                                    )
                                                    .size(24.),
                                                )
                                                .checked_when(*deaf.read(cx))
                                                .on_click(move |_, _, cx| {
                                                    let deafened = *deaf.read(cx);
                                                    deaf.write(cx, !deafened);
                                                }),
                                        )
                                        .child(
                                            button("mute")
                                                .p(px(16.))
                                                .child(
                                                    icon(
                                                        if *mute.read(cx) {
                                                            "mic-off"
                                                        } else {
                                                            "mic-on"
                                                        }
                                                        .into(),
                                                    )
                                                    .size(24.),
                                                )
                                                .checked_when(*mute.read(cx))
                                                .on_click(move |_, _, cx| {
                                                    let muted = *mute.read(cx);
                                                    mute.write(cx, !muted);
                                                }),
                                        ),
                                )
                                .child(
                                    button("hangup-call")
                                        .p(px(16.))
                                        .destructive()
                                        .child(icon("call-stop".into()).size(24.))
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.call.update(cx, |call, cx| {
                                                call.end_call(cx);
                                            });
                                            (this.on_surface_change)(
                                                &SurfaceChangeEvent {
                                                    change: SurfaceChange::Pop,
                                                },
                                                window,
                                                cx,
                                            )
                                        })),
                                ),
                        ),
                    ),
            )
    }
}

#[derive(IntoElement)]
struct CallMemberDisplay {
    call_member: CallMember,
}

impl RenderOnce for CallMemberDisplay {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let call_member = self.call_member;

        let connecting = matches!(call_member.mic_state, StreamState::Unavailable)
            && matches!(call_member.camera_state, StreamState::Unavailable)
            && matches!(call_member.screenshare_state, StreamState::Unavailable);
        let is_muted = matches!(call_member.mic_state, StreamState::Off);

        div()
            .bg(if call_member.mic_active {
                theme.info_accent_color
            } else {
                theme.layer_background
            })
            .rounded(theme.border_radius)
            .border(px(1.))
            .border_color(theme.border_color)
            .p(px(8.))
            .child(
                div()
                    .flex()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .child(
                        mxc_image(call_member.room_member.avatar_url())
                            .rounded(theme.border_radius)
                            .size(px(96.))
                            .size_policy(SizePolicy::Fit)
                            .when(connecting, |david| david.opacity(0.5)),
                    )
                    .child(
                        div()
                            .absolute()
                            .left_0()
                            .top_0()
                            .size_full()
                            .flex()
                            .items_end()
                            .child(
                                div()
                                    .flex()
                                    .flex_grow()
                                    .child(
                                        call_member
                                            .room_member
                                            .display_name()
                                            .unwrap_or_default()
                                            .to_string(),
                                    )
                                    .child(div().flex_grow())
                                    .when(is_muted, |david| david.child(icon("mic-off".into()))),
                            ),
                    ),
            )
    }
}
