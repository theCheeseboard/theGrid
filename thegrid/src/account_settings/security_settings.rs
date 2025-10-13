mod key_export_popover;
mod key_import_popover;

use crate::account_settings::security_settings::key_export_popover::KeyExportPopover;
use crate::account_settings::security_settings::key_import_popover::KeyImportPopover;
use cntp_i18n::tr;
use contemporary::components::button::button;
use contemporary::components::constrainer::constrainer;
use contemporary::components::grandstand::grandstand;
use contemporary::components::icon_text::icon_text;
use contemporary::components::layer::layer;
use contemporary::components::subtitle::subtitle;
use contemporary::styling::theme::Theme;
use directories::UserDirs;
use gpui::{
    App, AppContext, AsyncApp, Context, Entity, IntoElement, ParentElement, PathPromptOptions,
    Render, Styled, WeakEntity, Window, div, px,
};

pub struct SecuritySettings {
    key_export_popover: Entity<KeyExportPopover>,
    key_import_popover: Entity<KeyImportPopover>,
}

impl SecuritySettings {
    pub fn new(cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self {
            key_export_popover: cx.new(|cx| KeyExportPopover::new(cx)),
            key_import_popover: cx.new(|cx| KeyImportPopover::new(cx)),
        })
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
                key_import_popover
                    .update(cx, |key_import_popover, cx| {
                        key_import_popover.open(path.remove(0));
                        cx.notify()
                    })
                    .unwrap();
            };
        })
        .detach();
    }
}

impl Render for SecuritySettings {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>();

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
                    ),
            )
            .child(self.key_export_popover.clone())
            .child(self.key_import_popover.clone())
    }
}
