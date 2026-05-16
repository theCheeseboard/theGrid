use crate::uiaa_client::{CancelAuthenticationEvent, SendAuthDataEvent, UiaaClient};
use cntp_i18n::tr;
use contemporary::components::admonition::{AdmonitionSeverity, admonition};
use contemporary::components::button::button;
use contemporary::components::constrainer::constrainer;
use contemporary::components::dialog_box::{StandardButton, dialog_box};
use contemporary::components::grandstand::grandstand;
use contemporary::components::icon_text::icon_text;
use contemporary::components::layer::layer;
use contemporary::components::pager::pager;
use contemporary::components::pager::pager_animation::PagerAnimationDirection;
use contemporary::components::pager::slide_horizontal_animation::SlideHorizontalAnimation;
use contemporary::components::spinner::spinner;
use contemporary::components::subtitle::subtitle;
use contemporary::components::text_field::{MaskMode, TextField};
use contemporary::styling::theme::Theme;
use contemporary::surface::surface;
use gpui::http_client::anyhow;
use gpui::prelude::FluentBuilder;
use gpui::{
    App, AppContext, AsyncApp, AsyncWindowContext, Context, Entity, IntoElement, ParentElement,
    Render, Styled, WeakEntity, Window, div, px,
};
use matrix_sdk::Error;
use matrix_sdk::encryption::CrossSigningResetAuthType;
use matrix_sdk::encryption::recovery::{IdentityResetHandle, RecoveryError};
use matrix_sdk::ruma::api::client::uiaa::AuthData;
use matrix_sdk::ruma::api::error::ErrorKind;
use std::rc::Rc;
use thegrid_common::session::session_manager::SessionManager;
use thegrid_common::surfaces::{SurfaceChange, SurfaceChangeEvent, SurfaceChangeHandler};
use thegrid_common::tokio_helper::TokioHelper;
use tracing::{Id, error};

pub struct PasswordChangeSurface {
    state: PasswordChangeState,
    error: Option<Error>,

    password_field: Entity<TextField>,
    password_confirm_field: Entity<TextField>,

    uiaa_client: Entity<UiaaClient>,

    on_surface_change: Rc<Box<SurfaceChangeHandler>>,
}

enum PasswordChangeState {
    Confirm,
    Processing,
    Complete,
}

impl PasswordChangeSurface {
    pub fn new(
        cx: &mut Context<Self>,
        on_surface_change: impl Fn(&SurfaceChangeEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        let send_auth_data =
            cx.listener(|this: &mut Self, event: &SendAuthDataEvent, window, cx| {
                this.perform_password_change(event.auth_data.clone(), window, cx);
            });
        let cancel_auth_listener = cx.listener(|this, _: &CancelAuthenticationEvent, _, cx| {
            this.state = PasswordChangeState::Confirm;
            cx.notify();
        });

        let uiaa_client = cx.new(|cx| UiaaClient::new(send_auth_data, cancel_auth_listener, cx));

        Self {
            state: PasswordChangeState::Confirm,
            error: None,

            password_field: cx.new(|cx| {
                let mut text_field = TextField::new("password-field", cx);
                text_field.set_placeholder(&tr!("PASSWORD_NEW", "New Password"));
                text_field.set_mask_mode(MaskMode::password_mask());
                text_field
            }),
            password_confirm_field: cx.new(|cx| {
                let mut text_field = TextField::new("password-confirm-field", cx);
                text_field.set_placeholder(&tr!("PASSWORD_CONFIRM", "Confirm Password"));
                text_field.set_mask_mode(MaskMode::password_mask());
                text_field
            }),

            uiaa_client,

            on_surface_change: Rc::new(Box::new(on_surface_change)),
        }
    }

    fn perform_password_change(
        &mut self,
        auth_data: Option<AuthData>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let password = self.password_field.read(cx).text();
        let confirm_password = self.password_confirm_field.read(cx).text();

        if password.is_empty() {
            self.password_field
                .update(cx, |field, cx| field.flash_error(window, cx));
            return;
        } else if password != confirm_password {
            self.password_confirm_field
                .update(cx, |field, cx| field.flash_error(window, cx));
            return;
        }

        let password = password.to_string();

        self.state = PasswordChangeState::Processing;
        cx.notify();

        let uiaa_client_entity = self.uiaa_client.clone();
        let session_manager = cx.global::<SessionManager>();
        let client = session_manager.client().unwrap().read(cx).clone();

        cx.spawn(
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                if let Err(e) = cx
                    .spawn_tokio(async move {
                        client.account().change_password(&password, auth_data).await
                    })
                    .await
                {
                    if let Some(uiaa) = e.as_uiaa_response() {
                        uiaa_client_entity.update(cx, |uiaa_client, cx| {
                            uiaa_client.set_uiaa_info(uiaa.clone(), cx);
                            cx.notify()
                        });
                    } else {
                        error!("Failed to change password: {:?}", e);
                        weak_this
                            .update(cx, |this, cx| {
                                this.error = Some(e);
                                this.state = PasswordChangeState::Confirm;
                                cx.notify();
                            })
                            .unwrap();
                    }
                } else {
                    weak_this
                        .update(cx, |this, cx| {
                            this.complete_password_change(cx);
                        })
                        .unwrap();
                }
            },
        )
        .detach();
    }

