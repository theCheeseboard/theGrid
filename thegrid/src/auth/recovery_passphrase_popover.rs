use cntp_i18n::tr;
use contemporary::components::admonition::{AdmonitionSeverity, admonition};
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
use contemporary::components::text_field::{MaskMode, TextField};
use gpui::{
    App, AppContext, AsyncApp, Context, Entity, IntoElement, ParentElement, Render, Styled,
    WeakEntity, Window, div, px,
};
use matrix_sdk::encryption::recovery::RecoveryState::Enabled;
use thegrid::session::session_manager::SessionManager;
use thegrid::tokio_helper::TokioHelper;

pub struct RecoveryPassphrasePopover {
    visible: bool,
    recovery_passphrase_field: Entity<TextField>,
    recovery_state: RecoveryState,
}

enum RecoveryState {
    Idle,
    Recovering,
    Error(String),
    Complete,
    CompleteWithIncompleteRecovery,
}

impl RecoveryPassphrasePopover {
    pub fn new(cx: &mut App) -> Self {
        RecoveryPassphrasePopover {
            visible: false,
            recovery_passphrase_field: cx.new(|cx| {
                let mut text_field = TextField::new("recovery-passphrase-field", cx);
                text_field.set_mask_mode(MaskMode::password_mask());
                text_field.set_placeholder(
                    tr!("RECOVERY_PASSPHRASE_PLACEHOLDER", "Recovery Passphrase")
                        .to_string()
                        .as_str(),
                );
                text_field
            }),
            recovery_state: RecoveryState::Idle,
        }
    }

    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    fn perform_recovery(&mut self, cx: &mut Context<Self>) {
        self.recovery_state = RecoveryState::Recovering;

        let session_manager = cx.global::<SessionManager>();
        let client = session_manager.client().unwrap().read(cx).clone();
        let recovery = client.encryption().recovery();
        let recovery_key = self.recovery_passphrase_field.read(cx).text().to_string();
        cx.spawn(
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                if let Err(error) = cx
                    .spawn_tokio(async move {
                        client
                            .encryption()
                            .recovery()
                            .recover(recovery_key.as_str())
                            .await
                    })
                    .await
                {
                    weak_this.update(cx, |this, cx| {
                        this.recovery_state = RecoveryState::Error(error.to_string());
                        cx.notify();
                    })
                } else {
                    if recovery.state() == Enabled {
                        weak_this.update(cx, |this, cx| {
                            this.recovery_state = RecoveryState::Complete;
                            cx.notify();
                        })
                    } else {
                        weak_this.update(cx, |this, cx| {
                            this.recovery_state = RecoveryState::CompleteWithIncompleteRecovery;
                            cx.notify();
                        })
                    }
                }
            },
        )
        .detach();

        cx.notify();
    }
}

