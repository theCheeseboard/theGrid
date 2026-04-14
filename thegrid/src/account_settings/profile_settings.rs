use crate::auth::oauth_management_page_redirect_dialog::OAuthManagementPageRedirectDialog;
use cntp_i18n::tr;
use contemporary::components::button::button;
use contemporary::components::constrainer::constrainer;
use contemporary::components::dialog_box::{StandardButton, dialog_box};
use contemporary::components::grandstand::grandstand;
use contemporary::components::icon_text::icon_text;
use contemporary::components::layer::layer;
use contemporary::components::subtitle::subtitle;
use contemporary::components::text_field::TextField;
use contemporary::styling::theme::{Theme, VariableColor};
use gpui::prelude::FluentBuilder;
use gpui::{
    App, AppContext, AsyncApp, Context, Entity, IntoElement, ParentElement, Render, Styled,
    WeakEntity, Window, div, px,
};
use matrix_sdk::AuthApi;
use matrix_sdk::authentication::oauth::AccountManagementActionFull;
use matrix_sdk::ruma::api::client::discovery::get_authorization_server_metadata::v1::AccountManagementAction;
use std::rc::Rc;
use thegrid_common::mxc_image::{SizePolicy, mxc_image};
use thegrid_common::session::session_manager::SessionManager;
use thegrid_common::surfaces::{
    MainWindowSurface, SurfaceChange, SurfaceChangeEvent, SurfaceChangeHandler,
};
use thegrid_common::tokio_helper::TokioHelper;
use thegrid_rtc_livekit::call_disconnect_confirmation_dialog::CallDisconnectConfirmationDialog;
use url::Url;

pub struct ProfileSettings {
    edit_display_name_open: bool,
    edit_profile_picture_open: bool,
    new_display_name_text_field: Entity<TextField>,
    on_surface_change: Rc<Box<SurfaceChangeHandler>>,
    call_disconnect_confirmation_dialog: Entity<CallDisconnectConfirmationDialog>,
    oauth_management_page_redirect_dialog: Entity<OAuthManagementPageRedirectDialog>,

    current_client_settings: Option<Url>,
    account_management_url: Option<Url>,
}

impl ProfileSettings {
    pub fn new(
        cx: &mut Context<Self>,
        on_surface_change: impl Fn(&SurfaceChangeEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        let call_disconnect_confirmation_dialog =
            cx.new(|cx| CallDisconnectConfirmationDialog::new(cx));
        let oauth_management_page_redirect_dialog =
            cx.new(|cx| OAuthManagementPageRedirectDialog::new(cx));

        cx.observe_global::<SessionManager>(|this, cx| {
            let session_manager = cx.global::<SessionManager>();
            let Some(client) = session_manager.client() else {
                this.current_client_settings = None;
                this.account_management_url = None;
                cx.notify();
                return;
            };

            let client = client.read(cx).clone();
            if this
                .current_client_settings
                .as_ref()
                .is_none_or(|url| url != &client.homeserver())
            {
                // Update the current information for the homeserver
                this.current_client_settings = Some(client.homeserver());

                if let Some(AuthApi::OAuth(oauth)) = client.auth_api() {
                    cx.spawn(
                        async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                            let Ok(Some(account_management_url)) = cx
                                .spawn_tokio(async move { oauth.account_management_url().await })
                                .await
                            else {
                                return;
                            };

                            let profile_management_url = account_management_url
                                .action(AccountManagementActionFull::Profile)
                                .build();
                            let _ = weak_this.update(cx, |this, cx| {
                                this.account_management_url = Some(profile_management_url);
                                cx.notify();
                            });
                        },
                    )
                    .detach();
                }
            }
        })
        .detach();

        Self {
            edit_display_name_open: false,
            edit_profile_picture_open: false,

            new_display_name_text_field: cx.new(|cx| {
                let mut text_field = TextField::new("new-display-name", cx);
                text_field.set_placeholder(
                    tr!("NEW_DISPLAY_NAME_PLACEHOLDER", "New Display Name")
                        .to_string()
                        .as_str(),
                );
                text_field
            }),
            on_surface_change: Rc::new(Box::new(on_surface_change)),
            call_disconnect_confirmation_dialog,
            oauth_management_page_redirect_dialog,

            current_client_settings: None,
            account_management_url: None,
        }
    }

