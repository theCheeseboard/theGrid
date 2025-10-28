use crate::utilities::default_device_name;
use cntp_i18n::{i18n_manager, tr};
use contemporary::application::Details;
use contemporary::components::button::button;
use contemporary::components::constrainer;
use contemporary::components::constrainer::constrainer;
use contemporary::components::grandstand::grandstand;
use contemporary::components::icon_text::icon_text;
use contemporary::components::layer::layer;
use contemporary::components::popover::popover;
use contemporary::components::spinner::spinner;
use contemporary::components::subtitle::subtitle;
use contemporary::components::text_field::{MaskMode, TextField};
use contemporary::surface::surface;
use gpui::LineFragment::Text;
use gpui::http_client::anyhow;
use gpui::prelude::FluentBuilder;
use gpui::{
    App, AppContext, AsyncApp, BorrowAppContext, Context, ElementId, Entity, InteractiveElement,
    IntoElement, ParentElement, Render, Styled, WeakEntity, Window, div, img, px,
};
use gpui_tokio::Tokio;
use matrix_sdk::authentication::matrix::MatrixSession;
use matrix_sdk::config::SyncSettings;
use matrix_sdk::encryption::CrossSigningStatus;
use matrix_sdk::ruma::api::client::session::get_login_types::v3::{IdentityProvider, LoginType};
use matrix_sdk::ruma::{DeviceId, OwnedUserId, user_id};
use matrix_sdk::{Client, ClientBuildError};
use smol::future::FutureExt;
use std::path::PathBuf;
use std::sync::Arc;
use thegrid::session::session_manager::SessionManager;
use thegrid::tokio_helper::TokioHelper;
use tracing::{error, info};
use url::Url;
use uuid::Uuid;

#[derive(Clone, Debug)]
enum AuthState {
    Idle,
    Advanced,
    Connecting,
    ConnectionError,
    AuthRequired,
    SsoTokenRequired(IdentityProvider),
}

enum LoginMethod {
    Password(String),
    SsoToken(String),
}

pub struct AuthSurface {
    matrix_id_field: Entity<TextField>,
    password_field: Entity<TextField>,
    token_field: Entity<TextField>,
    username_field: Entity<TextField>,
    homeserver_field: Entity<TextField>,
    state: AuthState,
    client: Option<Client>,
    login_types: Vec<LoginType>,
    user_id: Option<OwnedUserId>,
    session_uuid: Uuid,
}

