use cntp_i18n::tr;
use contemporary::components::admonition::{AdmonitionSeverity, admonition};
use contemporary::components::button::button;
use contemporary::components::constrainer::constrainer;
use contemporary::components::grandstand::grandstand;
use contemporary::components::icon::icon;
use contemporary::components::icon_text::icon_text;
use contemporary::components::layer::layer;
use contemporary::components::pager::pager;
use contemporary::components::pager::slide_horizontal_animation::SlideHorizontalAnimation;
use contemporary::components::popover::popover;
use contemporary::components::spinner::spinner;
use contemporary::components::subtitle::subtitle;
use contemporary::components::text_field::TextField;
use gpui::prelude::FluentBuilder;
use gpui::{
    App, AsyncApp, ClipboardItem, Context, Entity, IntoElement, ParentElement, Render, Styled,
    WeakEntity, Window, div, px,
};
use matrix_sdk::crypto::KeyExportError;
use matrix_sdk::encryption::RoomKeyImportError;
use matrix_sdk::encryption::recovery::{RecoveryError, RecoveryState};
use std::path::PathBuf;
use thegrid::session::session_manager::SessionManager;
use thegrid::tokio_helper::TokioHelper;
use tracing::error;

pub struct RecoveryKeyResetPopover {
    visible: bool,
    recovery_not_set_up: bool,
    state: RecoveryKeyResetState,
    error: Option<RecoveryError>,
    passphrase_field: Entity<TextField>,
}

#[derive(Clone, PartialEq)]
enum RecoveryKeyResetState {
    RecoveryPassphrase,
    Processing,
    DisplayRecoveryKey(String),
}

impl RecoveryKeyResetPopover {
    pub fn new(cx: &mut App) -> Self {
        let password_field = TextField::new(
            cx,
            "password-field",
            "".into(),
            tr!("RECOVERY_PASSPHRASE", "Recovery Passphrase (optional)").into(),
        );
        password_field.update(cx, |password_field, cx| {
            password_field.password_field(cx, true);
        });

        Self {
            recovery_not_set_up: false,
            visible: false,
            state: RecoveryKeyResetState::RecoveryPassphrase,
            error: None,
            passphrase_field: password_field,
        }
    }

    pub fn open(&mut self, cx: &mut Context<Self>) {
        let session_manager = cx.global::<SessionManager>();
        let client = session_manager.client().unwrap().read(cx);
        let recovery = client.encryption().recovery();
        let recovery_not_set_up = recovery.state() == RecoveryState::Disabled;

        self.recovery_not_set_up = recovery_not_set_up;
        self.state = RecoveryKeyResetState::RecoveryPassphrase;
        self.error = None;
        self.visible = true;
    }

    fn perform_reset(&mut self, cx: &mut Context<Self>) {
        let passphrase = self.passphrase_field.read(cx).current_text(cx);

        let session_manager = cx.global::<SessionManager>();
        let client = session_manager.client().unwrap().read(cx).clone();
        let encryption = client.encryption();
        let recovery = encryption.recovery();
        let backups = encryption.backups();

        self.state = RecoveryKeyResetState::Processing;
        cx.notify();

        cx.spawn(
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                let result = cx
                    .spawn_tokio(async move {
                        if backups.fetch_exists_on_server().await.unwrap() {
                            if passphrase.is_empty() {
                                recovery.reset_key().await
                            } else {
                                recovery
                                    .reset_key()
                                    .with_passphrase(passphrase.as_str())
                                    .await
                            }
                        } else {
                            if passphrase.is_empty() {
                                recovery.enable().await
                            } else {
                                recovery.enable().with_passphrase(passphrase.as_str()).await
                            }
                        }
                    })
                    .await;
                match result {
                    Ok(recovery_key) => {
                        weak_this
                            .update(cx, |this, cx| {
                                this.state =
                                    RecoveryKeyResetState::DisplayRecoveryKey(recovery_key);
                            })
                            .unwrap();
                    }
                    Err(e) => {
                        error!("Recovery key setup failure: {e:?}");
                        weak_this
                            .update(cx, |this, cx| {
                                this.error = Some(e);
                                this.state = RecoveryKeyResetState::RecoveryPassphrase;
                            })
                            .unwrap();
                    }
                }
            },
        )
        .detach();
    }
}

