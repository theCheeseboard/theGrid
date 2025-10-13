use cntp_i18n::tr;
use contemporary::components::button::button;
use contemporary::components::dialog_box::{StandardButton, dialog_box};
use contemporary::components::icon_text::icon_text;
use gpui::prelude::FluentBuilder;
use gpui::{
    App, AsyncApp, Context, IntoElement, ParentElement, Render, Styled, WeakEntity, Window, div,
};
use matrix_sdk::ruma::api::client::uiaa::{AuthData, AuthType, FallbackAcknowledgement, UiaaInfo};
use matrix_sdk::ruma::api::{OutgoingRequest, SendAccessToken};
use std::rc::Rc;
use thegrid::session::session_manager::SessionManager;
use thegrid::tokio_helper::TokioHelper;
use url::Url;

use matrix_sdk::ruma::api::client::uiaa::get_uiaa_fallback_page::v3::Request as UiaaFallbackRequest;

pub type SendAuthDataHandler = dyn Fn(&SendAuthDataEvent, &mut Window, &mut App) + 'static;
pub type CancelAuthenticationHandler =
    dyn Fn(&CancelAuthenticationEvent, &mut Window, &mut App) + 'static;

#[derive(Clone)]
pub struct SendAuthDataEvent {
    pub auth_data: Option<AuthData>,
}

#[derive(Clone)]
pub struct CancelAuthenticationEvent;

pub struct UiaaClient {
    uiaa_info: Option<UiaaInfo>,
    send_auth_data_handler: Rc<Box<SendAuthDataHandler>>,
    cancel_authentication_handler: Rc<Box<CancelAuthenticationHandler>>,
    current_step: CurrentStep,
    uiaa_step_completed: bool,
}

enum CurrentStep {
    None,
    BrowserAuth(Url),
    Error,
}

impl UiaaClient {
    pub fn new(
        send_auth_data_handler: impl Fn(&SendAuthDataEvent, &mut Window, &mut App) + 'static,
        cancel_authentication_handler: impl Fn(&CancelAuthenticationEvent, &mut Window, &mut App)
        + 'static,
        cx: &mut App,
    ) -> Self {
        Self {
            uiaa_info: None,
            send_auth_data_handler: Rc::new(Box::new(send_auth_data_handler)),
            cancel_authentication_handler: Rc::new(Box::new(cancel_authentication_handler)),
            current_step: CurrentStep::None,
            uiaa_step_completed: false,
        }
    }

    pub fn set_uiaa_info(&mut self, uiaa_info: UiaaInfo, _: &mut Window, cx: &mut Context<Self>) {
        let session_manager = cx.global::<SessionManager>();
        let client = session_manager.client().unwrap().read(cx).clone();

        let first_flow = uiaa_info.flows.first().cloned();
        let this_step = first_flow
            .and_then(|first_flow| first_flow.stages.get(uiaa_info.completed.len()).cloned());
        let uiaa_session = uiaa_info.session.clone();

        match this_step {
            Some(auth_type) if uiaa_session.is_some() => {
                self.uiaa_step_completed = false;

                let uiaa_session = uiaa_session.unwrap();
                cx.spawn(
                    async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                        let client_clone = client.clone();
                        let Ok(supported_versions) = cx
                            .spawn_tokio(async move { client_clone.supported_versions().await })
                            .await
                        else {
                            weak_this
                                .update(cx, |this, cx| {
                                    this.current_step = CurrentStep::Error;
                                    cx.notify();
                                })
                                .unwrap();
                            return;
                        };

                        let request = UiaaFallbackRequest::new(auth_type.clone(), uiaa_session);
                        let Ok(http_request) = request.try_into_http_request::<Vec<u8>>(
                            client.homeserver().as_str(),
                            SendAccessToken::None,
                            &supported_versions,
                        ) else {
                            weak_this
                                .update(cx, |this, cx| {
                                    this.current_step = CurrentStep::Error;
                                    cx.notify();
                                })
                                .unwrap();
                            return;
                        };

                        let Ok(url) = http_request.uri().to_string().parse::<Url>() else {
                            weak_this
                                .update(cx, |this, cx| {
                                    this.current_step = CurrentStep::Error;
                                    cx.notify();
                                })
                                .unwrap();
                            return;
                        };

                        weak_this
                            .update(cx, |this, cx| {
                                this.current_step = CurrentStep::BrowserAuth(url);
                                cx.notify();
                            })
                            .unwrap();
                    },
                )
                .detach();
            }
            _ => {
                self.current_step = CurrentStep::Error;
            }
        };
        self.uiaa_info = Some(uiaa_info);
    }

    pub fn clear_uuia_info(&mut self) {
        self.uiaa_info = None;
        self.current_step = CurrentStep::None;
    }

    fn cancel_authentication(&mut self, window: &mut Window, cx: &mut App) {
        (self.cancel_authentication_handler)(&CancelAuthenticationEvent, window, cx);
        self.clear_uuia_info();
    }

    fn send_auth_data(&mut self, auth_data: Option<AuthData>, window: &mut Window, cx: &mut App) {
        (self.send_auth_data_handler)(&SendAuthDataEvent { auth_data }, window, cx);
        self.clear_uuia_info();
    }
}

impl Render for UiaaClient {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let uiaa_session = self
            .uiaa_info
            .as_ref()
            .and_then(|uiaa_info| uiaa_info.session.clone());
        match &self.current_step {
            CurrentStep::None => div().into_any_element(),
            CurrentStep::BrowserAuth(url) => {
                let url = url.clone();
                dialog_box("uiaa-dialog")
                    .visible(true)
                    .title(tr!("AUTH_REQUIRED", "Authentication Required").into())
                    .content(
                        div()
                            .flex()
                            .flex_col()
                            .child(tr!(
                                "UIAA_BROWSER_AUTH",
                                "To continue, complete authentication in your web browser."
                            ))
                            .child(
                                button("open-browser-button")
                                    .child(icon_text(
                                        "text-html".into(),
                                        tr!("UIAA_BROWSER_OPEN", "Open Web Browser").into(),
                                    ))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        cx.open_url(url.as_str());
                                        this.uiaa_step_completed = true;
                                    })),
                            ),
                    )
                    .standard_button(
                        StandardButton::Cancel,
                        cx.listener(|this, _, window, cx| {
                            this.cancel_authentication(window, cx);
                        }),
                    )
                    .button(
                        button("continue-button")
                            .child(icon_text(
                                "arrow-right".into(),
                                tr!("AUTH_REQUIRED_BROWSER_GO", "Continue").into(),
                            ))
                            .when(!self.uiaa_step_completed, |david| david.disabled())
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.send_auth_data(
                                    Some(AuthData::FallbackAcknowledgement(
                                        FallbackAcknowledgement::new(uiaa_session.clone().unwrap()),
                                    )),
                                    window,
                                    cx,
                                );
                            })),
                    )
                    .into_any_element()
            }
            CurrentStep::Error => dialog_box("uiaa-dialog")
                .visible(true)
                .title(tr!("AUTH_REQUIRED", "Authentication Required").into())
                .content(tr!(
                    "UIAA_ERROR",
                    "There was a problem authenticating with the homeserver."
                ))
                .standard_button(
                    StandardButton::Sorry,
                    cx.listener(|this, _, window, cx| {
                        this.cancel_authentication(window, cx);
                    }),
                )
                .into_any_element(),
        }
    }
}
