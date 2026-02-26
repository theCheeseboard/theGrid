use crate::call_manager::LivekitCallManager;
use crate::{CallMember, CallState, StreamState};
use cntp_i18n::tr;
use contemporary::components::admonition::{AdmonitionSeverity, admonition};
use contemporary::components::button::button;
use contemporary::components::icon::icon;
use contemporary::components::icon_text::icon_text;
use contemporary::styling::theme::ThemeStorage;
use gpui::prelude::FluentBuilder;
use gpui::{App, IntoElement, ParentElement, Render, RenderOnce, Styled, Window, div, px};
use matrix_sdk::room::RoomMember;
use thegrid_common::mxc_image::{SizePolicy, mxc_image};
use thegrid_common::session::session_manager::SessionManager;

#[derive(IntoElement)]
pub struct ActiveCallSidebarAlert {}

pub fn active_call_sidebar_alert() -> ActiveCallSidebarAlert {
    ActiveCallSidebarAlert {}
}

impl RenderOnce for ActiveCallSidebarAlert {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let call_manager = cx.global::<LivekitCallManager>();
        let mute = call_manager.mute();

        let call = call_manager.current_call().unwrap().clone().read(cx);
        let call_members = call.call_members().read(cx);

        let theme = cx.theme();

        let session_manager = cx.global::<SessionManager>();
        let room = session_manager
            .rooms()
            .read(cx)
            .room(&call.room)
            .unwrap()
            .read(cx);

        let call_error = match call.state {
            CallState::Connecting => None,
            CallState::Active { .. } => None,
            CallState::Ended => None,
            CallState::Error(error) => Some(error),
        };

        admonition()
            .title(tr!("ACTIVE_CALL", "Active Call"))
            .severity(if call_error.is_some() {
                AdmonitionSeverity::Error
            } else {
                AdmonitionSeverity::Info
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .when(matches!(call.state, CallState::Connecting), |david| {
                        david.child(format!(
                            "{} - {}",
                            room.display_name(),
                            tr!("CALL_CONNECTING", "Connecting...")
                        ))
                    })
                    .when(matches!(call.state, CallState::Active { .. }), |david| {
                        let secs = call.started_at.elapsed().as_secs();
                        let mins = secs / 60;
                        let secs = secs % 60;
                        let hours = mins / 60;
                        let mins = mins % 60;

                        david.child(format!(
                            "{} - {:02}:{:02}:{:02}",
                            room.display_name(),
                            hours,
                            mins,
                            secs
                        ))
                    })
                    .child(call_members.iter().fold(
                        div().flex().flex_col(),
                        |david, call_member| match call.get_cached_room_user(&call_member.user_id) {
                            None => david,
                            Some(room_member) => david.child(CallMemberState {
                                room_member,
                                call_member: call_member.clone(),
                            }),
                        },
                    ))
                    .when_some(call_error, |david, err| {
                        david.child(icon_text("exception".into(), err.to_string().into()))
                    })
                    .child(
                        div()
                            .flex()
                            .bg(theme.button_background)
                            .rounded(theme.border_radius)
                            .child(
                                button("mute")
                                    .child(icon(
                                        if *mute.read(cx) { "mic-off" } else { "mic-on" }.into(),
                                    ))
                                    .checked_when(*mute.read(cx))
                                    .on_click(move |_, _, cx| {
                                        let muted = *mute.read(cx);
                                        mute.write(cx, !muted);
                                    })
                                    .flex_grow(),
                            )
                            .child(
                                button("call-end")
                                    .destructive()
                                    .child(icon_text(
                                        "call-stop".into(),
                                        tr!("CALL_HANG_UP", "Hang Up").into(),
                                    ))
                                    .on_click(|_, _, cx| {
                                        let call_manager = cx.global::<LivekitCallManager>();
                                        call_manager
                                            .current_call()
                                            .unwrap()
                                            .update(cx, |call, cx| call.end_call(cx))
                                    })
                                    .flex_grow(),
                            ),
                    ),
            )
    }
}

#[derive(IntoElement)]
struct CallMemberState {
    room_member: RoomMember,
    call_member: CallMember,
}

impl RenderOnce for CallMemberState {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();

        let member_is_connecting = self.call_member.mic_state == StreamState::Unavailable;

        div()
            .flex()
            .items_center()
            .gap(px(2.))
            .when(member_is_connecting, |david| david.opacity(0.5))
            .child(
                mxc_image(self.room_member.avatar_url().map(|url| url.to_owned()))
                    .size(px(16.))
                    .size_policy(SizePolicy::Fit)
                    .rounded(theme.border_radius),
            )
            .child(
                self.room_member
                    .display_name()
                    .unwrap_or_default()
                    .to_string(),
            )
            .child(div().flex_grow())
            .when(
                self.call_member.screenshare_state == StreamState::On,
                |david| david.child(icon("video-display".into())),
            )
            .when(self.call_member.camera_state == StreamState::On, |david| {
                david.child(icon("camera-photo".into()))
            })
            .when(self.call_member.mic_state == StreamState::Off, |david| {
                david.child(icon("mic-off".into()))
            })
    }
}
