use crate::utilities::default_device_name;
use cntp_i18n::{i18n_manager, tr};
use contemporary::application::Details;
use contemporary::components::button::button;
use contemporary::components::constrainer::constrainer;
use contemporary::components::grandstand::grandstand;
use contemporary::components::icon_text::icon_text;
use contemporary::components::layer::layer;
use contemporary::components::popover::popover;
use contemporary::components::spinner::spinner;
use contemporary::components::subtitle::subtitle;
use contemporary::components::text_field::TextField;
use contemporary::surface::surface;
use gpui::LineFragment::Text;
use gpui::http_client::anyhow;
use gpui::prelude::FluentBuilder;
use gpui::{
    App, AppContext, AsyncApp, Context, ElementId, Entity, InteractiveElement, IntoElement,
    ParentElement, Render, Styled, WeakEntity, Window, div, img, px,
};
use gpui_tokio::Tokio;
use matrix_sdk::authentication::matrix::MatrixSession;
use matrix_sdk::ruma::api::client::session::get_login_types::v3::LoginType;
use matrix_sdk::ruma::{OwnedUserId, user_id};
use matrix_sdk::{Client, ClientBuildError};
use smol::future::FutureExt;
use std::sync::Arc;
use tracing::{error, info};

#[derive(Clone, Debug, PartialEq)]
enum AuthState {
    Idle,
    Connecting,
    ConnectionError,
    AuthRequired,
}

pub struct AuthSurface {
    matrix_id_field: Entity<TextField>,
    password_field: Entity<TextField>,
    state: AuthState,
    client: Option<Client>,
    login_types: Vec<LoginType>,
    user_id: Option<OwnedUserId>,
}

impl AuthSurface {
    pub fn new(cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let surface = Self {
                matrix_id_field: TextField::new(
                    cx,
                    "matrix_id",
                    "".into(),
                    tr!("AUTH_MATRIX_ID_EXAMPLE", "@user:example.org").into(),
                ),
                password_field: TextField::new(
                    cx,
                    "password",
                    "".into(),
                    tr!("AUTH_PASSWORD_PLACEHOLDER", "Password").into(),
                ),
                state: AuthState::Idle,
                client: None,
                user_id: None,
                login_types: Vec::new(),
            };
            surface.password_field.update(cx, |this, cx| {
                this.password_field(cx, true);
                cx.notify();
            });
            surface
        })
    }

    fn login_clicked(&mut self, cx: &mut Context<Self>) {
        let username = self.matrix_id_field.read(cx).current_text(cx);
        let user_id = user_id::UserId::parse(username.as_str());
        let Ok(user_id) = user_id else {
            error!("user_id not okay");
            return;
        };
        self.user_id = Some(user_id.clone());

        let details = cx.global::<Details>();
        let directories = details.standard_dirs().unwrap();
        let data_dir = directories.data_dir();
        let store_dir = data_dir.join("store");

        std::fs::create_dir_all(&store_dir).unwrap();

        cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let client = Tokio::spawn_result(cx, async move {
                Client::builder()
                    .server_name(user_id.server_name())
                    .sqlite_store(store_dir.join("database.db"), None)
                    .build()
                    .await
                    .map_err(|e| anyhow!(e))
            })
            .unwrap()
            .await;

            match client {
                Ok(client) => {
                    let client_clone = client.clone();
                    let login_types = Tokio::spawn_result(cx, async move {
                        client_clone
                            .matrix_auth()
                            .get_login_types()
                            .await
                            .map_err(|e| anyhow!(e))
                    })
                    .unwrap()
                    .await;

                    match login_types {
                        Ok(login_types) => {
                            this.update(cx, |this, cx| {
                                if this.state != AuthState::Connecting {
                                    return;
                                }

                                this.client = Some(client);
                                this.login_types = login_types.flows;
                                this.state = AuthState::AuthRequired;
                                cx.notify();
                            })
                            .unwrap();
                        }
                        Err(e) => {
                            this.update(cx, |this, cx| {
                                if this.state != AuthState::Connecting {
                                    return;
                                }

                                this.state = AuthState::ConnectionError;
                                error!("Unable to create client");
                                cx.notify();
                            })
                            .unwrap();
                        }
                    }
                }
                Err(e) => {
                    this.update(cx, |this, cx| {
                        if this.state != AuthState::Connecting {
                            return;
                        }

                        this.state = AuthState::ConnectionError;
                        error!("Unable to create client");
                        cx.notify();
                    })
                    .unwrap();
                }
            }
        })
        .detach();

        self.state = AuthState::Connecting;
        cx.notify();
    }

    fn login_password_clicked(&mut self, cx: &mut Context<Self>) {
        let default_device_name = default_device_name(cx);
        let client = self.client.clone().unwrap();
        let client_clone = client.clone();
        let user_id = self.user_id.clone().unwrap().clone();
        let password = self.password_field.read(cx).current_text(cx).to_string();

        cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let login_response = Tokio::spawn_result(cx, async move {
                client_clone
                    .matrix_auth()
                    .login_username(user_id.localpart(), password.as_str())
                    .initial_device_display_name(default_device_name.as_str())
                    .send()
                    .await
                    .map_err(|e| anyhow!(e))
            })
            .unwrap()
            .await;

            match login_response {
                Ok(login_response) => {
                    let matrix_session: MatrixSession = (&login_response.clone()).into();

                    this.update(cx, |this, cx| {
                        if this.state != AuthState::Connecting {
                            return;
                        }

                        this.state = AuthState::Idle;
                        info!("Logged in");
                        cx.notify();
                    })
                    .unwrap();
                }
                Err(e) => {
                    this.update(cx, |this, cx| {
                        if this.state != AuthState::Connecting {
                            return;
                        }

                        this.state = AuthState::AuthRequired;
                        error!("Unable to log in");
                        cx.notify();
                    })
                    .unwrap();
                }
            }
        })
        .detach();

        self.state = AuthState::Connecting;
        cx.notify();
    }
}

