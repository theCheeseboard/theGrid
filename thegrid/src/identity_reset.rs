use crate::main_window::{SurfaceChange, SurfaceChangeEvent, SurfaceChangeHandler};
use crate::uiaa_client::{CancelAuthenticationEvent, SendAuthDataEvent, UiaaClient};
use cntp_i18n::tr;
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
use contemporary::styling::theme::Theme;
use contemporary::surface::surface;
use gpui::prelude::FluentBuilder;
use gpui::{
    App, AppContext, AsyncApp, AsyncWindowContext, Context, Entity, IntoElement, ParentElement,
    Render, Styled, WeakEntity, Window, div, px,
};
use matrix_sdk::encryption::CrossSigningResetAuthType;
use matrix_sdk::encryption::recovery::{IdentityResetHandle, RecoveryError};
use matrix_sdk::ruma::api::client::uiaa::AuthData;
use std::rc::Rc;
use thegrid::session::session_manager::SessionManager;
use tracing::{Id, error};

pub struct IdentityResetSurface {
    state: IdentityResetState,
    handle: Option<IdentityResetHandle>,
    error: Option<RecoveryError>,

    uiaa_client: Entity<UiaaClient>,

    on_surface_change: Rc<Box<SurfaceChangeHandler>>,
}

enum IdentityResetState {
    Confirm,
    Processing,
    Complete,
}

impl IdentityResetSurface {
    pub fn new(
        cx: &mut App,
        on_surface_change: impl Fn(&SurfaceChangeEvent, &mut Window, &mut App) + 'static,
    ) -> Entity<Self> {
        cx.new(|cx| {
            let send_auth_data_listener =
                cx.listener(|this: &mut Self, event: &SendAuthDataEvent, _, cx| {
                    this.continue_handle(event.auth_data.clone(), cx);
                });
            let cancel_auth_listener = cx.listener(|this, _: &CancelAuthenticationEvent, _, cx| {
                this.cancel_handle(cx);
            });

            let uiaa_client =
                cx.new(|cx| UiaaClient::new(send_auth_data_listener, cancel_auth_listener, cx));

            Self {
                state: IdentityResetState::Confirm,
                handle: None,
                error: None,
                uiaa_client,

                on_surface_change: Rc::new(Box::new(on_surface_change)),
            }
        })
    }

    fn perform_identity_reset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.state = IdentityResetState::Processing;
        cx.notify();

        let session_manager = cx.global::<SessionManager>();
        let client = session_manager.client().unwrap().read(cx).clone();
        let encryption = client.encryption();
        let recovery = encryption.recovery();

        let weak_this = cx.entity().downgrade();
        window
            .spawn(cx, async move |cx: &mut AsyncWindowContext| {
                match recovery.reset_identity().await {
                    Ok(Some(handle)) => {
                        cx.window_handle()
                            .update(cx, |_, window, cx| {
                                weak_this
                                    .update(cx, |this, cx| {
                                        if let CrossSigningResetAuthType::Uiaa(uiaa) =
                                            handle.auth_type()
                                        {
                                            this.uiaa_client.update(cx, |uiaa_client, cx| {
                                                uiaa_client.set_uiaa_info(uiaa.clone(), cx);
                                                cx.notify();
                                            });
                                        }
                                        this.handle = Some(handle);
                                        cx.notify();
                                    })
                                    .unwrap();
                            })
                            .unwrap();
                    }
                    Ok(None) => {
                        // Crypto identity was reset successfully
                        weak_this
                            .update(cx, |this, cx| {
                                this.state = IdentityResetState::Complete;
                                cx.notify();
                            })
                            .unwrap();
                    }
                    Err(e) => {
                        error!("Failed to reset crypto identity: {e:?}");
                        weak_this
                            .update(cx, |this, cx| {
                                this.error = Some(e);
                                this.state = IdentityResetState::Confirm;
                                cx.notify();
                            })
                            .unwrap();
                    }
                }
            })
            .detach();
    }

    fn cancel_handle(&mut self, cx: &mut Context<Self>) {
        let handle = self.handle.take();

        cx.spawn(
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                if let Some(handle) = handle {
                    handle.cancel().await;
                }

                weak_this
                    .update(cx, |this, cx| {
                        this.state = IdentityResetState::Confirm;
                        cx.notify();
                    })
                    .unwrap();
            },
        )
        .detach();
    }

    fn continue_handle(&mut self, auth_data: Option<AuthData>, cx: &mut Context<Self>) {
        let Some(handle) = self.handle.take() else {
            return;
        };

        cx.spawn(
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                if let Err(e) = handle.reset(auth_data).await {
                    weak_this
                        .update(cx, |this, cx| {
                            this.error = Some(e);
                            this.state = IdentityResetState::Confirm;
                            cx.notify();
                        })
                        .unwrap();
                } else {
                    weak_this
                        .update(cx, |this, cx| {
                            this.state = IdentityResetState::Complete;
                            cx.notify();
                        })
                        .unwrap();
                }
            },
        )
        .detach();
    }
}

