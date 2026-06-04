use crate::auth::recovery_passphrase_popover::RecoveryPassphrasePopover;
use crate::auth::verification_popover::VerificationPopover;
use crate::chat::chat_surface::{
    RequestCryptographicResetEvent, RequestCryptographicResetHandler, SelfVerificationUi,
};
use cntp_i18n::tr;
use contemporary::components::button::button;
use contemporary::components::constrainer::constrainer;
use contemporary::components::dialog_box::{StandardButton, dialog_box};
use contemporary::components::grandstand::grandstand;
use contemporary::components::icon_text::icon_text;
use contemporary::components::layer::layer;
use contemporary::components::pager::fade_animation::FadeAnimation;
use contemporary::components::pager::pager;
use contemporary::components::subtitle::subtitle;
use contemporary::styling::theme::{ThemeStorage, VariableColor};
use gpui::prelude::FluentBuilder;
use gpui::{App, Entity, IntoElement, ParentElement, RenderOnce, Styled, Window, div, px};
use matrix_sdk::encryption::VerificationState;
use matrix_sdk::ruma::OwnedUserId;
use std::rc::Rc;
use thegrid_common::mxc_image::{SizePolicy, mxc_image};
use thegrid_common::session::session_manager::SessionManager;
use thegrid_common::surfaces::{MainWindowSurface, SurfaceChangeEvent, SurfaceChangeHandler};

#[derive(IntoElement)]
pub struct ForcedDeviceVerification {
    verification_ui: SelfVerificationUi,
}

pub fn forced_device_verification(verification_ui: SelfVerificationUi) -> ForcedDeviceVerification {
    ForcedDeviceVerification { verification_ui }
}

impl RenderOnce for ForcedDeviceVerification {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();

        let session_manager = cx.global::<SessionManager>();
        let client = session_manager.client().unwrap().read(cx);
        let account = session_manager.current_account().read(cx);
        let devices = session_manager.devices().read(cx);
        let device_list = devices.devices();
        let is_last_device = devices.is_last_device();

        let verification_requests = session_manager.verification_requests().read(cx);
        let shown_verification_requests: Vec<_> = verification_requests
            .pending_verification_requests
            .iter()
            .filter(|request| request.read(cx).is_active())
            .collect();

        let pending_verification = shown_verification_requests.first().cloned().cloned();
        let device_name = pending_verification
            .as_ref()
            .and_then(|verification| {
                verification
                    .read(cx)
                    .device_id
                    .as_ref()
                    .and_then(|device_id| {
                        device_list
                            .iter()
                            .find(|device| &device.inner.device_id == device_id)
                    })
            })
            .map(|device| {
                device
                    .inner
                    .display_name
                    .clone()
                    .map(|display_name| format!("{display_name} ({})", device.inner.device_id))
                    .unwrap_or_else(|| device.inner.device_id.to_string())
            })
            .unwrap_or_else(|| tr!("UNKNOWN_DEVICE", "Unknown Device").into());

