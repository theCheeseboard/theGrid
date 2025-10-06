use cntp_i18n::tr;
use contemporary::application::Details;
use contemporary::components::button::button;
use contemporary::components::constrainer::constrainer;
use contemporary::components::grandstand::grandstand;
use contemporary::components::icon_text::icon_text;
use contemporary::components::layer::layer;
use contemporary::components::popover::popover;
use contemporary::components::spinner::spinner;
use contemporary::components::subtitle::subtitle;
use gpui::http_client::anyhow;
use gpui::private::anyhow;
use gpui::{
    App, AppContext, AsyncApp, BorrowAppContext, Element, Entity, IntoElement, ParentElement,
    RenderOnce, Styled, Window, div, px,
};
use gpui_tokio::Tokio;
use std::fs::remove_dir_all;
use thegrid::admonition::{AdmonitionSeverity, admonition};
use thegrid::session::session_manager::SessionManager;

enum LogoutPopoverState {
    Idle,
    Processing,
    Error,
}

#[derive(IntoElement)]
pub struct LogoutPopover {
    visible: Entity<bool>,
}

pub fn logout_popover(visible: Entity<bool>) -> LogoutPopover {
    LogoutPopover { visible }
}

impl LogoutPopover {
    pub fn perform_logout(
        visible: Entity<bool>,
        state: Entity<LogoutPopoverState>,
        window: &mut Window,
        cx: &mut App,
    ) {
        cx.update_global::<SessionManager, ()>(|session_manager, cx| {
            let client = session_manager.client().unwrap().read(cx).clone();

            let details = cx.global::<Details>();
            let directories = details.standard_dirs().unwrap();
            let data_dir = directories.data_dir();
            let session_dir = data_dir.join("sessions");
            let this_session_dir = session_dir.join(
                session_manager
                    .current_session()
                    .as_ref()
                    .unwrap()
                    .uuid
                    .to_string(),
            );

            state.write(cx, LogoutPopoverState::Processing);

            cx.spawn(async move |cx: &mut AsyncApp| {
                if let Err(e) =
                    Tokio::spawn_result(
                        cx,
                        async move { client.logout().await.map_err(|e| anyhow!(e)) },
                    )
                    .unwrap()
                    .await
                {
                    state.write(cx, LogoutPopoverState::Error).unwrap();
                    return;
                };

                // Delete the session
                remove_dir_all(this_session_dir).unwrap();

                cx.update_global::<SessionManager, ()>(|session_manager, cx| {
                    session_manager.clear_session();
                })
                .unwrap();

                cx.update_entity(&visible, |visible, _| *visible = false)
                    .unwrap();
                cx.update_entity(&state, |state, _| *state = LogoutPopoverState::Idle)
                    .unwrap();
            })
            .detach()
        });
    }
}

impl RenderOnce for LogoutPopover {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_state(cx, |_, _| LogoutPopoverState::Idle);
        let state_clone = state.clone();
        let visible = self.visible.clone();
        let visible_clone = visible.clone();

        popover("verification-popover")
            .visible(*self.visible.read(cx))
            .size_neg(100.)
            .anchor_bottom()
            .content(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(9.))
                    .child(
                        grandstand("logout-grandstand")
                            .text(tr!("LOG_OUT_TITLE", "Log Out"))
                            .on_back_click(move |_, _, cx| {
                                visible.write(cx, false);
                            }),
                    )
                    .child(match state.read(cx) {
                        LogoutPopoverState::Idle | LogoutPopoverState::Error => constrainer(
                            "logout-constrainer",
                        )
                        .child(
                            layer()
                                .flex()
                                .flex_col()
                                .p(px(8.))
                                .w_full()
                                .child(subtitle(tr!(
                                    "LOG_OUT_CONFIRMATION_TITLE",
                                    "Log out of your account"
                                )))
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap(px(8.))
                                        .child(tr!(
                                            "LOG_OUT_COMFIRMATION_MESSAGE",
                                            "Log out of your account?"
                                        ))
                                        .child(
                                            admonition()
                                                .severity(AdmonitionSeverity::Warning)
                                                .title(tr!("WARNING", "Warning"))
                                                .child(tr!(
                                                    "LOG_OUT_WARNING",
                                                    "If you're not logged in anywhere else, \
                                                    logging out now will cause you to lose all \
                                                    your encrypted messages."
                                                )),
                                        )
                                        .child(
                                            button("do-log-out")
                                                .child(icon_text(
                                                    "system-log-out".into(),
                                                    tr!("LOG_OUT", "Log out now").into(),
                                                ))
                                                .destructive()
                                                .on_click(move |_, window, cx| {
                                                    LogoutPopover::perform_logout(
                                                        visible_clone.clone(),
                                                        state_clone.clone(),
                                                        window,
                                                        cx,
                                                    );
                                                }),
                                        ),
                                )
                                .into_any_element(),
                        )
                        .into_any_element(),
                        LogoutPopoverState::Processing => div()
                            .size_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(spinner())
                            .into_any_element(),
                    }),
            )
    }
}