impl Render for IdentityResetSurface {
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
                            "identity-reset-pager",
                            match self.state {
                                IdentityResetState::Confirm => 0,
                                IdentityResetState::Processing => 1,
                                IdentityResetState::Complete => 2,
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
                                    grandstand("devices-grandstand")
                                        .text(tr!("IDENTITY_RESET", "Reset Identity"))
                                        .pt(px(36.))
                                        .on_back_click(cx.listener(|this, _, window, cx| {
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
                                    constrainer("devices")
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
                                            .child(subtitle(tr!("IDENTITY_RESET")))
                                            .child(tr!(
                                                "IDENTITY_RESET_DESCRIPTION",
                                                "This is a drastic action intended to be used if \
                                                you have lost your account's recovery methods, \
                                                if you have lost access to your verified devices, \
                                                or if your account is believed to be compromised."
                                            ))
                                            .child(layer().p(px(4.)).child(tr!(
                                                "IDENTITY_RESET_UPSHOT_1",
                                                "Anyone with whom you have verified will be \
                                                notified that your identity was reset"
                                            )))
                                            .child(layer().p(px(4.)).child(tr!(
                                                "IDENTITY_RESET_UPSHOT_2",
                                                "Your encryption backup will be erased, and you \
                                                may lose encrypted messages you have sent in \
                                                the past"
                                            )))
                                            .child(layer().p(px(4.)).child(tr!(
                                                "IDENTITY_RESET_UPSHOT_3",
                                                "You will need to verify all of your devices again"
                                            )))
                                            .child(tr!(
                                                "IDENTITY_RESET_DESCRIPTION_2",
                                                "Continue to reset your identity?"
                                            ))
                                            .child(
                                                button("reset-crypto-identity-button")
                                                    .destructive()
                                                    .child(icon_text(
                                                        "view-refresh".into(),
                                                        tr!("SECURITY_IDENTITY_RESET").into(),
                                                    ))
                                                    .on_click(cx.listener(
                                                        |this, _, window, cx| {
                                                            this.perform_identity_reset(window, cx)
                                                        },
                                                    )),
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
                                .bg(theme.background)
                                .w_full()
                                .h_full()
                                .flex()
                                .flex_col()
                                .gap(px(4.))
                                .child(
                                    grandstand("devices-grandstand")
                                        .text(tr!("IDENTITY_RESET"))
                                        .pt(px(36.)),
                                )
                                .child(
                                    constrainer("devices")
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
                                                    "IDENTITY_RESET_COMPLETE",
                                                    "Identity Reset Complete"
                                                )))
                                                .child(tr!(
                                                    "IDENTITY_RESET_COMPLETE_DESCRIPTION",
                                                    "Your cryptographic identity was reset."
                                                ))
                                                .child(
                                                    button("reset-crypto-identity-ok")
                                                        .child(icon_text(
                                                            "dialog-ok".into(),
                                                            tr!("DONE", "Done").into(),
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
                                                                    IdentityResetState::Confirm;
                                                                cx.notify();
                                                            },
                                                        )),
                                                ),
                                        ),
                                )
                                .into_any_element(),
                        ),
                    )
                    .when_some(self.handle.as_ref(), |david, handle| {
                        match handle.auth_type() {
                            CrossSigningResetAuthType::Uiaa(_) => david,
                            CrossSigningResetAuthType::OAuth(oauth) => {
                                let oauth_url = oauth.approval_url.as_str().to_string();
                                david.child(
                                    dialog_box("oauth")
                                        .visible(true)
                                        .title(tr!("AUTH_REQUIRED").into())
                                        .content(tr!(
                                            "AUTH_REQUIRED_OAUTH_DESCRIPTION",
                                            "To continue, authenticate yourself with Single Sign-on"
                                        ))
                                        .standard_button(
                                            StandardButton::Cancel,
                                            cx.listener(|this, _, _, cx| {
                                                this.cancel_handle(cx);
                                            }),
                                        )
                                        .button(
                                            button("continue-button")
                                                .child(tr!(
                                                    "AUTH_REQUIRED_OAUTH_GO",
                                                    "Continue with Single Sign-on"
                                                ))
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    cx.open_url(oauth_url.as_str());
                                                    this.continue_handle(None, cx);
                                                })),
                                        ),
                                )
                            }
                        }
                    })
                    .child(self.uiaa_client.clone()),
            )
            .into_any_element()
    }
}