impl Render for AuthSurface {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let locale = &i18n_manager!().locale;
        let details = cx.global::<Details>();

        div().size_full().key_context("AuthSurface").child(
            surface().child(
                div()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        layer()
                            .p(px(16.))
                            .w(px(400.))
                            .flex()
                            .flex_col()
                            .gap(px(8.))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(12.))
                                    .child(img("contemporary-icon:/application").w(px(40.)))
                                    .child(
                                        div().text_size(px(35.)).child(
                                            details
                                                .generatable
                                                .application_name
                                                .resolve_languages_or_default(&locale.messages),
                                        ),
                                    ),
                            )
                            .child(tr!("AUTH_MATRIX_ID", "Matrix ID"))
                            .child(self.matrix_id_field.clone().into_any_element())
                            .child(
                                div().flex().child(div().flex_grow()).child(
                                    button("log_in_button")
                                        .child(icon_text(
                                            "arrow-right".into(),
                                            tr!("AUTH_LOG_IN", "Log In").into(),
                                        ))
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.login_clicked(cx);
                                        })),
                                ),
                            ),
                    )
                    .child(
                        popover("login-popover")
                            .visible(self.state != AuthState::Idle)
                            .size_neg(100.)
                            .anchor_bottom()
                            .content(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(9.))
                                    .child(
                                        grandstand("login-popover-grandstand")
                                            .text(tr!(
                                                "POPOVER_LOGIN",
                                                "Log in to {{homeserver}}",
                                                homeserver = self
                                                    .user_id
                                                    .clone()
                                                    .map(|user_id| user_id
                                                        .server_name()
                                                        .to_string())
                                                    .unwrap_or_default()
                                            ))
                                            .on_back_click(cx.listener(|this, _, _, cx| {
                                                this.client = None;
                                                this.state = AuthState::Idle;
                                                cx.notify()
                                            })),
                                    )
                                    .child(
                                        constrainer("login-popover-constrainer").child(match self
                                            .state
                                        {
                                            AuthState::Idle => div().into_any_element(),
                                            AuthState::Connecting => div().flex().items_center().justify_center().size_full().child(spinner()).into_any_element(),
                                            AuthState::ConnectionError => layer()
                                                .flex()
                                                .flex_col()
                                                .p(px(8.))
                                                .w_full()
                                                .child(subtitle(tr!(
                                                    "AUTH_POPOVER_CONNECTION_ERROR",
                                                    "Unable to connect to homeserver"
                                                )))
                                                .child(
                                                    div()
                                                        .flex()
                                                        .flex_col()
                                                        .gap(px(8.))
                                                        .child(tr!(
                                                            "AUTH_POPOVER_CONNECTION_ERROR_TEXT",
                                                            "Check your Matrix ID and try again."
                                                        ))
                                                        .child(
                                                            button(
                                                                "login-popover-connection-error-ok",
                                                            )
                                                                .child(icon_text(
                                                                    "dialog-ok".into(),
                                                                    tr!("SORRY", "Sorry").into(),
                                                                ))
                                                                .on_click(cx.listener(
                                                                    |this, _, _, cx| {
                                                                        this.state = AuthState::Idle;
                                                                        cx.notify()
                                                                    },
                                                                )),
                                                        ),
                                                )
                                                .into_any_element(),
                                            AuthState::AuthRequired => div()
                                                .flex()
                                                .flex_col().gap(px(8.))
                                                .when(self.login_types.iter().any(|login_type| matches!(login_type, LoginType::Password(_))), |david| {
                                                    david.child(
                                                        layer()
                                                            .flex()
                                                            .flex_col()
                                                            .p(px(8.))
                                                            .w_full()
                                                            .gap(px(6.))
                                                            .child(subtitle(tr!(
                                                                "AUTH_PASSWORD",
                                                                "Password Login"
                                                            )))
                                                            .child(self.password_field.clone().into_any_element())
                                                            .child(
                                                                div().flex().child(div().flex_grow()).child(
                                                                    button("log_in_button")
                                                                        .child(icon_text(
                                                                            "arrow-right".into(),
                                                                            tr!("AUTH_LOG_IN").into(),
                                                                        ))
                                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                                            this.login_password_clicked(cx);
                                                                        })),
                                                                ),
                                                            )
                                                            .into_any_element(),
                                                    )
                                                })
                                                .when(self.login_types.iter().any(|login_type| matches!(login_type, LoginType::Sso(_))), |david| {
                                                    let sso_providers = self.login_types.iter().flat_map(|login_type| match login_type {
                                                        LoginType::Sso(sso) => {
                                                            sso.identity_providers.clone()
                                                        },
                                                        _ => Vec::new()
                                                    });

                                                    david.child(sso_providers.fold(
                                                        layer()
                                                            .flex()
                                                            .flex_col()
                                                            .p(px(8.))
                                                            .w_full()
                                                            .child(subtitle(tr!(
                                                                "AUTH_SSO",
                                                                "Single Sign-on"
                                                            ))), |david, sso_provider| {
                                                            david.child(
                                                                button(ElementId::Name(
                                                                    format!("sso-provider-{}", sso_provider.id).into()
                                                                )).child(icon_text("arrow-right".into(), tr!("AUTH_SSO_BUTTON", "Log in with {{sso_provider}}", sso_provider=sso_provider.name).into())))
                                                        }
                                                    ))
                                                })
                                                .into_any_element(),
                                        }),
                                    ),
                            ),
                    ),
            ),
        )
    }
}
