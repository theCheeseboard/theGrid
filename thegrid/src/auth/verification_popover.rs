use cntp_i18n::tr;
use contemporary::components::button::button;
use contemporary::components::constrainer::constrainer;
use contemporary::components::grandstand::grandstand;
use contemporary::components::icon_text::icon_text;
use contemporary::components::layer::layer;
use contemporary::components::pager::fade_animation::FadeAnimation;
use contemporary::components::pager::pager;
use contemporary::components::popover::popover;
use contemporary::components::spinner::spinner;
use contemporary::components::subtitle::subtitle;
use gpui::http_client::anyhow;
use gpui::prelude::FluentBuilder;
use gpui::{
    AppContext, AsyncApp, Context, Flatten, IntoElement, ParentElement, Render, Styled, WeakEntity,
    Window, div, px,
};
use gpui_tokio::Tokio;
use matrix_sdk::encryption::verification::VerificationRequestState;
use matrix_sdk::ruma::events::key::verification::VerificationMethod;
use thegrid::session::session_manager::SessionManager;
use thegrid::session::verification_requests_cache::VerificationRequestDetails;
use thegrid::tokio_helper::TokioHelper;

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

    pub fn trigger_outgoing_verification(&mut self, cx: &mut Context<VerificationPopover>) {
        self.verification_request = Some(String::default());

        let session_manager = cx.global::<SessionManager>();
        let client = session_manager.client().unwrap().read(cx).clone();
        let user_id = client.user_id().unwrap().to_owned();
        let verification_requests = session_manager.verification_requests();

        cx.spawn(
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                if let Some(identity) = cx
                    .spawn_tokio(async move {
                        client.encryption().request_user_identity(&user_id).await
                    })
                    .await
                    .ok()
                    .flatten()
                {
                    if let Ok(verification_request) = cx
                        .spawn_tokio(async move {
                            identity
                                .request_verification_with_methods(vec![VerificationMethod::SasV1])
                                .await
                        })
                        .await
                    {
                        let verification_request_clone = verification_request.clone();
                        let _ = verification_requests.update(cx, |requests, cx| {
                            requests
                                .notify_new_verification_request(verification_request_clone, cx);
                        });
                        let _ = weak_this.update(cx, |this, cx| {
                            this.verification_request =
                                Some(verification_request.flow_id().to_string());
                            cx.notify()
                        });
                    }
                };
            },
        )
        .detach();

        cx.notify()
    }

    pub fn clear_verification_request(&mut self) {
        self.verification_request = None;
    }

    pub fn on_back_click(&mut self, cx: &mut Context<VerificationPopover>) {
        let session_manager = cx.global::<SessionManager>();
        let verification_requests = session_manager.verification_requests().read(cx);

        if let Some(verification_request) = &self.verification_request {
            let verification_request = verification_requests
                .verification_request(verification_request.as_str())
                .cloned();

            if let Some(verification_request) = verification_request.clone() {
                if !verification_request.inner.is_done()
                    && !verification_request.inner.is_cancelled()
                {
                    cx.spawn(async move |_, cx: &mut AsyncApp| {
                        let _ = cx
                            .spawn_tokio(async move {
                                verification_request.clone().inner.cancel().await
                            })
                            .await;
                    })
                    .detach();
                }
            }
            self.verification_request = None;
        }

        cx.notify()
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

        popover("verification-popover")
            .visible(self.verification_request.is_some())
            .size_neg(100.)
            .anchor_bottom()
            .content(
                pager(
                    "verification-popover-pager",
                    match &verification_request {
                        None => 0,
                        Some(verification_request) => match verification_request.inner.state() {
                            VerificationRequestState::Created { .. } => 4,
                            VerificationRequestState::Requested { .. } => {
                                if verification_request.inner.we_started() {
                                    4
                                } else {
                                    0
                                }
                            }
                            VerificationRequestState::Ready { .. } => 0,
                            VerificationRequestState::Transitioned { .. } => {
                                match &verification_request.sas_state {
                                    Some(sas_state)
                                        if !sas_state.is_done()
                                            && !sas_state.is_cancelled()
                                            && sas_state.emoji().is_some() =>
                                    {
                                        2
                                    }
                                    _ => 0,
                                }
                            }
                            VerificationRequestState::Done => 1,
                            VerificationRequestState::Cancelled(_) => 3,
                        },
                    },
                )
                .animation(FadeAnimation::new())
                .page(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(9.))
                        .child(
                            grandstand("verify-popover-grandstand")
                                .text(tr!("POPOVER_VERIFY", "Verification"))
                                .on_back_click(
                                    cx.listener(move |this, _, _, cx| this.on_back_click(cx)),
                                ),
                        )
                        .child(
                            div()
                                .size_full()
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(spinner()),
                        )
                        .into_any_element(),
                )
                .page(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(9.))
                        .child(
                            grandstand("verify-popover-grandstand")
                                .text(tr!("POPOVER_VERIFY", "Verification"))
                                .on_back_click(
                                    cx.listener(move |this, _, _, cx| this.on_back_click(cx)),
                                ),
                        )
                        .child(
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
                                                "Your device is now verified, and encryption \
                                                     keys have been shared."
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
                        .into_any_element(),
                )
                .page(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(9.))
                        .child(
                            grandstand("verify-popover-grandstand")
                                .text(tr!("POPOVER_VERIFY", "Verification"))
                                .on_back_click(
                                    cx.listener(move |this, _, _, cx| this.on_back_click(cx)),
                                ),
                        )
                        .child({
                            if let Some(verification_request) = &verification_request
                                && let Some(sas_state) = verification_request.sas_state.as_ref()
                                && let Some(emoji) = sas_state.emoji()
                            {
                                let sas_state_clone = sas_state.clone();
                                let sas_state_clone_2 = sas_state.clone();
                                constrainer("verify-popover-constrainer")
                                    .child(
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
                                                "Check on the other device and ensure that these \
                                                emoji are displayed, in the same order."
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
                                                                tr!(
                                                                    "EMOJI_MATCH",
                                                                    "The emoji match"
                                                                )
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
                                    )
                                    .into_any_element()
                            } else {
                                div().into_any_element()
                            }
                        })
                        .into_any_element(),
                )
                .page(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(9.))
                        .child(
                            grandstand("verify-popover-grandstand")
                                .text(tr!("POPOVER_VERIFY", "Verification"))
                                .on_back_click(
                                    cx.listener(move |this, _, _, cx| this.on_back_click(cx)),
                                ),
                        )
                        .child({
                            let reason = verification_request
                                .as_ref()
                                .and_then(|verification_request| {
                                    verification_request.inner.cancel_info()
                                })
                                .map(|cancel_info| cancel_info.reason().to_string())
                                .unwrap_or_default();

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
                                        div().flex().flex_col().gap(px(8.)).child(reason).child(
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
                            )
                        })
                        .into_any_element(),
                )
                .page(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(9.))
                        .child(
                            grandstand("verify-popover-grandstand")
                                .text(tr!("POPOVER_VERIFY", "Verification"))
                                .on_back_click(
                                    cx.listener(move |this, _, _, cx| this.on_back_click(cx)),
                                ),
                        )
                        .child(
                            constrainer("verify-popover-constrainer").child(
                                layer()
                                    .flex()
                                    .flex_col()
                                    .p(px(8.))
                                    .w_full()
                                    .child(subtitle(tr!(
                                        "VERIFICATION_POPOVER_AWAITING_OK",
                                        "Verification Request Sent"
                                    )))
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap(px(8.))
                                            .child(tr!(
                                                "VERIFICATION_POPOVER_AWAITING_OK_TEXT",
                                                "We've sent a verification request to all of your \
                                                 other devices. Go ahead and accept the \
                                                 verification request on one of your other verified \
                                                 devices to continue."
                                            ))
                                            .child(div().flex().gap(px(8.)).items_center().child(spinner().size(px(16.))).child(
                                                tr!(
                                                    "VERIFICATION_POPOVER_AWAITING_OK_SPINNER",
                                                    "Waiting for other device to respond..."
                                                ),
                                            )),
                                    )
                                    .into_any_element(),
                            ),
                        )
                        .into_any_element(),
                ),
            )
    }
}