impl AuthSurface {
    pub fn new(cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            cx.observe_global::<SessionManager>(|session_manager, cx| cx.notify())
                .detach();

            let surface = Self {
                matrix_id_field: cx.new(|cx| {
                    let mut text_field = TextField::new("matrix-id", cx);
                    text_field.set_placeholder(
                        tr!("AUTH_MATRIX_ID_EXAMPLE", "@user:example.org")
                            .to_string()
                            .as_str(),
                    );
                    text_field
                }),
                password_field: cx.new(|cx| {
                    let mut text_field = TextField::new("password", cx);
                    text_field.set_mask_mode(MaskMode::password_mask());
                    text_field.set_placeholder(
                        tr!("AUTH_PASSWORD_PLACEHOLDER", "Password")
                            .to_string()
                            .as_str(),
                    );
                    text_field
                }),
                token_field: cx.new(|cx| {
                    let mut text_field = TextField::new("token", cx);
                    text_field.set_mask_mode(MaskMode::password_mask());
                    text_field.set_placeholder(
                        tr!("AUTH_TOKEN_PLACEHOLDER", "Token").to_string().as_str(),
                    );
                    text_field
                }),
                username_field: cx.new(|cx| {
                    let mut text_field = TextField::new("username", cx);
                    text_field.set_placeholder(
                        tr!("AUTH_USERNAME_PLACEHOLDER", "Username")
                            .to_string()
                            .as_str(),
                    );
                    text_field
                }),
                homeserver_field: cx.new(|cx| {
                    let mut text_field = TextField::new("homeserver", cx);
                    text_field.set_placeholder(
                        tr!("AUTH_HOMESERVER_PLACEHOLDER", "Homeserver")
                            .to_string()
                            .as_str(),
                    );
                    text_field
                }),
                state: AuthState::Idle,
                client: None,
                user_id: None,
                login_types: Vec::new(),
                session_uuid: Uuid::new_v4(),
            };
            surface
        })
    }

    fn session_dir(&self, cx: &mut App) -> PathBuf {
        let details = cx.global::<Details>();
        let directories = details.standard_dirs().unwrap();
        let data_dir = directories.data_dir();
        let session_dir = data_dir.join("sessions");
        session_dir.join(self.session_uuid.to_string())
    }

    fn login_clicked(&mut self, cx: &mut Context<Self>) {
        let username = self.matrix_id_field.read(cx).text();
        let user_id = user_id::UserId::parse(username);
        let Ok(user_id) = user_id else {
            error!("user_id not okay");
            return;
        };
        self.user_id = Some(user_id.clone());

        let session_dir = self.session_dir(cx);
        let store_dir = session_dir.join("store");

        std::fs::create_dir_all(&store_dir).unwrap();

        cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let client = cx
                .spawn_tokio(async move {
                    Client::builder()
                        .server_name(user_id.server_name())
                        .sqlite_store(store_dir, None)
                        .build()
                        .await
                })
                .await;

            match client {
                Ok(client) => {
                    let client_clone = client.clone();
                    let login_types = cx
                        .spawn_tokio(
                            async move { client_clone.matrix_auth().get_login_types().await },
                        )
                        .await;

                    match login_types {
                        Ok(login_types) => {
                            this.update(cx, |this, cx| {
                                if !matches!(this.state, AuthState::Connecting) {
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
                                if !matches!(this.state, AuthState::Connecting) {
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
                        if !matches!(this.state, AuthState::Connecting) {
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

    fn advanced_login_clicked(&mut self, cx: &mut Context<Self>) {
        self.state = AuthState::Advanced;
        cx.notify();
    }

    fn trigger_advanced_login(&mut self, cx: &mut Context<Self>) {
        let session_dir = self.session_dir(cx);
        let store_dir = session_dir.join("store");
        let homeserver_url = self.homeserver_field.read(cx).text();
        let Ok(homeserver_url) = homeserver_url
            .parse::<Url>()
            .or_else(|_| format!("https://{homeserver_url}/").parse::<Url>())
        else {
            error!("Unable to parse homeserver URL");
            return;
        };

        std::fs::create_dir_all(&store_dir).unwrap();

        cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let client = cx
                .spawn_tokio(async move {
                    Client::builder()
                        .homeserver_url(&homeserver_url)
                        .sqlite_store(store_dir, None)
                        .build()
                        .await
                })
                .await;

            match client {
                Ok(client) => {
                    let client_clone = client.clone();
                    let login_types = cx
                        .spawn_tokio(
                            async move { client_clone.matrix_auth().get_login_types().await },
                        )
                        .await;

                    match login_types {
                        Ok(login_types) => {
                            this.update(cx, |this, cx| {
                                if !matches!(this.state, AuthState::Connecting) {
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
                                if !matches!(this.state, AuthState::Connecting) {
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
                        if !matches!(this.state, AuthState::Connecting) {
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
        let password = self.password_field.read(cx).text().to_string();
        self.perform_login(LoginMethod::Password(password), cx);
    }

    fn trigger_sso_login(&mut self, idp: IdentityProvider, cx: &mut Context<Self>) {
        let client = self.client.clone().unwrap();
        let client_clone = client.clone();

        let idp_clone = idp.clone();

        cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let requested_url = Tokio::spawn_result(cx, async move {
                client_clone
                    .matrix_auth()
                    .get_sso_login_url(
                        "https://thegrid.vicr123.com/idp-signin",
                        Some(idp.id.as_str()),
                    )
                    // .initial_device_display_name(default_device_name.as_str())
                    // .send()
                    .await
                    .map_err(|e| anyhow!(e))
            })
            .unwrap()
            .await;

            cx.update(|cx| cx.open_url(&requested_url.unwrap()))
                .unwrap();
        })
        .detach();

        self.state = AuthState::SsoTokenRequired(idp_clone);
        cx.notify();
    }

    fn trigger_sso_token_login(&mut self, cx: &mut Context<Self>) {
        let sso_token = self.token_field.read(cx).text().to_string();
        self.perform_login(LoginMethod::SsoToken(sso_token), cx);
    }

    fn perform_login(&mut self, login_method: LoginMethod, cx: &mut Context<Self>) {
        let session_dir = self.session_dir(cx);
        let default_device_name = default_device_name(cx);
        let client = self.client.clone().unwrap();
        let client_clone = client.clone();
        let username = self
            .user_id
            .clone()
            .map(|user_id| user_id.localpart().to_string())
            .unwrap_or_else(|| self.username_field.read(cx).text().to_string());
        let session_uuid = self.session_uuid;

        cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let login_response = Tokio::spawn_result(cx, async move {
                match login_method {
                    LoginMethod::Password(password) => client_clone
                        .matrix_auth()
                        .login_username(username, password.as_str()),
                    LoginMethod::SsoToken(sso_token) => {
                        client_clone.matrix_auth().login_token(&sso_token)
                    }
                }
                .initial_device_display_name(default_device_name.as_str())
                .send()
                .await
                .map_err(|e| anyhow!(e))
            })
            .unwrap()
            .await;

            // Start sync to ensure we have the latest state
            let client_clone = client.clone();
            let sync_handle = Tokio::spawn_result(cx, async move {
                client_clone
                    .sync_once(SyncSettings::new().ignore_timeout_on_first_sync(true))
                    .await
                    .map_err(|e| anyhow!(e))
            })
            .unwrap()
            .await;

            match login_response {
                Ok(login_response) => {
                    let matrix_session: MatrixSession = (&login_response.clone()).into();

                    let session_file = session_dir.join("session.json");
                    std::fs::write(
                        session_file,
                        serde_json::to_string(&matrix_session).unwrap(),
                    )
                    .unwrap();

                    let homeserver_file = session_dir.join("homeserver");
                    std::fs::write(homeserver_file, client.homeserver().to_string()).unwrap();

                    cx.update_global::<SessionManager, ()>(|session_manager, cx| {
                        session_manager.set_session(session_uuid, cx);
                    })
                    .unwrap();

                    this.update(cx, |this, cx| {
                        if !matches!(this.state, AuthState::Connecting) {
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
                        if !matches!(this.state, AuthState::Connecting) {
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

    fn render_constrainer_child(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        match &self.state {
            AuthState::Idle => div().into_any_element(),
            AuthState::Advanced => layer()
                .flex()
                .flex_col()
                .p(px(8.))
                .w_full()
                .child(subtitle(tr!(
                    "AUTH_POPOVER_ADVANCED_LOGIN",
                    "Advanced Login"
                )))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .child(tr!(
                            "AUTH_POPOVER_ADVANCED_LOGIN_TEXT",
                            "If your homeserver doesn't support server discovery, you can enter \
                            its URL here to log in."
                        ))
                        .child(self.homeserver_field.clone().into_any_element())
                        .child(self.username_field.clone().into_any_element())
                        .child(
                            button("advanced-login-popover-login")
                                .child(icon_text("dialog-ok".into(), tr!("AUTH_LOG_IN").into()))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.trigger_advanced_login(cx);
                                })),
                        ),
                )
                .into_any_element(),
            AuthState::Connecting => div()
                .flex()
                .items_center()
                .justify_center()
                .size_full()
                .child(spinner())
                .into_any_element(),
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
                            button("login-popover-connection-error-ok")
                                .child(icon_text("dialog-ok".into(), tr!("SORRY", "Sorry").into()))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.state = AuthState::Idle;
                                    cx.notify()
                                })),
                        ),
                )
                .into_any_element(),
            AuthState::AuthRequired => {
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .when(
                        self.login_types
                            .iter()
                            .any(|login_type| matches!(login_type, LoginType::Password(_))),
                        |david| {
                            david.child(
                                layer()
                                    .flex()
                                    .flex_col()
                                    .p(px(8.))
                                    .w_full()
                                    .gap(px(6.))
                                    .child(subtitle(tr!("AUTH_PASSWORD", "Password Login")))
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
                        },
                    )
                    .when(
                        self.login_types
                            .iter()
                            .any(|login_type| matches!(login_type, LoginType::Sso(_))),
                        |david| {
                            let sso_providers =
                                self.login_types
                                    .iter()
                                    .flat_map(|login_type| match login_type {
                                        LoginType::Sso(sso) => sso.identity_providers.clone(),
                                        _ => Vec::new(),
                                    });

                            david.child(
                                sso_providers.fold(
                                    layer()
                                        .flex()
                                        .flex_col()
                                        .p(px(8.))
                                        .w_full()
                                        .child(subtitle(tr!("AUTH_SSO", "Single Sign-on"))),
                                    |david, sso_provider| {
                                        david.child(
                                            button(ElementId::Name(
                                                format!("sso-provider-{}", sso_provider.id).into(),
                                            ))
                                            .child(icon_text(
                                                "arrow-right".into(),
                                                tr!(
                                                    "AUTH_SSO_BUTTON",
                                                    "Log in with {{sso_provider}}",
                                                    sso_provider = sso_provider.name
                                                )
                                                .into(),
                                            ))
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.trigger_sso_login(sso_provider.clone(), cx);
                                            })),
                                        )
                                    },
                                ),
                            )
                        },
                    )
                    .into_any_element()
            }
            AuthState::SsoTokenRequired(idp) => div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .when(
                    self.login_types
                        .iter()
                        .any(|login_type| matches!(login_type, LoginType::Password(_))),
                    |david| {
                        david.child(
                            layer()
                                .flex()
                                .flex_col()
                                .p(px(8.))
                                .w_full()
                                .gap(px(6.))
                                .child(subtitle(tr!(
                                    "AUTH_SSO_NAME",
                                    "Login with {{idp_name}}",
                                    idp_name = idp.name
                                )))
                                .child(tr!(
                                    "AUTH_SSO_MESSAGE",
                                    "We've opened a browser. Go ahead and log in there, \
                                    and come back when you're done."
                                ))
                                .child(self.token_field.clone().into_any_element())
                                .child(
                                    div().flex().child(div().flex_grow()).child(
                                        button("log_in_button")
                                            .child(icon_text(
                                                "arrow-right".into(),
                                                tr!("AUTH_LOG_IN").into(),
                                            ))
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.trigger_sso_token_login(cx);
                                            })),
                                    ),
                                )
                                .into_any_element(),
                        )
                    },
                )
                .into_any_element(),
        }
    }
}

impl Render for AuthSurface {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let locale = &i18n_manager!().locale;
        let details = cx.global::<Details>();
        let session_manager = cx.global::<SessionManager>();

        let sessions = session_manager.sessions(cx);

        div().size_full().key_context("AuthSurface").child(
            surface().child(
                div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
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
                    .when(!sessions.is_empty(), |david| {
                        david.child(
                            sessions.iter().fold(
                                layer()
                                    .p(px(8.))
                                    .w(px(400.))
                                    .flex()
                                    .flex_col()
                                    .gap(px(8.))
                                    .child(subtitle(tr!(
                                        "AUTH_SESSION_RESTORE",
                                        "Use existing login"
                                    ))),
                                |layer, session| {
                                    let uuid = session.uuid;
                                    layer.child(
                                        button(ElementId::Name(
                                            format!("session-{}", session.uuid).into(),
                                        ))
                                        .child(session.matrix_session.meta.user_id.to_string())
                                        .on_click(
                                            cx.listener(move |this, _, _, cx| {
                                                cx.update_global::<SessionManager, ()>(
                                                    |session_manager, cx| {
                                                        session_manager.set_session(uuid, cx);
                                                    },
                                                )
                                            }),
                                        ),
                                    )
                                },
                            ),
                        )
                    })
                    .child(
                        layer()
                            .p(px(8.))
                            .w(px(400.))
                            .flex()
                            .flex_col()
                            .gap(px(8.))
                            .child(subtitle(tr!("AUTH_LOG_IN_TO_MATRIX", "Log in to Matrix")))
                            .child(self.matrix_id_field.clone().into_any_element())
                            .child(
                                div()
                                    .flex()
                                    .gap(px(4.))
                                    .child(div().flex_grow())
                                    .child(
                                        button("advanced_log_in")
                                            .child(tr!("AUTH_ADVANCED_LOG_IN", "Advanced Login..."))
                                            .flat()
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.advanced_login_clicked(cx)
                                            })),
                                    )
                                    .child(
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
                            .visible(!matches!(self.state, AuthState::Idle))
                            .size_neg(100.)
                            .anchor_bottom()
                            .content(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(9.))
                                    .child(
                                        grandstand("login-popover-grandstand")
                                            .when_some(self.user_id.clone(), |david, user_id| {
                                                david.text(tr!(
                                                    "POPOVER_LOGIN_HOMESERVER",
                                                    "Log in to {{homeserver}}",
                                                    homeserver = user_id.server_name().to_string()
                                                ))
                                            })
                                            .when_none(&self.user_id, |david| {
                                                david.text(tr!("POPOVER_LOGIN", "Log in"))
                                            })
                                            .on_back_click(cx.listener(|this, _, _, cx| {
                                                this.client = None;
                                                this.state = AuthState::Idle;
                                                cx.notify()
                                            })),
                                    )
                                    .child(
                                        constrainer("login-popover-constrainer")
                                            .child(self.render_constrainer_child(window, cx)),
                                    ),
                            ),
                    ),
            ),
        )
    }
}
