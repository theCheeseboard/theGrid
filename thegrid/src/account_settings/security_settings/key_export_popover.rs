use cntp_i18n::tr;
use contemporary::components::admonition::{AdmonitionSeverity, admonition};
use contemporary::components::button::button;
use contemporary::components::constrainer::constrainer;
use contemporary::components::grandstand::grandstand;
use contemporary::components::icon_text::icon_text;
use contemporary::components::layer::layer;
use contemporary::components::pager::pager;
use contemporary::components::pager::slide_horizontal_animation::SlideHorizontalAnimation;
use contemporary::components::popover::popover;
use contemporary::components::spinner::spinner;
use contemporary::components::subtitle::subtitle;
use contemporary::components::text_field::{MaskMode, TextField};
use directories::UserDirs;
use gpui::{
    App, AppContext, AsyncApp, Context, Entity, IntoElement, ParentElement, Render, Styled,
    WeakEntity, Window, div, px,
};
use thegrid::session::session_manager::SessionManager;
use thegrid::tokio_helper::TokioHelper;

pub struct KeyExportPopover {
    visible: bool,
    processing: bool,
    password_field: Entity<TextField>,
    password_confirm_field: Entity<TextField>,
}

impl KeyExportPopover {
    pub fn new(cx: &mut App) -> Self {
        Self {
            visible: false,
            processing: false,
            password_field: cx.new(|cx| {
                let mut text_field = TextField::new("password", cx);
                text_field.set_mask_mode(MaskMode::password_mask());
                text_field
                    .set_placeholder(tr!("PASSWORD", "Password").to_string().as_str());
                text_field
            }),
            password_confirm_field: cx.new(|cx| {
                let mut text_field = TextField::new("password-confirm-field", cx);
                text_field.set_mask_mode(MaskMode::password_mask());
                text_field.set_placeholder(
                    tr!("PASSWORD_CONFIRM", "Confirm Password")
                        .to_string()
                        .as_str(),
                );
                text_field
            }),
        }
    }

    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    fn perform_export(&mut self, cx: &mut Context<Self>) {
        let password = self.password_field.read(cx).text().to_string();
        let password_confirm = self.password_confirm_field.read(cx).text().to_string();
        if password.is_empty() {
            // TODO: Show error
            return;
        }
        if password != password_confirm {
            // TODO: Show error
            return;
        }

        let session_manager = cx.global::<SessionManager>();
        let client = session_manager.client().unwrap().read(cx).clone();
        let encryption = client.encryption();

        let user_dirs = UserDirs::new().unwrap();
        let prompt =
            cx.prompt_for_new_path(user_dirs.document_dir().unwrap(), Some("thegrid-keys.txt"));
        cx.spawn(
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                if let Some(path) = prompt.await.ok().and_then(|result| result.ok()).flatten() {
                    weak_this
                        .update(cx, |this, cx| {
                            this.processing = true;
                        })
                        .unwrap();

                    if cx
                        .spawn_tokio(async move {
                            encryption
                                .export_room_keys(path, password.as_str(), |_| true)
                                .await
                        })
                        .await
                        .is_ok()
                    {
                        weak_this
                            .update(cx, |this, cx| {
                                this.processing = false;
                                this.visible = false;
                                cx.notify();
                            })
                            .unwrap();
                    } else {
                        weak_this
                            .update(cx, |this, cx| {
                                this.processing = false;
                            })
                            .unwrap();
                    }
                };
            },
        )
        .detach();
    }
}

impl Render for KeyExportPopover {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        popover("key-export-popover")
            .visible(self.visible)
            .size_neg(100.)
            .anchor_bottom()
            .content(
                pager("key-export-pager", if self.processing { 1 } else { 0 })
                    .animation(SlideHorizontalAnimation::new())
                    .size_full()
                    .page(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(9.))
                            .child(
                                grandstand("key-export-grandstand")
                                    .text(tr!("KEY_EXPORT_TITLE", "Export Encryption Keys"))
                                    .on_back_click(cx.listener(move |this, _, _, cx| {
                                        this.visible = false;
                                        cx.notify()
                                    })),
                            )
                            .child(
                                constrainer("key-export-constrainer").child(
                                    layer()
                                        .flex()
                                        .flex_col()
                                        .p(px(8.))
                                        .w_full()
                                        .child(subtitle(tr!(
                                            "KEY_EXPORT_OPTIONS",
                                            "Export Options"
                                        )))
                                        .child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .gap(px(8.))
                                                .child(tr!(
                                                    "KEY_EXPORT_DESCRIPTION",
                                                    "Enter a password to encrypt the exported \
                                                encryption keys. This password will be necessary \
                                                to import these keys again."
                                                ))
                                                .child(self.password_field.clone())
                                                .child(self.password_confirm_field.clone())
                                                .child(
                                                    admonition()
                                                        .severity(AdmonitionSeverity::Warning)
                                                        .title(tr!("WARNING"))
                                                        .child(tr!(
                                                            "SECURITY_KEY_EXPORT_WARNING",
                                                            "Keep this file and the password \
                                                        guarded. Anyone who gets access to this \
                                                        file and the password will be able to \
                                                        decrypt your messages."
                                                        )),
                                                )
                                                .child(
                                                    button("do-export")
                                                        .child(icon_text(
                                                            "cloud-download".into(),
                                                            tr!("SECURITY_KEY_BACKUP_EXPORT")
                                                                .into(),
                                                        ))
                                                        .on_click(cx.listener(
                                                            move |this, _, _, cx| {
                                                                this.perform_export(cx)
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
                    ),
            )
    }
}
