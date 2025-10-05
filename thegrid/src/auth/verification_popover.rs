use cntp_i18n::tr;
use contemporary::components::button::button;
use contemporary::components::constrainer::constrainer;
use contemporary::components::grandstand::grandstand;
use contemporary::components::icon_text::icon_text;
use contemporary::components::layer::layer;
use contemporary::components::popover::popover;
use contemporary::components::spinner::spinner;
use contemporary::components::subtitle::subtitle;
use gpui::http_client::anyhow;
use gpui::prelude::FluentBuilder;
use gpui::{
    App, AppContext, AsyncApp, Context, ElementId, IntoElement, ParentElement, Render, Styled,
    Window, div, px,
};
use gpui_tokio::Tokio;
use matrix_sdk::encryption::verification::VerificationRequest;
use matrix_sdk::ruma::api::client::session::get_login_types::v3::LoginType;
use thegrid::session::session_manager::SessionManager;
use thegrid::session::verification_requests_cache::VerificationRequestDetails;

pub struct VerificationPopover {
    verification_request: Option<String>,
}

impl VerificationPopover {
    pub fn new(cx: &mut Context<VerificationPopover>) -> VerificationPopover {
        VerificationPopover {
            verification_request: None,
        }
    }

    pub fn set_verification_request(
        &mut self,
        verification_request: VerificationRequestDetails,
        cx: &mut Context<VerificationPopover>,
    ) {
        self.verification_request = Some(verification_request.inner.flow_id().to_string());
    }

    pub fn clear_verification_request(&mut self) {
        self.verification_request = None;
    }
}

impl Render for VerificationPopover {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let session_manager = cx.global::<SessionManager>();
        let verification_request = session_manager
            .verification_requests()
            .read(cx)
            .verification_request(
                self.verification_request
                    .clone()
                    .unwrap_or_default()
                    .as_str(),
            )
            .cloned();
        let verification_request_clone = verification_request.clone();

