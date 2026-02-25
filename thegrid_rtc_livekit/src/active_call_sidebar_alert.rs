use crate::call_manager::LivekitCallManager;
use cntp_i18n::tr;
use contemporary::components::admonition::admonition;
use contemporary::components::button::button;
use contemporary::components::icon_text::icon_text;
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

        admonition().title(tr!("ACTIVE_CALL", "Active Call")).child(
            div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(room.display_name())
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