impl Render for RecoveryKeyResetPopover {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        popover("key-reset-popover")
            .visible(self.visible)
            .size_neg(100.)
            .anchor_bottom()
            .content(
                pager(
                    "key-reset-pager",
                    match self.state {
                        RecoveryKeyResetState::RecoveryPassphrase => 0,
                        RecoveryKeyResetState::Processing => 1,
                        RecoveryKeyResetState::DisplayRecoveryKey(_) => 2,
                    },
                )
                .animation(SlideHorizontalAnimation::new())
                .size_full()
                .page(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(9.))
                        .child(
                            grandstand("key-reset-grandstand")
                                .when_else(
                                    self.recovery_not_set_up,
                                    |david| {
                                        david.text(tr!("KEY_SETUP_TITLE", "Set up Recovery Key"))
                                    },
                                    |david| {
                                        david.text(tr!("KEY_RESET_TITLE", "Reset Recovery Key"))
                                    },
                                )
                                .on_back_click(cx.listener(move |this, _, _, cx| {
                                    this.visible = false;
                                    cx.notify()
                                })),
                        )
                        .child(
                            constrainer("key-reset-constrainer").child(
                                layer()
                                    .flex()
                                    .flex_col()
                                    .p(px(8.))
                                    .w_full()
                                    .child(subtitle(tr!(
                                        "KEY_RESET_OPTIONS",
                                        "Recovery Key Options"
                                    )))
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap(px(8.))
                                            .when_else(
                                                self.recovery_not_set_up,
                                                |david| {
                                                    david.child(tr!(
                                                        "KEY_SETUP_DESCRIPTION",
                                                        "A recovery key will be created, \
                                                        which you can use to recover your \
                                                        encrypted messages in the event \
                                                        you log out of all of your devices."
                                                    ))
                                                },
                                                |david| {
                                                    david.child(tr!(
                                                        "KEY_RESET_DESCRIPTION",
                                                        "If you've forgotten your recovery \
                                                        key, you can reset it here. Your old \
                                                        recovery key and recovery passphrase, \
                                                        if set, will become invalid."
                                                    ))
                                                },
                                            )
                                            .child(tr!(
                                                "KEY_RESET_PASSPHRASE",
                                                "You have the option of setting up a recovery \
                                                passphrase if you so desire. You can use the \
                                                recovery passphrase in lieu of the recovery key \
                                                to recover your encrypted messages."
                                            ))
                                            .child(
                                                admonition()
                                                    .severity(AdmonitionSeverity::Warning)
                                                    .title(tr!("HEADS_UP"))
                                                    .child(tr!(
                                                        "KEY_RESET_PASSPHRASE_WARNING",
                                                        "Avoid using your account password as the \
                                                        recovery passphrase. If someone gains \
                                                        knowledge of your account password, they \
                                                        will be able to both log into your Matrix \
                                                        account, decrypt all of your messages, and \
                                                        will also be able to impersonate you."
                                                    )),
                                            )
                                            .child(self.passphrase_field.clone())
                                            .when_some(self.error.as_ref(), |david, error| {
                                                let error_text = match error {
                                                    _ => tr!(
                                                        "KEY_RESET_ERROR_MESSAGE",
                                                        "Sorry, we were unable to update your \
                                                        account's recovery key."
                                                    ),
                                                };

                                                david.child(
                                                    admonition()
                                                        .severity(AdmonitionSeverity::Error)
                                                        .title(tr!(
                                                            "KEY_RESET_ERROR_TITLE",
                                                            "Unable to update recovery key"
                                                        ))
                                                        .child(error_text),
                                                )
                                            })
                                            .child(
                                                button("do-import")
                                                    .when_else(
                                                        self.recovery_not_set_up,
                                                        |button| {
                                                            button.child(icon_text(
                                                                "dialog-ok".into(),
                                                                tr!(
                                                                    "SECURITY_RECOVERY_KEY_SETUP",
                                                                    "Set up Recovery Key"
                                                                )
                                                                .into(),
                                                            ))
                                                        },
                                                        |button| {
                                                            button.child(icon_text(
                                                                "edit-rename".into(),
                                                                tr!(
                                                                    "SECURITY_RECOVERY_KEY_RESET",
                                                                    "Reset Recovery Key"
                                                                )
                                                                .into(),
                                                            ))
                                                        },
                                                    )
                                                    .on_click(cx.listener(
                                                        move |this, _, _, cx| {
                                                            this.perform_reset(cx)
                                                        },
                                                    )),
                                            ),
                                    ),
                            ),
                        )
                        .into_any_element(),
                )
                .page(
                    div()
                        .size_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(spinner())
                        .into_any_element(),
                )
                .page(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(9.))
                        .child(
                            grandstand("key-reset-grandstand")
                                .when_else(
                                    self.recovery_not_set_up,
                                    |david| {
                                        david.text(tr!("KEY_SETUP_TITLE", "Set up Recovery Key"))
                                    },
                                    |david| {
                                        david.text(tr!("KEY_RESET_TITLE", "Reset Recovery Key"))
                                    },
                                )
                                .on_back_click(cx.listener(move |this, _, _, cx| {
                                    this.visible = false;
                                    cx.notify()
                                })),
                        )
                        .child(
                            constrainer("key-reset-constrainer").child(
                                layer()
                                    .flex()
                                    .flex_col()
                                    .p(px(8.))
                                    .w_full()
                                    .child(subtitle(tr!("KEY_RESET_COMPLETE", "Recovery Key")))
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap(px(8.))
                                            .child(tr!(
                                                "KEY_RESET_OK_DESCRIPTION",
                                                "Your recovery key has been set up. Keep the \
                                                recovery key somewhere safe, as you will need it \
                                                if you lose access to all your verified devices."
                                            ))
                                            .child(
                                                if let RecoveryKeyResetState::DisplayRecoveryKey(
                                                    recovery_key,
                                                ) = self.state.clone()
                                                {
                                                    layer()
                                                        .flex()
                                                        .items_center()
                                                        .p(px(4.))
                                                        .gap(px(4.))
                                                        .child(
                                                            div()
                                                                .flex_grow()
                                                                .child(recovery_key.clone()),
                                                        )
                                                        .child(
                                                            button("copy-recovery-key")
                                                                .flat()
                                                                .child(icon("edit-copy".into()))
                                                                .on_click(move |_, _, cx| {
                                                                    cx.write_to_clipboard(
                                                                        ClipboardItem::new_string(
                                                                            recovery_key.clone(),
                                                                        ),
                                                                    )
                                                                }),
                                                        )
                                                        .into_any_element()
                                                } else {
                                                    div().into_any_element()
                                                },
                                            )
                                            .child(
                                                admonition()
                                                    .severity(AdmonitionSeverity::Error)
                                                    .title(tr!("HEADS_UP", "Heads up!"))
                                                    .child(tr!(
                                                        "KEY_RESET_WARNING",
                                                        "It is imperative that you save this \
                                                        recovery key now. You won't be able to \
                                                        see it again."
                                                    )),
                                            )
                                            .child(
                                                button("finish")
                                                    .child(icon_text(
                                                        "dialog-ok".into(),
                                                        tr!("DONE").into(),
                                                    ))
                                                    .on_click(cx.listener(
                                                        move |this, _, _, cx| {
                                                            this.visible = false;
                                                            cx.notify();
                                                        },
                                                    )),
                                            ),
                                    ),
                            ),
                        )
                        .into_any_element(),
                ),
            )
    }
}