impl Render for RecoveryPassphrasePopover {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        popover("recovery-passphrase-popover")
            .visible(self.visible)
            .size_neg(100.)
            .anchor_bottom()
            .content(
                pager(
                    "recovery-passphrase-pager",
                    match &self.recovery_state {
                        RecoveryState::Idle => 0,
                        RecoveryState::Recovering => 1,
                        RecoveryState::Error(_) => 0,
                        RecoveryState::Complete => 2,
                        RecoveryState::CompleteWithIncompleteRecovery => 3,
                    },
                )
                .size_full()
                .animation(FadeAnimation::new())
                .page(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(9.))
                        .child(
                            grandstand("recovery-passphrase-popover-grandstand")
                                .text(tr!(
                                    "POPOVER_RECOVERY_PASSPHRASE_GRANDSTAND",
                                    "Verify with Recovery Key"
                                ))
                                .on_back_click(cx.listener(move |this, _, _, cx| {
                                    this.visible = false;
                                    this.recovery_state = RecoveryState::Idle;
                                    cx.notify()
                                })),
                        )
                        .child(
                            constrainer("recovery-passphrase-popover-constrainer").child(
                                layer()
                                    .flex()
                                    .flex_col()
                                    .p(px(8.))
                                    .w_full()
                                    .child(subtitle(tr!(
                                        "POPOVER_RECOVERY_PASSPHRASE",
                                        "Enter the recovery passphrase"
                                    )))
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap(px(8.))
                                            .child(tr!(
                                                "POPOVER_RECOVERY_PASSPHRASE_DESCRIPTION",
                                                "Enter the recovery passphrase for your \
                                                    account to verify this session. You can also \
                                                    use the recovery key here."
                                            ))
                                            .child(
                                                admonition()
                                                    .severity(AdmonitionSeverity::Info)
                                                    .title(tr!(
                                                        "RECOVERY_KEY_WHAT_TITLE",
                                                        "Recovery what?"
                                                    ))
                                                    .child(tr!(
                                                        "RECOVERY_KEY_WHAT_DESCRIPTION",
                                                        "The recovery passphrase and key was \
                                                            set up when you first set up your \
                                                            account. If you don't know your \
                                                            recovery passphrase or key, and you \
                                                            don't have any verified devices to \
                                                            recover from, you'll have to reset \
                                                            your account's encryption details."
                                                    )),
                                            )
                                            .child(self.recovery_passphrase_field.clone())
                                            .child(
                                                button("verification-popover-ok")
                                                    .child(icon_text(
                                                        "dialog-ok".into(),
                                                        tr!("RECOVER_ACCOUNT", "Recover Account")
                                                            .into(),
                                                    ))
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.perform_recovery(cx)
                                                    })),
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
                            grandstand("recovery-passphrase-popover-grandstand")
                                .text(tr!(
                                    "POPOVER_RECOVERY_PASSPHRASE_GRANDSTAND",
                                    "Verify with Recovery Key"
                                ))
                                .on_back_click(cx.listener(move |this, _, _, cx| {
                                    this.visible = false;
                                    this.recovery_state = RecoveryState::Idle;
                                    cx.notify()
                                })),
                        )
                        .child(
                            constrainer("recovery-passphrase-popover-constrainer").child(
                                layer()
                                    .flex()
                                    .flex_col()
                                    .p(px(8.))
                                    .w_full()
                                    .child(subtitle(tr!(
                                        "RECOVERY_PASSPHRASE_POPOVER_OK",
                                        "Account recovered"
                                    )))
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap(px(8.))
                                            .child(tr!(
                                                "RECOVERY_PASSPHRASE_POPOVER_OK_MESSAGE",
                                                "This session was verified with the recovery key."
                                            ))
                                            .child(
                                                button("recovery-passphrase-popover-ok")
                                                    .child(icon_text(
                                                        "dialog-ok".into(),
                                                        tr!("CLOSE", "Close").into(),
                                                    ))
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.visible = false;
                                                        this.recovery_state = RecoveryState::Idle;
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
                            grandstand("recovery-passphrase-popover-grandstand")
                                .text(tr!(
                                    "POPOVER_RECOVERY_PASSPHRASE_GRANDSTAND",
                                    "Verify with Recovery Key"
                                ))
                                .on_back_click(cx.listener(move |this, _, _, cx| {
                                    this.visible = false;
                                    this.recovery_state = RecoveryState::Idle;
                                    cx.notify()
                                })),
                        )
                        .child(
                            constrainer("recovery-passphrase-popover-constrainer").child(
                                layer()
                                    .flex()
                                    .flex_col()
                                    .p(px(8.))
                                    .w_full()
                                    .child(subtitle(tr!(
                                        "RECOVERY_PASSPHRASE_POPOVER_CORRUPT",
                                        "Your account recovery data is corrupt"
                                    )))
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap(px(8.))
                                            .child(tr!(
                                                "RECOVERY_PASSPHRASE_POPOVER_CORRUPT_MESSAGE",
                                                "The recovery data for your account is corrupt. To \
                                                recover from this state, you need to reset the \
                                                recovery key from a verified session that has all \
                                                of your encryption data. If you don't have one, \
                                                you will need to reset your cryptographic identity \
                                                from Account Settings."
                                            ))
                                            .child(
                                                button("recovery-passphrase-popover-ok")
                                                    .child(icon_text(
                                                        "dialog-ok".into(),
                                                        tr!("SORRY", "Sorry").into(),
                                                    ))
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.visible = false;
                                                        this.recovery_state = RecoveryState::Idle;
                                                        cx.notify()
                                                    })),
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