        pager(
            "force-verification-pager",
            if let Some(verification) = pending_verification.as_ref()
                && !verification.read(cx).inner.we_started()
            {
                1
            } else {
                0
            },
        )
        .animation(FadeAnimation::new())
        .size_full()
        .page(
            div()
                .bg(theme.background)
                .w_full()
                .h_full()
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(
                    grandstand("force-verification-grandstand")
                        .text(tr!("FORCE_VERIFICATION_TITLE", "Verify this device"))
                        .pt(px(36.)),
                )
                .child(
                    constrainer("user-pane")
                        .flex()
                        .flex_col()
                        .w_full()
                        .px(px(8.))
                        .child(
                            layer()
                                .p(px(4.))
                                .flex()
                                .gap(px(4.))
                                .child(
                                    mxc_image(account.avatar_url())
                                        .fallback_image(client.user_id().unwrap())
                                        .rounded(theme.border_radius)
                                        .fixed_square(px(48.))
                                        .size_policy(SizePolicy::Fit),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .justify_center()
                                        .gap(px(4.))
                                        .child(account.display_name().unwrap_or_default())
                                        .child(
                                            div()
                                                .text_color(theme.foreground.disabled())
                                                .child(client.user_id().unwrap().to_string()),
                                        ),
                                ),
                        ),
                )
                .child(
                    constrainer("force-verification")
                        .flex()
                        .flex_col()
                        .w_full()
                        .px(px(8.))
                        .child(
                            layer()
                                .flex()
                                .flex_col()
                                .p(px(8.))
                                .gap(px(8.))
                                .w_full()
                                .child(subtitle(tr!("FORCE_VERIFICATION_TITLE")))
                                .child(tr!(
                                    "FORCE_VERIFICATION_PROMPT",
                                    "To proceed, you need to verify this device. Verification \
                                    ensures that you and the people that you talk to can be \
                                    certain that no one can intercept your messages, and that \
                                    you are really who you say you are."
                                ))
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .rounded(theme.border_radius)
                                        .bg(theme.button_background)
                                        .when(!is_last_device, |david| {
                                            david.child(
                                                button("verify-now")
                                                    .child(icon_text(
                                                        "edit-copy",
                                                        tr!("VERIFY_SESSION_OTHER_DEVICE"),
                                                    ))
                                                    .on_click({
                                                        let verification_popover = self
                                                            .verification_ui
                                                            .verification_popover
                                                            .clone();
                                                        move |_, _, cx| {
                                                            verification_popover.update(
                                                                cx,
                                                                |verification_popover, cx| {
                                                                    verification_popover
                                                                        .trigger_outgoing_verification(
                                                                            cx,
                                                                        )
                                                                },
                                                            );
                                                        }
                                                    }),
                                            )
                                        })
                                        .child(
                                            button("verify-recovery")
                                                .child(icon_text(
                                                    "visibility",
                                                    tr!("VERIFY_SESSION_RECOVERY_KEY"),
                                                ))
                                                .on_click({
                                                    let recovery_passphrase_popover = self
                                                        .verification_ui
                                                        .recovery_passphrase_popover
                                                        .clone();
                                                    move |_, _, cx| {
                                                        recovery_passphrase_popover.update(
                                                            cx,
                                                            |recovery_passphrase_popover, cx| {
                                                                recovery_passphrase_popover
                                                                    .set_visible(true);
                                                                cx.notify()
                                                            },
                                                        )
                                                    }
                                                }),
                                        )
                                        .child(
                                            button("reset-crypto")
                                                .destructive()
                                                .child(icon_text(
                                                    "help-contents",
                                                    tr!("VERIFY_SESSION_RESET_CRYPTO"),
                                                ))
                                                .on_click({
                                                    let on_request_cryptographic_reset = self
                                                        .verification_ui
                                                        .on_request_cryptographic_reset
                                                        .clone();
                                                    move |_, window, cx| {
                                                        on_request_cryptographic_reset(
                                                            &RequestCryptographicResetEvent,
                                                            window,
                                                            cx,
                                                        );
                                                    }
                                                }),
                                        ),
                                ),
                        ),
                ),
        )
        .page(
            div()
                .bg(theme.background)
                .w_full()
                .h_full()
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(
                    grandstand("force-verification-grandstand")
                        .text(tr!("INCOMING_VERIFICATION"))
                        .pt(px(36.))
                        .on_back_click({
                            let verification_request_entity = pending_verification.clone();
                            move |_, _, cx: &mut App| {
                                verification_request_entity.clone().unwrap().update(
                                    cx,
                                    |verification_request, cx| {
                                        verification_request.cancel(cx);
                                    },
                                );
                            }
                        }),
                )
                .child(
                    constrainer("force-verification")
                        .flex()
                        .flex_col()
                        .w_full()
                        .px(px(8.))
                        .child(
                            layer()
                                .flex()
                                .flex_col()
                                .p(px(8.))
                                .gap(px(8.))
                                .w_full()
                                .child(subtitle(tr!("INCOMING_VERIFICATION")))
                                .child(tr!(
                                    "FORCE_VERIFICATION_INCOMING_VERIFICATION_PROMPT",
                                    "Your device {{device_id}} has offered to verify this device.",
                                    device_id:quote = device_name
                                ))
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .rounded(theme.border_radius)
                                        .bg(theme.button_background)
                                        .child(
                                            button("verification-request-accept")
                                                .child(icon_text(
                                                    "dialog-ok",
                                                    tr!("INCOMING_VERIFICATION_ACCEPT"),
                                                ))
                                                .on_click({
                                                    let verification_request_entity =
                                                        pending_verification.clone();
                                                    move |_, _, cx| {
                                                        let verification_request_entity =
                                                            verification_request_entity
                                                                .clone()
                                                                .unwrap();
                                                        verification_request_entity.update(
                                                            cx,
                                                            |verification_request, cx| {
                                                                verification_request.accept(cx);
                                                            },
                                                        );

                                                        self.verification_ui
                                                            .verification_popover
                                                            .update(
                                                                cx,
                                                                |verification_popover, cx| {
                                                                    verification_popover
                                                                        .set_verification_request(
                                                                        verification_request_entity,
                                                                        cx,
                                                                    );
                                                                },
                                                            );
                                                    }
                                                }),
                                        )
                                        .child(
                                            button("verification-request-decline")
                                                .child(icon_text(
                                                    "dialog-cancel",
                                                    tr!("INCOMING_VERIFICATION_DECLINE"),
                                                ))
                                                .on_click({
                                                    let verification_request_entity =
                                                        pending_verification.clone();
                                                    move |_, _, cx: &mut App| {
                                                        verification_request_entity
                                                            .clone()
                                                            .unwrap()
                                                            .update(
                                                                cx,
                                                                |verification_request, cx| {
                                                                    verification_request.cancel(cx);
                                                                },
                                                            );
                                                    }
                                                }),
                                        ),
                                ),
                        ),
                ),
        )
    }
}