    fn complete_password_change(&mut self, cx: &mut Context<Self>) {
        self.clear_password_fields(cx);

        self.state = PasswordChangeState::Complete;
        cx.notify();
    }

    fn clear_password_fields(&mut self, cx: &mut Context<Self>) {
        self.password_field.update(cx, |text_field, cx| {
            text_field.set_text("");
        });
        self.password_confirm_field.update(cx, |text_field, cx| {
            text_field.set_text("");
        });
        self.error = None;
    }
}

impl Render for PasswordChangeSurface {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>();
        let session_manager = cx.global::<SessionManager>();

        // Stop rendering here because we shouldn't get to see this page
        if session_manager.client().is_none() {
            return div().into_any_element();
        }

        surface()
            .child(
                div()
                    .size_full()
                    .child(
                        pager(
                            "password-change-pager",
                            match self.state {
                                PasswordChangeState::Confirm => 0,
                                PasswordChangeState::Processing => 1,
                                PasswordChangeState::Complete => 2,
                            },
                        )
                        .size_full()
                        .flex_grow()
                        .animation(SlideHorizontalAnimation::new())
                        .page(
                            div()
                                .bg(theme.background)
                                .w_full()
                                .h_full()
                                .flex()
                                .flex_col()
                                .gap(px(4.))
                                .child(
                                    grandstand("password-change-grandstand")
                                        .text(tr!("PASSWORD_CHANGE", "Change Password"))
                                        .pt(px(36.))
                                        .on_back_click(cx.listener(|this, _, window, cx| {
                                            this.clear_password_fields(cx);

                                            (this.on_surface_change)(
                                                &SurfaceChangeEvent {
                                                    change: SurfaceChange::Pop,
                                                },
                                                window,
                                                cx,
                                            )
                                        })),
                                )
                                .child(
                                    constrainer("password-change")
                                        .flex()
                                        .flex_col()
                                        .w_full()
                                        .p(px(8.))
                                        .child(
                                            layer()
                                                .flex()
                                                .flex_col()
                                                .p(px(8.))
                                                .gap(px(8.))
                                                .w_full()
                                                .child(subtitle(tr!("PASSWORD_CHANGE")))
                                                .child(tr!(
                                                    "PASSWORD_CHANGE_DEVICE_LOGOUT_MESSAGE",
                                                    "After changing your password, we'll \
                                                    log you out of all your other devices."
                                                ))
                                                .child(tr!(
                                                    "PASSWORD_CHANGE_DESCRIPTION",
                                                    "Make it a good password and save it for this \
                                                    account. You don't want to be reusing this \
                                                    password."
                                                ))
                                                .child(self.password_field.clone())
                                                .child(self.password_confirm_field.clone())
                                                .child(
                                                    button("password-change-button")
                                                        .child(icon_text(
                                                            "edit-rename",
                                                            tr!("PASSWORD_CHANGE"),
                                                        ))
                                                        .on_click(cx.listener(
                                                            |this, _, window, cx| {
                                                                this.perform_password_change(
                                                                    None, window, cx,
                                                                )
                                                            },
                                                        )),
                                                )
                                                .when_some(self.error.as_ref(), |david, error| {
                                                    david.child(
                                                        admonition()
                                                            .severity(AdmonitionSeverity::Error)
                                                            .title(tr!(
                                                                "PASSWORD_CHANGE_ERROR",
                                                                "Unable to change your password"
                                                            ))
                                                            .child(format!("{}", error)),
                                                    )
                                                }),
                                        ),
                                ),
                        )
                        .page(
                            div()
                                .size_full()
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(spinner()),
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
                                    grandstand("password-change-grandstand")
                                        .text(tr!("PASSWORD_CHANGE"))
                                        .pt(px(36.)),
                                )
                                .child(
                                    constrainer("password-change")
                                        .flex()
                                        .flex_col()
                                        .w_full()
                                        .p(px(8.))
                                        .child(
                                            layer()
                                                .flex()
                                                .flex_col()
                                                .p(px(8.))
                                                .gap(px(8.))
                                                .w_full()
                                                .child(subtitle(tr!(
                                                    "PASSWORD_CHANGE_COMPLETE",
                                                    "Password Changed"
                                                )))
                                                .child(tr!(
                                                    "PASSWORD_CHANGE_COMPLETE_DESCRIPTION",
                                                    "Your password was changed."
                                                ))
                                                .child(
                                                    button("password-change-ok")
                                                        .child(icon_text(
                                                            "dialog-ok",
                                                            tr!("DONE", "Done"),
                                                        ))
                                                        .on_click(cx.listener(
                                                            |this, _, window, cx| {
                                                                (this.on_surface_change)(
                                                                    &SurfaceChangeEvent {
                                                                        change: SurfaceChange::Pop,
                                                                    },
                                                                    window,
                                                                    cx,
                                                                );
                                                                this.state =
                                                                    PasswordChangeState::Confirm;
                                                                cx.notify();
                                                            },
                                                        )),
                                                ),
                                        ),
                                )
                                .into_any_element(),
                        ),
                    )
                    .child(self.uiaa_client.clone()),
            )
            .into_any_element()
    }
}
