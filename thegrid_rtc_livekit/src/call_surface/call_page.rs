use crate::call_manager::LivekitCallManager;
use crate::call_surface::call_page::webcam_start_dialog::WebcamStartDialog;
use crate::{CallMember, CallState, LivekitCall, StreamState};
use cntp_i18n::tr;
use contemporary::components::button::button;
use contemporary::components::grandstand::grandstand;
use contemporary::components::icon::icon;
use contemporary::components::icon_text::icon_text;
use contemporary::components::interstitial::interstitial;
use contemporary::components::layer::layer;
use contemporary::components::spinner::spinner;
use contemporary::styling::theme::ThemeStorage;
use gpui::prelude::FluentBuilder;
use gpui::{
    App, AppContext, BorrowAppContext, Context, Entity, IntoElement, ObjectFit, ParentElement,
    Render, RenderOnce, Styled, StyledImage, Window, div, img, px, rgb,
};
use matrix_sdk::ruma::OwnedRoomId;
use std::rc::Rc;
use thegrid_common::mxc_image::{SizePolicy, mxc_image};
use thegrid_common::session::session_manager::SessionManager;
use thegrid_common::surfaces::{SurfaceChange, SurfaceChangeEvent, SurfaceChangeHandler};

mod webcam_start_dialog;

pub struct CallPage {
    call: Entity<LivekitCall>,
    room_id: OwnedRoomId,
    on_surface_change: Rc<Box<SurfaceChangeHandler>>,

    webcam_start_dialog: Entity<WebcamStartDialog>,
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

        let webcam_start_dialog = cx.new(|cx| WebcamStartDialog::new(cx));

        Self {
            call,
            room_id,
            on_surface_change,
            webcam_start_dialog,
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
            3..=4 => (2, 2),
            5..=6 => (3, 2),
            7..=9 => (3, 3),
            10..=12 => (3, 4),
            // If there are more than 16 people, arrange in a grid of 4 columns
            _ => ((call_members.len() / 4 + 1) as u16, 4),
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
                    .when_else(
                        call.on_hold,
                        |david| {
                            david.child(
                                interstitial()
                                    .flex_grow()
                                    .icon("media-playback-pause".into())
                                    .title(tr!("CALL_ON_HOLD", "This call is on hold").into())
                                    .message(
                                        tr!(
                                            "CALL_ON_HOLD_MESSAGE",
                                            "Take the call off hold to continue talking"
                                        )
                                        .into(),
                                    )
                                    .child(
                                        button("resume-call")
                                            .child(icon_text(
                                                "call-start".into(),
                                                tr!("CALL_TAKE_OFF_HOLD", "Take off hold").into(),
                                            ))
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                let call = this.call.clone();
                                                cx.update_global::<LivekitCallManager, _>(
                                                    |call_manager, cx| {
                                                        if call_manager.current_call()
                                                            == Some(call.clone())
                                                        {
                                                            call.update(cx, |call, cx| {
                                                                call.set_on_hold(false, cx);
                                                            })
                                                        } else {
                                                            call_manager
                                                                .switch_to_call(call.clone(), cx);
                                                        }
                                                    },
                                                );
                                            })),
                                    ),
                            )
                        },
                        |david| {
                            david.child(match call.state {
                                CallState::Connecting => div()
                                    .flex_grow()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(spinner().size(px(32.)))
                                    .into_any_element(),
                                CallState::Active { .. } => call_members
                                    .iter()
                                    .fold(
                                        div()
                                            .flex_grow()
                                            .grid()
                                            .grid_rows(rows)
                                            .grid_cols(cols)
                                            .m(px(16.))
                                            .gap(px(16.)),
                                        |david, call_member| {
                                            david.child(CallMemberDisplay {
                                                call: self.call.clone(),
                                                call_member: call_member.clone(),
                                            })
                                        },
                                    )
                                    .into_any_element(),
                                CallState::Ended => div().flex_grow().into_any_element(),
                                CallState::Error(error) => interstitial()
                                    .flex_grow()
                                    .icon("call-start".into())
                                    .title(
                                        tr!("CALL_CONNECTION_ERROR", "Unable to connect the call")
                                            .into(),
                                    )
                                    .message(error.to_string().into())
                                    .into_any_element(),
                            })
                        },
                    )
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
                                            button("camera")
                                                .p(px(16.))
                                                .child(icon("camera-photo".into()).size(24.))
                                                .checked_when(call.active_camera().is_some())
                                                .on_click(cx.listener(
                                                    move |this, _, window, cx| {
                                                        let call = this.call.read(cx);
                                                        if call.active_camera().is_some() {
                                                            this.call.update(cx, |call, cx| {
                                                                call.set_active_camera(None, cx)
                                                            });
                                                        } else {
                                                            let call = this.call.clone();
                                                            this.webcam_start_dialog.update(
                                                                cx,
                                                                |webcam_start_dialog, cx| {
                                                                    webcam_start_dialog
                                                                        .open(call, window, cx)
                                                                },
                                                            )
                                                        }
                                                    },
                                                )),
                                        )
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
            .child(self.webcam_start_dialog.clone())
    }
}

#[derive(IntoElement)]
struct CallMemberDisplay {
    call: Entity<LivekitCall>,
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

        let camera_sid = match call_member.camera_state {
            StreamState::On(sid) => self.call.read(cx).video_stream_images.get(&sid),
            _ => None,
        };
        let screenshare_sid = match call_member.screenshare_state {
            StreamState::On(sid) => self.call.read(cx).video_stream_images.get(&sid),
            _ => None,
        };

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
                if let Some(camera_frame) = screenshare_sid.or(camera_sid) {
                    div().flex().size_full().overflow_hidden().child(
                        img(camera_frame.clone())
                            .object_fit(ObjectFit::Contain)
                            .size_full(),
                    )
                } else {
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
                }
                .when(screenshare_sid.is_some(), |david| {
                    david.when_some(camera_sid, |david, camera_frame| {
                        // TODO: Calculate aspect ratio and find out good size for the inside frame
                        let width = 300.;
                        let height = 200.;

                        david.child(
                            div()
                                .absolute()
                                .left_0()
                                .top_0()
                                .size_full()
                                .flex()
                                .items_start()
                                .justify_end()
                                .p(px(16.))
                                .child(
                                    layer()
                                        .flex()
                                        .border(px(1.))
                                        .border_color(theme.border_color)
                                        .w(px(width))
                                        .h(px(height))
                                        .child(
                                            img(camera_frame.clone())
                                                .object_fit(ObjectFit::Contain)
                                                .size_full(),
                                        ),
                                ),
                        )
                    })
                })
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
