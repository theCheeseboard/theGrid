use crate::CallState;
use crate::call_manager::LivekitCallManager;
use cntp_i18n::tr;
use contemporary::components::admonition::{AdmonitionSeverity, admonition};
use contemporary::components::button::button;
use contemporary::components::icon_text::icon_text;
use gpui::prelude::FluentBuilder;
use gpui::{App, IntoElement, ParentElement, RenderOnce, Styled, Window, div, px};
use thegrid_common::session::session_manager::SessionManager;

#[derive(IntoElement)]
pub struct ActiveCallSidebarAlert {}

pub fn active_call_sidebar_alert() -> ActiveCallSidebarAlert {
    ActiveCallSidebarAlert {}
}

impl RenderOnce for ActiveCallSidebarAlert {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let call_manager = cx.global::<LivekitCallManager>();
        let call = call_manager.current_call().unwrap().read(cx);

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
                    .when_some(call_error, |david, err| {
                        david.child(icon_text("exception".into(), err.to_string().into()))
                    })
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
                            }),
                    ),
            )
    }
}
