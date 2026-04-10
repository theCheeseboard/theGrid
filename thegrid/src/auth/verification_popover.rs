mod verification_reciporicate_page;
mod verification_sas_page;
mod verification_select_page;

use crate::auth::verification_popover::verification_reciporicate_page::VerificationReciporicatePage;
use crate::auth::verification_popover::verification_sas_page::VerificationSasPage;
use crate::auth::verification_popover::verification_select_page::VerificationSelectPage;
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
use gpui::prelude::FluentBuilder;
use gpui::{
    div, px, App, AppContext, AsyncApp, ClickEvent, Context, Element, Entity,
    IntoElement, ParentElement, Render, Styled, WeakEntity, Window,
};
use matrix_sdk::encryption::identities::Device;
use matrix_sdk::encryption::verification::VerificationRequestState;
use matrix_sdk::encryption::VerificationState;
use matrix_sdk::ruma::events::key::verification::cancel::CancelCode;
use matrix_sdk_crypto::{CancelInfo, QrVerificationState};
use std::rc::Rc;
use thegrid_common::sas_emoji::SasEmoji;
use thegrid_common::session::session_manager::SessionManager;
use thegrid_common::session::verification_requests_cache::{
    VerificationRequestDetails, SUPPORTED_VERIFICATION_METHODS,
};
use thegrid_common::tokio_helper::TokioHelper;

pub struct VerificationPopover {
    state: VerificationPopoverState,
}

enum VerificationPopoverState {
    Idle,
    RequestingVerification,
    ActiveVerification(Entity<VerificationRequestDetails>),
}

impl VerificationPopover {
    pub fn new(cx: &mut Context<VerificationPopover>) -> VerificationPopover {
        VerificationPopover {
            state: VerificationPopoverState::Idle,
        }
    }

    pub fn set_verification_request(
        &mut self,
        verification_request: Entity<VerificationRequestDetails>,
        cx: &mut Context<VerificationPopover>,
    ) {
        self.state = VerificationPopoverState::ActiveVerification(verification_request);
    }

    pub fn trigger_outgoing_verification(&mut self, cx: &mut Context<VerificationPopover>) {
        self.state = VerificationPopoverState::RequestingVerification;

        let session_manager = cx.global::<SessionManager>();
        let client = session_manager.client().unwrap().read(cx).clone();
        let user_id = client.user_id().unwrap().to_owned();
        let verification_requests = session_manager.verification_requests();

        cx.spawn({
            let verification_requests = verification_requests.downgrade();
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
                                .request_verification_with_methods(
                                    SUPPORTED_VERIFICATION_METHODS.to_vec(),
                                )
                                .await
                        })
                        .await
                    {
                        let verification_request_clone = verification_request.clone();
                        let Ok(verification_request) =
                            verification_requests.update(cx, |requests, cx| {
                                requests
                                    .notify_new_verification_request(verification_request_clone, cx)
                            })
                        else {
                            return;
                        };

                        let _ = weak_this.update(cx, |this, cx| {
                            this.state =
                                VerificationPopoverState::ActiveVerification(verification_request);
                            cx.notify()
                        });
                    }
                };
            }
        })
        .detach();

        cx.notify()
    }

    pub fn request_device_verification(
        &mut self,
        device: Device,
        cx: &mut Context<VerificationPopover>,
    ) {
        self.state = VerificationPopoverState::RequestingVerification;

        let session_manager = cx.global::<SessionManager>();
        let verification_requests = session_manager.verification_requests();

        cx.spawn(
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                if let Ok(verification_request) = cx
                    .spawn_tokio(async move {
                        device
                            .request_verification_with_methods(
                                SUPPORTED_VERIFICATION_METHODS.to_vec(),
                            )
                            .await
                    })
                    .await
                {
                    let verification_request_clone = verification_request.clone();
                    let verification_request = verification_requests.update(cx, |requests, cx| {
                        requests.notify_new_verification_request(verification_request_clone, cx)
                    });
                    let _ = weak_this.update(cx, |this, cx| {
                        this.state =
                            VerificationPopoverState::ActiveVerification(verification_request);
                        cx.notify()
                    });
                }
            },
        )
        .detach();

        cx.notify()
    }

    pub fn clear_verification_request(&mut self) {
        self.state = VerificationPopoverState::Idle;
    }

    pub fn on_back_click(&mut self, cx: &mut Context<VerificationPopover>) {
        if let Some(verification_request) = self.verification_request(cx) {
            verification_request.update(cx, |verification_request, cx| {
                if !verification_request.inner.is_done()
                    && !verification_request.inner.is_cancelled()
                {
                    verification_request.cancel(cx);
                }
            });
            self.state = VerificationPopoverState::Idle;
        }

        cx.notify()
    }

    fn verification_request(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<Entity<VerificationRequestDetails>> {
        let session_manager = cx.global::<SessionManager>();
        let verification_requests = session_manager.verification_requests().read(cx);

        match &self.state {
            VerificationPopoverState::ActiveVerification(verification_request) => {
                Some(verification_request.clone())
            }
            _ => None,
        }
    }
}