        popover("verification-popover")
            .visible(self.verification_request.is_some())
            .size_neg(100.)
            .anchor_bottom()
            .content(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(9.))
                    .child(
                        grandstand("verify-popover-grandstand")
                            .text(tr!("POPOVER_VERIFY", "Verification"))
                            .on_back_click(cx.listener(move |this, _, _, cx| {
                                if let Some(verification_request) =
                                    verification_request_clone.clone()
                                {
                                    if !verification_request.inner.is_done()
                                        && !verification_request.inner.is_cancelled()
                                    {
                                        cx.spawn(async move |_, cx: &mut AsyncApp| {
                                            Tokio::spawn(cx, async move {
                                                verification_request
                                                    .clone()
                                                    .inner
                                                    .cancel()
                                                    .await
                                                    .map_err(|e| anyhow!(e))
                                            })
                                            .unwrap()
                                            .await
                                        })
                                        .detach();
                                    }
                                }

                                this.verification_request = None;
                                cx.notify()
                            })),
                    )
                    .when_some(verification_request, |david, verification_request| {
                        if verification_request.inner.is_done() {
                            david.child(
                                constrainer("verify-popover-constrainer").child(
                                    layer()
                                        .flex()
                                        .flex_col()
                                        .p(px(8.))
                                        .w_full()
                                        .child(subtitle(tr!(
                                            "VERIFICATION_POPOVER_OK",
                                            "Verification completed"
                                        )))
                                        .child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .gap(px(8.))
                                                .child(tr!(
                                                    "VERIFICATION_POPOVER_OK_MESSAGE",
                                                    "Good stuff."
                                                ))
                                                .child(
                                                    button("verification-popover-ok")
                                                        .child(icon_text(
                                                            "dialog-ok".into(),
                                                            tr!("CLOSE", "Close").into(),
                                                        ))
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            this.verification_request = None;
                                                            cx.notify()
                                                        })),
                                                ),
                                        )
                                        .into_any_element(),
                                ),
                            )
                        } else if verification_request.inner.is_cancelled() {
                            let reason = verification_request.inner.cancel_info().unwrap().reason();
                            david.child(
                                constrainer("verify-popover-constrainer").child(
                                    layer()
                                        .flex()
                                        .flex_col()
                                        .p(px(8.))
                                        .w_full()
                                        .child(subtitle(tr!(
                                            "VERIFICATION_POPOVER_CANCELLED",
                                            "Verification cancelled"
                                        )))
                                        .child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .gap(px(8.))
                                                .child(reason)
                                                .child(
                                                    button("verification-popover-ok")
                                                        .child(icon_text(
                                                            "dialog-ok".into(),
                                                            tr!("CLOSE", "Close").into(),
                                                        ))
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            this.verification_request = None;
                                                            cx.notify()
                                                        })),
                                                ),
                                        )
                                        .into_any_element(),
                                ),
                            )
                        } else if let Some(sas_state) = verification_request.sas_state
                            && !sas_state.is_done()
                            && !sas_state.is_cancelled()
                            && let Some(emoji) = sas_state.emoji()
                        {
                            let sas_state_clone = sas_state.clone();
                            let sas_state_clone_2 = sas_state.clone();
                            david.child(
                                constrainer("verify-popover-constrainer").child(
                                    layer()
                                        .flex()
                                        .flex_col()
                                        .p(px(8.))
                                        .w_full()
                                        .child(subtitle(tr!(
                                            "VERIFICATION_SAS_EMOJI",
                                            "Compare these emoji"
                                        )))
                                        .child(tr!(
                                            "VERIFICATION_SAS_EMOJI_DESCRIPTION",
                                            "Check on the other device and ensure that these emoji \
                                            are displayed, in the same order."
                                        ))
                                        .child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .gap(px(8.))
                                                .child(emoji.iter().fold(
                                                    div().flex().flex_col(),
                                                    |david, emoji| {
                                                        david.child(format!(
                                                            "{} {}",
                                                            emoji.symbol, emoji.description
                                                        ))
                                                    },
                                                ))
                                                .child(
                                                    button("verification-popover-ok")
                                                        .child(icon_text(
                                                            "dialog-ok".into(),
                                                            tr!("EMOJI_MATCH", "The emoji match")
                                                                .into(),
                                                        ))
                                                        .on_click(cx.listener(
                                                            move |this, _, _, cx| {
                                                                let sas_state =
                                                                    sas_state_clone.clone();
                                                                cx.spawn(async move |_, cx| {
                                                                    Tokio::spawn_result(
                                                                        cx,
                                                                        async move {
                                                                            sas_state
                                                                                .confirm()
                                                                                .await
                                                                                .map_err(|e| {
                                                                                    anyhow!(e)
                                                                                })
                                                                        },
                                                                    )
                                                                    .unwrap()
                                                                    .await
                                                                })
                                                                .detach();
                                                                cx.notify()
                                                            },
                                                        )),
                                                )
                                                .child(
                                                    button("verification-popover-not-ok")
                                                        .child(icon_text(
                                                            "dialog-cancel".into(),
                                                            tr!(
                                                                "EMOJI_NO_MATCH",
                                                                "The emoji do not match"
                                                            )
                                                            .into(),
                                                        ))
                                                        .on_click(cx.listener(
                                                            move |_, _, _, cx| {
                                                                let sas_state =
                                                                    sas_state_clone_2.clone();
                                                                cx.spawn(async move |_, cx| {
                                                                    Tokio::spawn_result(
                                                                        cx,
                                                                        async move {
                                                                            sas_state
                                                                                .mismatch()
                                                                                .await
                                                                                .map_err(|e| {
                                                                                    anyhow!(e)
                                                                                })
                                                                        },
                                                                    )
                                                                    .unwrap()
                                                                    .await
                                                                })
                                                                .detach();
                                                                cx.notify()
                                                            },
                                                        )),
                                                ),
                                        )
                                        .into_any_element(),
                                ),
                            )
                        } else {
                            david.child(
                                div()
                                    .size_full()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(spinner()),
                            )
                        }
                    }),
            )
    }
}
