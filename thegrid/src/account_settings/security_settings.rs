pub mod identity_reset;
mod key_export_popover;
mod key_import_popover;
pub mod recovery_key_reset_popover;

use crate::account_settings::security_settings::key_export_popover::KeyExportPopover;
use crate::account_settings::security_settings::key_import_popover::KeyImportPopover;
use crate::account_settings::security_settings::recovery_key_reset_popover::RecoveryKeyResetPopover;
use crate::auth::oauth_management_page_redirect_dialog::OAuthManagementPageRedirectDialog;
use crate::auth::recovery_passphrase_popover::RecoveryPassphrasePopover;
use cntp_i18n::tr;
use contemporary::components::admonition::{admonition, AdmonitionSeverity};
use contemporary::components::button::button;
use contemporary::components::constrainer::constrainer;
use contemporary::components::grandstand::grandstand;
use contemporary::components::icon_text::icon_text;
use contemporary::components::layer::layer;
use contemporary::components::subtitle::subtitle;
use contemporary::styling::theme::Theme;
use gpui::prelude::FluentBuilder;
use gpui::{
    div, px, App, AppContext, AsyncApp, ClickEvent, Context, Entity,
    IntoElement, ParentElement, PathPromptOptions, Render, Styled, Window,
};
use matrix_sdk::encryption::recovery::RecoveryState;
use matrix_sdk::encryption::VerificationState;
use std::rc::Rc;
use thegrid_common::session::session_manager::SessionManager;
use thegrid_common::surfaces::{
    MainWindowSurface, SurfaceChange, SurfaceChangeEvent, SurfaceChangeHandler,
};

pub struct SecuritySettings {
    recovery_key_reset_popover: Entity<RecoveryKeyResetPopover>,
    recovery_passphrase_popover: Entity<RecoveryPassphrasePopover>,
    key_export_popover: Entity<KeyExportPopover>,
    key_import_popover: Entity<KeyImportPopover>,
    on_surface_change: Rc<Box<SurfaceChangeHandler>>,
    oauth_management_page_redirect_dialog: Entity<OAuthManagementPageRedirectDialog>,
}

impl SecuritySettings {
    pub fn new(
        cx: &mut App,
        on_surface_change: impl Fn(&SurfaceChangeEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            recovery_key_reset_popover: cx.new(|cx| RecoveryKeyResetPopover::new(cx)),
            recovery_passphrase_popover: cx.new(|cx| RecoveryPassphrasePopover::new(cx)),
            key_export_popover: cx.new(|cx| KeyExportPopover::new(cx)),
            key_import_popover: cx.new(|cx| KeyImportPopover::new(cx)),
            on_surface_change: Rc::new(Box::new(on_surface_change)),
            oauth_management_page_redirect_dialog: cx
                .new(|cx| OAuthManagementPageRedirectDialog::new(cx)),
        }
    }

    fn open_crypto_reset_page(
        &mut self,
        _: &ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        (self.on_surface_change)(
            &SurfaceChangeEvent {
                change: SurfaceChange::Push(MainWindowSurface::IdentityReset),
            },
            window,
            cx,
        );
        cx.notify();
    }

    fn start_import(&mut self, cx: &mut Context<Self>) {
        let key_import_popover = self.key_import_popover.clone();
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            prompt: Some(tr!("KEY_IMPORT_IMPORT", "Import").into()),
            directories: false,
            files: true,
            multiple: false,
        });
        cx.spawn(async move |_, cx: &mut AsyncApp| {
            if let Some(mut path) = prompt.await.ok().and_then(|result| result.ok()).flatten() {
                key_import_popover.update(cx, |key_import_popover, cx| {
                    key_import_popover.open(path.remove(0));
                    cx.notify()
                });
            };
        })
        .detach();
    }
}

impl Render for SecuritySettings {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>();

        let session_manager = cx.global::<SessionManager>();
        let client = session_manager.client().unwrap().read(cx);
        let recovery = client.encryption().recovery();
        let recovery_state = recovery.state();
        let verified = session_manager
            .current_account()
            .read(cx)
            .verification_state()
            == VerificationState::Verified;

