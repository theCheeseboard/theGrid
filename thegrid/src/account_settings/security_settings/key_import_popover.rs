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
use gpui::prelude::FluentBuilder;
use gpui::{
    App, AppContext, AsyncApp, Context, Entity, IntoElement, ParentElement, Render, Styled,
    WeakEntity, Window, div, px,
};
use matrix_sdk::crypto::KeyExportError;
use matrix_sdk::encryption::RoomKeyImportError;
use std::path::PathBuf;
use thegrid::session::session_manager::SessionManager;
use thegrid::tokio_helper::TokioHelper;
use tracing::error;

pub struct KeyImportPopover {
    visible: bool,
    processing: bool,
    error: Option<RoomKeyImportError>,
    export_file: Option<PathBuf>,
    password_field: Entity<TextField>,
}

impl KeyImportPopover {
    pub fn new(cx: &mut App) -> Self {
        Self {
            visible: false,
            processing: false,
            error: None,
            export_file: None,
            password_field: cx.new(|cx| {
                let mut text_field = TextField::new("password-field", cx);
                text_field.set_mask_mode(MaskMode::password_mask());
                text_field
                    .set_placeholder(tr!("KEY_IMPORT_PASSWORD", "Password").to_string().as_str());
                text_field
            }),
        }
    }

    pub fn open(&mut self, export_file: PathBuf) {
        self.export_file = Some(export_file);
        self.error = None;
        self.visible = true;
    }

    fn perform_import(&mut self, cx: &mut Context<Self>) {
        let password = self.password_field.read(cx).text().to_string();

        let session_manager = cx.global::<SessionManager>();
        let client = session_manager.client().unwrap().read(cx).clone();
        let encryption = client.encryption();
        let export_file = self.export_file.clone().unwrap();

        self.processing = true;
        cx.notify();

        cx.spawn(
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                if let Err(e) = cx
                    .spawn_tokio(async move {
                        encryption
                            .import_room_keys(export_file, password.as_str())
                            .await
                    })
                    .await
                {
                    error!("Key import failure: {e:?}");
                    weak_this
                        .update(cx, |this, cx| {
                            this.error = Some(e);
                            this.processing = false;
                        })
                        .unwrap();
                } else {
                    weak_this
                        .update(cx, |this, cx| {
                            this.processing = false;
                            this.visible = false;
                            cx.notify();
                        })
                        .unwrap();
                }
            },
        )
        .detach();
    }
}

impl Render for KeyImportPopover {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        popover("key-import-popover")
            .visible(self.visible)
            .size_neg(100.)
            .anchor_bottom()
            .content(
                pager("key-import-pager", if self.processing { 1 } else { 0 })
                    .animation(SlideHorizontalAnimation::new())
                    .size_full()
                    .page(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(9.))
                            .child(
                                grandstand("key-import-grandstand")
                                    .text(tr!("KEY_IMPORT_TITLE", "Import Encryption Keys"))
                                    .on_back_click(cx.listener(move |this, _, _, cx| {
                                        this.visible = false;
                                        cx.notify()
                                    })),
                            )
                            .child(
                                constrainer("key-import-constrainer").child(
                                    layer()
                                        .flex()
                                        .flex_col()
                                        .p(px(8.))
                                        .w_full()
                                        .child(subtitle(tr!(
                                            "KEY_IMPORT_OPTIONS",
                                            "Import Options"
                                        )))
                                        .child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .gap(px(8.))
                                                .child(tr!(
                                                    "KEY_IMPORT_DESCRIPTION",
                                                    "Enter the password for the key export"
                                                ))
                                                .child(self.password_field.clone())
                                                .when_some(self.error.as_ref(), |david, error| {
                                                    let error_text = match error {
                                                        RoomKeyImportError::Export(
                                                            KeyExportError::InvalidMac,
                                                        ) => tr!(
                                                            "KEY_IMPORT_ERROR_INVALID_MAC",
                                                            "Check the password and try again"
                                                        ),
                                                        _ => tr!(
                                                            "KEY_IMPORT_ERROR_MESSAGE",
                                                            "Sorry, we were unable to import the \
                                                            key backup."
                                                        ),
                                                    };

                                                    david.child(
                                                        admonition()
                                                            .severity(AdmonitionSeverity::Error)
                                                            .title(tr!(
                                                                "KEY_IMPORT_ERROR_TITLE",
                                                                "Unable to import keys"
                                                            ))
                                                            .child(error_text),
                                                    )
                                                })
                                                .child(
                                                    button("do-import")
                                                        .child(icon_text(
                                                            "cloud-upload".into(),
                                                            tr!("SECURITY_KEY_BACKUP_IMPORT")
                                                                .into(),
                                                        ))
                                                        .on_click(cx.listener(
                                                            move |this, _, _, cx| {
                                                                this.perform_import(cx)
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
