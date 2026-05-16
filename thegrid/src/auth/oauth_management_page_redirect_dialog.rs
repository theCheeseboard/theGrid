use cntp_i18n::tr;
use contemporary::components::button::button;
use contemporary::components::dialog_box::{StandardButton, dialog_box};
use contemporary::components::icon_text::icon_text;
use gpui::prelude::FluentBuilder;
use gpui::{
    AsyncApp, Context, IntoElement, ParentElement, Render, SharedString, WeakEntity, Window,
};
use matrix_sdk::ruma::api::client::discovery::get_authorization_server_metadata::v1::{
    AccountManagementAction, AccountManagementActionData,
};
use thegrid_common::session::session_manager::SessionManager;
use thegrid_common::tokio_helper::TokioHelper;
use url::Url;

pub struct OAuthManagementPageRedirectDialog {
    visible: bool,
    continue_url: Option<Url>,
    title: SharedString,
    text: SharedString,
}

impl OAuthManagementPageRedirectDialog {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            visible: false,
            continue_url: None,
            title: Default::default(),
            text: Default::default(),
        }
    }

    pub fn perform_action(
        &mut self,
        action: AccountManagementActionData,
        cx: &mut Context<Self>,
    ) -> bool {
        let session_manager = cx.global::<SessionManager>();
        let oauth_metadata = session_manager.current_account().read(cx).oauth_metadata();
        if oauth_metadata.is_none_or(|oauth_metadata| {
            !oauth_metadata.is_account_management_action_supported(match action {
                AccountManagementActionData::Profile => &AccountManagementAction::Profile,
                AccountManagementActionData::DevicesList => &AccountManagementAction::DevicesList,
                AccountManagementActionData::DeviceView(_) => &AccountManagementAction::DeviceView,
                AccountManagementActionData::DeviceDelete(_) => {
                    &AccountManagementAction::DeviceDelete
                }
                AccountManagementActionData::AccountDeactivate => {
                    &AccountManagementAction::AccountDeactivate
                }
                AccountManagementActionData::CrossSigningReset => {
                    &AccountManagementAction::CrossSigningReset
                }
                _ => return false,
            })
        }) {
            // Don't open the dialog because the homeserver doesn't support this action.
            return false;
        }

        (self.title, self.text) = match action {
            AccountManagementActionData::DeviceDelete { .. } => (
                tr!(
                    "OAUTH_MANAGEMENT_ACTION_SESSION_END_TITLE",
                    "Forcibly log device out"
                )
                .into(),
                tr!(
                    "OAUTH_MANAGEMENT_ACTION_SESSION_END_TEXT",
                    "To forcibly log this device out, proceed on your homeserver."
                )
                .into(),
            ),
            AccountManagementActionData::AccountDeactivate => (
                tr!(
                    "OAUTH_MANAGEMENT_ACTION_ACCOUNT_DEACTIVATE_TITLE",
                    "Deactivate Account"
                )
                .into(),
                tr!(
                    "OAUTH_MANAGEMENT_ACTION_ACCOUNT_DEACTIVATE_TEXT",
                    "To deactivate your account, proceed on your homeserver."
                )
                .into(),
            ),
            AccountManagementActionData::CrossSigningReset => (
                tr!(
                    "OAUTH_MANAGEMENT_ACTION_CROSS_SIGNING_RESET_TITLE",
                    "Reset Cryptographic Identity"
                )
                .into(),
                tr!(
                    "OAUTH_MANAGEMENT_ACTION_CROSS_SIGNING_RESET_TEXT",
                    "To reset your cryptographic identity, proceed on your homeserver."
                )
                .into(),
            ),
            _ => (
                tr!(
                    "OAUTH_MANAGEMENT_ACTION_GENERIC_TITLE",
                    "Continue on your Homeserver"
                )
                .into(),
                tr!(
                    "OAUTH_MANAGEMENT_ACTION_GENERIC_TEXT",
                    "Proceed with this action on your homeserver."
                )
                .into(),
            ),
        };

        let url = oauth_metadata
            .unwrap()
            .account_management_url_with_action(action);

        self.continue_url = url;
        self.visible = true;
        cx.notify();

        true
    }
}

impl Render for OAuthManagementPageRedirectDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        dialog_box("oauth-management-page-redirect")
            .visible(self.visible)
            .title(self.title.clone())
            .content(self.text.clone())
            .standard_button(
                StandardButton::Cancel,
                cx.listener(|this, _, _, cx| {
                    this.visible = false;
                    cx.notify();
                }),
            )
            .button(
                button("proceed-button")
                    .child(icon_text(
                        "dialog-ok",
                        tr!(
                            "OAUTH_MANAGEMENT_PAGE_REDIRECT_DIALOG_CONTINUE",
                            "Continue in Browser"
                        ),
                    ))
                    .when_none(&self.continue_url, |david| david.disabled())
                    .on_click(cx.listener(|this, _, _, cx| {
                        cx.open_url(this.continue_url.as_ref().unwrap().as_str());

                        this.visible = false;
                        cx.notify();
                    })),
            )
    }
}