        div()
            .bg(theme.background)
            .w_full()
            .h_full()
            .flex()
            .flex_col()
            .child(
                grandstand("security-grandstand")
                    .text(tr!("ACCOUNT_SETTINGS_SECURITY"))
                    .pt(px(36.)),
            )
            .child(
                constrainer("security")
                    .flex()
                    .flex_col()
                    .w_full()
                    .p(px(8.))
                    .gap(px(8.))
                    .when(recovery_state == RecoveryState::Disabled, |david| {
                        david.child(
                            admonition()
                                .severity(AdmonitionSeverity::Warning)
                                .title(tr!("SETUP_RECOVERY"))
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap(px(4.))
                                        .child(tr!("SETUP_RECOVERY_DESCRIPTION"))
                                        .child(
                                            div().flex().child(div().flex_grow()).child(
                                                button("setup-now")
                                                    .child(icon_text(
                                                        "configure".into(),
                                                        tr!("SETUP_RECOVERY_NOW").into(),
                                                    ))
                                                    .on_click(cx.listener(
                                                        move |this, _, _, cx| {
                                                            this.recovery_key_reset_popover.update(
                                                                cx,
                                                                |popover, cx| {
                                                                    popover.open(cx);
                                                                    cx.notify();
                                                                },
                                                            )
                                                        },
                                                    )),
                                            ),
                                        ),
                                ),
                        )
                    })
                    .when(
                        recovery_state == RecoveryState::Incomplete && verified,
                        |david| {
                            david.child(
                                admonition()
                                    .severity(AdmonitionSeverity::Warning)
                                    .title(tr!("FIX_RECOVERY"))
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap(px(4.))
                                            .child(tr!("FIX_RECOVERY_DESCRIPTION"))
                                            .child(
                                                div().flex().child(div().flex_grow()).child(
                                                    button("verify-recovery")
                                                        .child(icon_text(
                                                            "visibility".into(),
                                                            tr!("VERIFY_SESSION_RECOVERY_KEY",).into(),
                                                        ))
                                                        .on_click(cx.listener(
                                                            move |this, _, _, cx| {
                                                                this.recovery_passphrase_popover.update(
                                                                    cx,
                                                                    |recovery_passphrase_popover, cx| {
                                                                        recovery_passphrase_popover
                                                                            .set_visible(true);
                                                                        cx.notify()
                                                                    },
                                                                )
                                                            },
                                                        )),
                                                ),
                                            ),
                                    ),
                            )
                        },
                    )
                    .when(verified, |david| {
                        david.child(
                            layer()
                                .flex()
                                .flex_col()
                                .p(px(8.))
                                .w_full()
                                .child(subtitle(tr!("SECURITY_ENCRYPTION", "Encryption")))
                                .child(div().child(tr!("SETUP_RECOVERY_DESCRIPTION")))
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .bg(theme.button_background)
                                        .rounded(theme.border_radius)
                                        .child(
                                            button("key-setup")
                                                .child(match recovery_state {
                                                    RecoveryState::Enabled => icon_text(
                                                        "edit-rename".into(),
                                                        tr!(
                                                            "SECURITY_RECOVERY_KEY_CHANGE",
                                                            "Change Recovery Key"
                                                        )
                                                            .into(),
                                                    ),
                                                    RecoveryState::Unknown
                                                    | RecoveryState::Disabled => icon_text(
                                                        "configure".into(),
                                                        tr!(
                                                            "SECURITY_RECOVERY_KEY_SETUP",
                                                            "Set up Recovery Key"
                                                        )
                                                            .into(),
                                                    ),
                                                    RecoveryState::Incomplete => icon_text(
                                                        "edit-rename".into(),
                                                        tr!(
                                                            "SECURITY_RECOVERY_KEY_RESET",
                                                            "Reset Recovery Key"
                                                        )
                                                            .into(),
                                                    ),
                                                })
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.recovery_key_reset_popover.update(
                                                        cx,
                                                        |recovery_key_reset_popover, cx| {
                                                            recovery_key_reset_popover.open(cx);
                                                            cx.notify()
                                                        },
                                                    );
                                                    cx.notify()
                                                })),
                                        )
                                ),
                        )
                    })
                    .child(
                        layer()
                            .flex()
                            .flex_col()
                            .p(px(8.))
                            .w_full()
                            .child(subtitle(tr!("SECURITY_KEY_BACKUP", "Key Backup")))
                            .child(div().child(tr!(
                                "SECURITY_KEY_BACKUP_DESCRIPTION",
                                "If you'd like, you can back up the keys used to encrypt your \
                                secure messages. You can import these keys into another Matrix \
                                client in order to grant it access to decrypt your messages."
                            )))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .bg(theme.button_background)
                                    .rounded(theme.border_radius)
                                    .child(
                                        button("key-backup")
                                            .child(icon_text(
                                                "cloud-download".into(),
                                                tr!(
                                                    "SECURITY_KEY_BACKUP_EXPORT",
                                                    "Export Encryption Keys"
                                                )
                                                    .into(),
                                            ))
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.key_export_popover.update(
                                                    cx,
                                                    |key_export_popover, cx| {
                                                        key_export_popover.set_visible(true)
                                                    },
                                                );
                                                cx.notify()
                                            })),
                                    )
                                    .child(
                                        button("profile-change-profile-picture")
                                            .child(icon_text(
                                                "cloud-upload".into(),
                                                tr!(
                                                    "SECURITY_KEY_BACKUP_IMPORT",
                                                    "Import Encryption Keys"
                                                )
                                                    .into(),
                                            ))
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.start_import(cx);
                                            })),
                                    ),
                            ),
                    ).child(
                        layer()
                            .flex()
                            .flex_col()
                            .p(px(8.))
                            .gap(px(4.))
                            .w_full()
                            .child(subtitle(tr!(
                                "SECURITY_CRYPTO_IDENTITY",
                                "Cryptographic Identity"
                            )))
                            .child(tr!(
                                "SECURITY_IDENTITY_DESCRIPTION",
                                "Your cryptographic identity allows you to build a network of \
                                devices and users that you can verify and trust."
                            ))
                            .child(tr!(
                                "SECURITY_IDENTITY_DESCRIPTION_2",
                                "If you have lost access to all of your verified devices, or \
                                if you suspect that an unauthorised device has gained access \
                                to your Matrix account, you can reset your cryptographic identity \
                                to clear out your verified devices."
                            ))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .bg(theme.button_background)
                                    .rounded(theme.border_radius)
                                    .child(
                                        button("key-reset")
                                            .child(icon_text(
                                                "view-refresh".into(),
                                                tr!(
                                                    "SECURITY_IDENTITY_RESET",
                                                    "Reset Cryptographic Identity"
                                                )
                                                    .into(),
                                            ))
                                            .destructive()
                                            .on_click(cx.listener(Self::open_crypto_reset_page)),
                                    ),
                            ),
                    ),
            )
            .child(self.key_export_popover.clone())
            .child(self.key_import_popover.clone())
            .child(self.recovery_key_reset_popover.clone())
            .child(self.recovery_passphrase_popover.clone()).child(self.oauth_management_page_redirect_dialog.clone())
    }
}