enum VerificationPopoverPage {
    Loading = 0,
    Completed = 1,
    SelectMethod = 2,
    Sas = 3,
    QrReciporicate = 4,
    Cancelled = 5,
    AwaitingOtherDevice = 6,
}

impl Render for VerificationPopover {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let session_manager = cx.global::<SessionManager>();
        let account = session_manager.current_account().read(cx);
        let verified = account.verification_state() == VerificationState::Verified;

        let on_back_click: Rc<Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>> = Rc::new(Box::new(
            cx.listener(move |this, _, _, cx| this.on_back_click(cx)),
        ));

        let verification_request_entity = self.verification_request(cx);
        let verification_request = verification_request_entity
            .as_ref()
            .map(|verification_request| verification_request.read(cx).clone());

        popover("verification-popover")
            .visible(matches!(
                self.state,
                VerificationPopoverState::RequestingVerification
                    | VerificationPopoverState::ActiveVerification(_)
            ))
            .size_neg(100.)
            .anchor_bottom()
            .content(
                pager(
                    "verification-popover-pager",
                    match &verification_request {
                        None => VerificationPopoverPage::Loading,
                        Some(verification_request) => match verification_request.inner.state() {
                            VerificationRequestState::Created { .. } => {
                                VerificationPopoverPage::AwaitingOtherDevice
                            }
                            VerificationRequestState::Requested { .. } => {
                                if verification_request.inner.we_started() {
                                    VerificationPopoverPage::AwaitingOtherDevice
                                } else {
                                    VerificationPopoverPage::Loading
                                }
                            }
                            VerificationRequestState::Ready { .. } => {
                                VerificationPopoverPage::SelectMethod
                            }
                            VerificationRequestState::Transitioned { .. } => {
                                match &verification_request.sas_state {
                                    Some(sas_state)
                                        if !sas_state.is_done()
                                            && !sas_state.is_cancelled()
                                            && sas_state.emoji().is_some() =>
                                    {
                                        VerificationPopoverPage::Sas
                                    }
                                    _ => {
                                        if verification_request.sas_manually_started {
                                            VerificationPopoverPage::Loading
                                        } else {
                                            match &verification_request.qr_state {
                                                Some(qr_state)
                                                    if !matches!(
                                                        qr_state.state(),
                                                        QrVerificationState::Done { .. }
                                                            | QrVerificationState::Cancelled(..)
                                                            | QrVerificationState::Confirmed
                                                    ) =>
                                                {
                                                    if qr_state.has_been_scanned() {
                                                        VerificationPopoverPage::QrReciporicate
                                                    } else {
                                                        VerificationPopoverPage::SelectMethod
                                                    }
                                                }
                                                _ => VerificationPopoverPage::Loading,
                                            }
                                        }
                                    }
                                }
                            }
                            VerificationRequestState::Done => VerificationPopoverPage::Completed,
                            VerificationRequestState::Cancelled(_) => {
                                VerificationPopoverPage::Cancelled
                            }
                        },
                    } as usize,
                )
                .animation(FadeAnimation::new())
                .size_full()
                .page(
                    div()
                        .flex()
                        .flex_col()
                        .size_full()
                        .gap(px(9.))
                        .child(
                            grandstand("verify-popover-grandstand")
                                .text(tr!("POPOVER_VERIFY", "Verification"))
                                .on_back_click({
                                    let on_back_click = on_back_click.clone();
                                    move |event, window, cx| on_back_click(event, window, cx)
                                }),
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
                                .text(tr!("POPOVER_VERIFY"))
                                .on_back_click({
                                    let on_back_click = on_back_click.clone();
                                    move |event, window, cx| on_back_click(event, window, cx)
                                }),
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
                                                        this.clear_verification_request();
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
                    VerificationSelectPage {
                        verification_request: verification_request_entity.clone(),
                        on_back_click: on_back_click.clone(),
                    }
                    .into_any_element(),
                )
                .page(
                    VerificationSasPage {
                        verification_request: verification_request_entity.clone(),
                        on_back_click: on_back_click.clone(),
                    }
                    .into_any_element(),
                )
                .page(
                    VerificationReciporicatePage {
                        verification_request: verification_request_entity.clone(),
                        on_back_click: on_back_click.clone(),
                    }
                    .into_any_element(),
                )
                .page(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(9.))
                        .child(
                            grandstand("verify-popover-grandstand")
                                .text(tr!("POPOVER_VERIFY"))
                                .on_back_click({
                                    let on_back_click = on_back_click.clone();
                                    move |event, window, cx| on_back_click(event, window, cx)
                                }),
                        )
                        .child({
                            let reason = verification_request
                                .as_ref()
                                .and_then(|verification_request| {
                                    verification_request.inner.cancel_info()
                                })
                                .map(|cancel_info| cancel_string(&cancel_info))
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
                                                    tr!("CLOSE").into(),
                                                ))
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.clear_verification_request();
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
                                .text(tr!("POPOVER_VERIFY"))
                                .on_back_click({
                                    let on_back_click = on_back_click.clone();
                                    move |event, window, cx| on_back_click(event, window, cx)
                                }),
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
                                            .when_else(
                                                verified,
                                                |david| {
                                                    david.child(tr!(
                                                        "VERIFICATION_POPOVER_AWAITING_OK_TEXT",
                                                        "We sent a verification request to that \
                                                        device. Go ahead and accept it on that \
                                                        device to continue."
                                                    ))
                                                },
                                                |david| {
                                                    david.child(tr!(
                                                        "VERIFICATION_POPOVER_AWAITING_OK_US_TEXT",
                                                        "We sent a verification request to all \
                                                        of your other devices. Go ahead and accept \
                                                        the verification request on one of your \
                                                        other verified devices to continue."
                                                    ))
                                                },
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .gap(px(8.))
                                                    .items_center()
                                                    .child(spinner().size(px(16.)))
                                                    .child(tr!(
                                                        "VERIFICATION_POPOVER_AWAITING_OK_SPINNER",
                                                        "Waiting for other device to respond..."
                                                    )),
                                            ),
                                    )
                                    .into_any_element(),
                            ),
                        )
                        .into_any_element(),
                ),
            )
    }
}

