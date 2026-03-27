use cntp_i18n::tr;
use contemporary::components::button::button;
use contemporary::components::dialog_box::{dialog_box, StandardButton};
use contemporary::components::icon_text::icon_text;
use gpui::prelude::FluentBuilder;
use gpui::{
    AsyncApp, Context, IntoElement, ParentElement, Render, SharedString, WeakEntity, Window,
};
use matrix_sdk::authentication::oauth::AccountManagementActionFull;
use matrix_sdk::ruma::html::RemoveReplyFallback::No;
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
        action: AccountManagementActionFull,
        cx: &mut Context<Self>,
    ) -> bool {
        let session_manager = cx.global::<SessionManager>();
        if !session_manager
            .current_account()
            .read(cx)
            .supports_account_management_action(action.action_type())
        {
            // Don't open the dialog because the homeserver doesn't support this action.
            return false;
        }

        (self.title, self.text) = match action {
            AccountManagementActionFull::SessionEnd { .. } => (
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
            AccountManagementActionFull::AccountDeactivate => (
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
            AccountManagementActionFull::CrossSigningReset => (
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

        let client = session_manager.client().unwrap().read(cx).clone();

        cx.spawn(
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                let Ok(Some(account_management_url)) = cx
                    .spawn_tokio(async move { client.oauth().account_management_url().await })
                    .await
                else {
                    // TODO: Signal error
                    return;
                };

                let url = account_management_url.action(action).build();
                let _ = weak_this.update(cx, |this, cx| this.continue_url = Some(url));
            },
        )
        .detach();

        self.continue_url = None;
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
                        "dialog-ok".into(),
                        tr!(
                            "OAUTH_MANAGEMENT_PAGE_REDIRECT_DIALOG_CONTINUE",
                            "Continue in Browser"
                        )
                        .into(),
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