    fn open_deactivate_account_page(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let on_complete = cx.listener(|this, _, window, cx| {
            // Try to go through the homeserver management page first
            if !this
                .oauth_management_page_redirect_dialog
                .update(cx, |dialog, cx| {
                    dialog.perform_action(AccountManagementActionFull::AccountDeactivate, cx)
                })
            {
                (this.on_surface_change)(
                    &SurfaceChangeEvent {
                        change: SurfaceChange::Push(MainWindowSurface::DeactivateAccount),
                    },
                    window,
                    cx,
                );
                cx.notify();
            }
        });

        self.call_disconnect_confirmation_dialog.update(
            cx,
            |call_disconnect_confirmation_dialog, cx| {
                call_disconnect_confirmation_dialog.ensure_calls_disconnected(
                    window,
                    cx,
                    on_complete,
                );
            },
        )
    }
}

impl Render for ProfileSettings {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>();
        let session_manager = cx.global::<SessionManager>();

        let client = session_manager.client().unwrap().read(cx);
        let account = session_manager.current_account().read(cx);
        let session = session_manager.current_session().unwrap();

        let display_name = account.display_name().unwrap_or_default();

        div()
            .bg(theme.background)
            .w_full()
            .h_full()
            .flex()
            .flex_col()
            .child(
                grandstand("profile-grandstand")
                    .text(tr!("ACCOUNT_SETTINGS_PROFILE"))
                    .pt(px(36.)),
            )
            .child(
                constrainer("profile")
                    .flex()
                    .flex_col()
                    .w_full()
                    .p(px(8.))
                    .gap(px(8.))
                    .child(
                        div()
                            .flex()
                            .gap(px(4.))
                            .child(
                                mxc_image(account.avatar_url())
                                    .fallback_image(client.user_id().unwrap())
                                    .rounded(theme.border_radius)
                                    .fixed_square(px(48.))
                                    .size_policy(SizePolicy::Fit),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .justify_center()
                                    .gap(px(4.))
                                    .child(account.display_name().unwrap_or_default())
                                    .child(div().text_color(theme.foreground.disabled()).child(
                                        session.secrets.session_meta().unwrap().user_id.to_string(),
                                    )),
                            ),
                    )
                    .when_some(self.account_management_url.clone(), |david, url| {
                        david.child(
                            layer()
                                .flex()
                                .flex_col()
                                .p(px(8.))
                                .w_full()
                                .child(subtitle(tr!("PROFILE_MANAGE_ACCOUNT", "Manage Account")))
                                .child(tr!(
                                    "PROFILE_MANAGE_ACCOUNT_DESCRIPTION",
                                    "Proceed to your homeserver to see and configure \
                                    additional settings."
                                ))
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .bg(theme.button_background)
                                        .rounded(theme.border_radius)
                                        .child(
                                            button("account-manage")
                                                .child(icon_text(
                                                    "configure",
                                                    tr!("PROFILE_MANAGE_ACCOUNT"),
                                                ))
                                                .on_click(cx.listener(
                                                    move |this, _, window, cx| {
                                                        cx.open_url(url.as_str())
                                                    },
                                                )),
                                        ),
                                ),
                        )
                    })
                    .child(
                        layer()
                            .flex()
                            .flex_col()
                            .p(px(8.))
                            .w_full()
                            .child(subtitle(tr!("PROFILE_PROFILE", "Profile")))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .bg(theme.button_background)
                                    .rounded(theme.border_radius)
                                    .child(
                                        button("profile-change-display-name")
                                            .child(icon_text(
                                                "edit-rename",
                                                tr!(
                                                    "PROFILE_CHANGE_DISPLAY_NAME",
                                                    "Change Display Name"
                                                ),
                                            ))
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.new_display_name_text_field.update(
                                                    cx,
                                                    |text_field, cx| {
                                                        text_field.set_text(display_name.as_str());
                                                    },
                                                );
                                                this.edit_display_name_open = true;
                                                cx.notify()
                                            })),
                                    )
                                    .child(button("profile-change-profile-picture").child(
                                        icon_text(
                                            "edit-rename",
                                            tr!(
                                                "PROFILE_CHANGE_PROFILE_PICTURE",
                                                "Change Profile Picture"
                                            ),
                                        ),
                                    )),
                            ),
                    )
                    .when(
                        match client.auth_api() {
                            Some(AuthApi::OAuth(_)) => {
                                let session_manager = cx.global::<SessionManager>();
                                session_manager
                                    .current_account()
                                    .read(cx)
                                    .supports_account_management_action(
                                        AccountManagementAction::AccountDeactivate,
                                    )
                            }
                            Some(AuthApi::Matrix(_)) => true,
                            _ => false,
                        },
                        |david| {
                            david.child(
                                layer()
                                    .flex()
                                    .flex_col()
                                    .p(px(8.))
                                    .w_full()
                                    .child(subtitle(tr!("PROFILE_ACCOUNT", "Account")))
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .bg(theme.button_background)
                                            .rounded(theme.border_radius)
                                            .child(
                                                button("profile-deactivate")
                                                    .child(icon_text(
                                                        "list-remove",
                                                        tr!(
                                                            "PROFILE_DEACTIVATE",
                                                            "Deactivate Account"
                                                        ),
                                                    ))
                                                    .destructive()
                                                    .on_click(cx.listener(
                                                        move |this, _, window, cx| {
                                                            this.open_deactivate_account_page(
                                                                window, cx,
                                                            )
                                                        },
                                                    )),
                                            ),
                                    ),
                            )
                        },
                    ),
            )
            .child(
                dialog_box("edit-display-name")
                    .visible(self.edit_display_name_open)
                    .title(tr!("PROFILE_CHANGE_DISPLAY_NAME"))
                    .content(
                        div()
                            .flex()
                            .flex_col()
                            .w(px(500.))
                            .gap(px(12.))
                            .child(tr!(
                                "PROFILE_CHANGE_DISPLAY_NAME_DESCRIPTION",
                                "Your Display Name is shown to other users to identify you"
                            ))
                            .child(self.new_display_name_text_field.clone().into_any_element()),
                    )
                    .standard_button(
                        StandardButton::Cancel,
                        cx.listener(|this, _, _, cx| {
                            this.edit_display_name_open = false;
                            cx.notify()
                        }),
                    )
                    .button(
                        button("change-profile-picture-button")
                            .child(icon_text("dialog-ok", tr!("PROFILE_CHANGE_DISPLAY_NAME")))
                            .on_click(cx.listener(|this, _, _, cx| {
                                let new_display_name =
                                    this.new_display_name_text_field.read(cx).text().to_string();
                                let session_manager = cx.global::<SessionManager>();
                                let client = session_manager.client().unwrap().read(cx).clone();
                                cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                                    if cx
                                        .spawn_tokio(async move {
                                            client
                                                .account()
                                                .set_display_name(Some(new_display_name.as_str()))
                                                .await
                                        })
                                        .await
                                        .is_err()
                                    {
                                        this.update(cx, |this, cx| {
                                            // TODO: Show the error
                                            cx.notify()
                                        })
                                    } else {
                                        this.update(cx, |this, cx| {
                                            this.edit_display_name_open = false;
                                            cx.notify()
                                        })
                                    }
                                })
                                .detach();
                                cx.notify()
                            })),
                    ),
            )
            .child(self.call_disconnect_confirmation_dialog.clone())
            .child(self.oauth_management_page_redirect_dialog.clone())
    }
}