fn cancel_string(cancel_info: &CancelInfo) -> String {
    match cancel_info.cancel_code() {
        CancelCode::User => if cancel_info.cancelled_by_us() {
            tr!(
                "VERIFICATION_CANCEL_REASON_USER_IS",
                "Verification was cancelled by this device."
            )
        } else {
            tr!(
                "VERIFICATION_CANCEL_REASON_USER",
                "Verification was cancelled from the other device."
            )
        }
        .to_string(),
        CancelCode::Timeout => tr!(
            "VERIFICATION_CANCEL_REASON_TIMEOUT",
            "Verification failed because the verification process took too long to complete."
        )
        .to_string(),
        CancelCode::UnknownMethod => tr!(
            "VERIFICATION_CANCEL_REASON_UNKNOWN_METHOD",
            "Verification failed because the negotiated verification method is not supported."
        )
        .to_string(),
        CancelCode::UnexpectedMessage => tr!(
            "VERIFICATION_CANCEL_UNEXPECTED_MESSAGE",
            "Verification failed because an unexpected message was received."
        )
        .to_string(),
        CancelCode::Accepted => tr!(
            "VERIFICATION_CANCEL_REASON_ACCEPTED",
            "The verification request was accepted on a different device."
        )
        .to_string(),
        CancelCode::MismatchedSas => tr!(
            "VERIFICATION_CANCEL_REASON_MISMATCHED_SAS",
            "Verification failed because the displayed emoji could not be confirmed on \
                both devices."
        )
        .to_string(),
        _ => cancel_info.reason().to_string(),
    }
}
