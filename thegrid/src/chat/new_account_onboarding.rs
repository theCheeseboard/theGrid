use crate::uiaa_client::{SendAuthDataEvent, UiaaClient};
use cntp_i18n::tr;
use contemporary::components::admonition::AdmonitionSeverity;
use contemporary::components::button::button;
use contemporary::components::icon_text::icon_text;
use contemporary::components::layer::layer;
use contemporary::components::pager::pager;
use contemporary::components::pager::slide_horizontal_animation::SlideHorizontalAnimation;
use contemporary::components::spinner::spinner;
use contemporary::components::toast::Toast;
use contemporary::styling::theme::ThemeStorage;
use gpui::{
    div, px, App, AsyncApp, AsyncWindowContext, Entity, IntoElement, ParentElement,
    RenderOnce, Styled, TextAlign, Window,
};
use matrix_sdk::ruma::api::client::uiaa::AuthData;
use thegrid_common::session::session_manager::SessionManager;
use thegrid_common::tokio_helper::TokioHelper;

#[derive(IntoElement)]
pub struct NewAccountOnboarding {}

pub fn new_account_onboarding() -> NewAccountOnboarding {
    NewAccountOnboarding {}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AccountOnboardingState {
    Idle,
    EncryptionSetup,
}

impl AccountOnboardingState {
    pub fn page(&self) -> usize {
        match self {
            AccountOnboardingState::Idle => 0,
            AccountOnboardingState::EncryptionSetup => 1,
        }
    }
}

impl RenderOnce for NewAccountOnboarding {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let session_manager = cx.global::<SessionManager>();
        let Some(client) = session_manager.client() else {
            return div().into_any_element();
        };
        let client = client.read(cx).clone();

        let state = window.use_state(cx, |_, _| AccountOnboardingState::Idle);
        let uiaa_client = window.use_state(cx, |window, cx| {
            UiaaClient::new(
                cx.listener({
                    let state = state.clone();
                    move |this, event: &SendAuthDataEvent, window, cx| {
                        bootstrap_cross_signing(
                            event.auth_data.clone(),
                            cx.entity(),
                            state.clone(),
                            window,
                            cx,
                        );
                    }
                }),
                {
                    let state = state.clone();
                    move |event, window, cx| {
                        state.write(cx, AccountOnboardingState::Idle);
                    }
                },
                cx,
            )
        });

        let theme = cx.theme();

        pager("new-account-onboarding-pager", state.read(cx).page())
            .animation(SlideHorizontalAnimation::new())
            .size_full()
            .page(
                div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(4.))
                    .p(px(4.))
                    .text_align(TextAlign::Center)
                    .child(
                        div()
                            .text_size(theme.heading_font_size)
                            .child(tr!("NEW_ACCOUNT_ONBOARDING_TITLE", "Welcome to Matrix")),
                    )
                    .child(tr!(
                        "NEW_ACCOUNT_ONBOARDING_DESCRIPTION",
                        "This is your Matrix ID. It is your identifier on the Matrix network, and \
                        anyone can contact you using this ID."
                    ))
                    .child(
                        layer()
                            .p(px(4.))
                            .text_size(theme.heading_font_size)
                            .child(client.user_id().unwrap().to_string()),
                    )
                    .child(
                        button("continue-button")
                            .child(icon_text(
                                "go-next",
                                tr!("NEW_ACCOUNT_ONBOARDING_START", "Start Chatting"),
                            ))
                            .on_click({
                                let state = state.clone();
                                let uiaa_client = uiaa_client.clone();
                                move |_, window, cx| {
                                    bootstrap_cross_signing(
                                        None,
                                        uiaa_client.clone(),
                                        state.clone(),
                                        window,
                                        cx,
                                    );
                                }
                            }),
                    )
                    .into_any_element(),
            )
            .page(
                div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(8.))
                    .child(spinner())
                    .child(tr!(
                        "NEW_ACCOUNT_ONBOARDING_CROSS_SIGNING_BOOTSTRAP",
                        "Encryption setup in progress..."
                    ))
                    .into_any_element(),
            )
            .into_any_element()
    }
}

fn bootstrap_cross_signing(
    auth_data: Option<AuthData>,
    uiaa_client: Entity<UiaaClient>,
    state: Entity<AccountOnboardingState>,
    window: &mut Window,
    cx: &mut App,
) {
    state.write(cx, AccountOnboardingState::EncryptionSetup);

    let session_manager = cx.global::<SessionManager>();
    let client = session_manager.client().unwrap().read(cx).clone();
    window
        .spawn(cx, {
            let weak_state = state.downgrade();
            async move |cx: &mut AsyncWindowContext| {
                if let Err(error) = cx
                    .spawn_tokio(async move {
                        client.encryption().bootstrap_cross_signing(auth_data).await
                    })
                    .await
                {
                    if let Some(response) = error.as_uiaa_response() {
                        uiaa_client.update(cx, |uiaa_client, cx| {
                            uiaa_client.set_uiaa_info(response.clone(), cx);
                        });
                    } else {
                        // Error
                        let _ = cx.update(|window, cx| {
                            let _ = weak_state.update(cx, |state, cx| {
                                *state = AccountOnboardingState::Idle;
                                cx.notify();
                            });

                            Toast::new()
                                .severity(AdmonitionSeverity::Error)
                                .title(tr!(
                                    "NEW_ACCOUNT_ONBOARDING_CROSS_SIGNING_BOOTSTRAP_ERROR_TITLE",
                                    "Unable to complete encryption setup"
                                ).to_string().as_str())
                                .body(tr!(
                                    "NEW_ACCOUNT_ONBOARDING_CROSS_SIGNING_BOOTSTRAP_ERROR_MESSAGE",
                                    "Please try again, or contact your homeserver provider."
                                ).to_string().as_str())
                                .post(window, cx);
                        });
                    }
                } else {
                    let _ = cx.update_global::<SessionManager, _>(|session_manager, _, cx| {
                        session_manager.clear_new_account_flag();

                        let _ = weak_state.update(cx, |state, cx| {
                            *state = AccountOnboardingState::Idle;
                            cx.notify();
                        });
                    });
                }
            }
        })
        .detach();
}
