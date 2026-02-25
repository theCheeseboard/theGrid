use crate::main_window::{
    MainWindowSurface, SurfaceChange, SurfaceChangeEvent, SurfaceChangeHandler,
};
use crate::mxc_image::{SizePolicy, mxc_image};
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
    WeakEntity, Window, div, px, rgb,
};
use std::rc::Rc;
use thegrid_common::session::session_manager::SessionManager;
use thegrid_common::tokio_helper::TokioHelper;

pub struct ProfileSettings {
    edit_display_name_open: bool,
    edit_profile_picture_open: bool,
    new_display_name_text_field: Entity<TextField>,
    on_surface_change: Rc<Box<SurfaceChangeHandler>>,
}

impl ProfileSettings {
    pub fn new(
        cx: &mut App,
        on_surface_change: impl Fn(&SurfaceChangeEvent, &mut Window, &mut App) + 'static,
    ) -> Entity<Self> {
        cx.new(|cx| Self {
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
        })
    }
}

impl Render for ProfileSettings {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>();
        let session_manager = cx.global::<SessionManager>();

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
                                    .rounded(theme.border_radius)
                                    .size(px(48.))
                                    .size_policy(SizePolicy::Fit),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .justify_center()
                                    .gap(px(4.))
                                    .child(account.display_name().unwrap_or_default())
                                    .child(
                                        div()
                                            .text_color(theme.foreground.disabled())
                                            .child(session.matrix_session.meta.user_id.to_string()),
                                    ),
                            ),
                    )
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
                                                "edit-rename".into(),
                                                tr!(
                                                    "PROFILE_CHANGE_DISPLAY_NAME",
                                                    "Change Display Name"
                                                )
                                                .into(),
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
                                    .child(
                                        button("profile-change-profile-picture").child(icon_text(
                                            "edit-rename".into(),
                                            tr!(
                                                "PROFILE_CHANGE_PROFILE_PICTURE",
                                                "Change Profile Picture"
                                            )
                                            .into(),
                                        )),
                                    ),
                            ),
                    )
                    .child(
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
                                                "list-remove".into(),
                                                tr!("PROFILE_DEACTIVATE", "Deactivate Account")
                                                    .into(),
                                            ))
                                            .destructive()
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                (this.on_surface_change)(
                                                    &SurfaceChangeEvent {
                                                        change: SurfaceChange::Push(
                                                            MainWindowSurface::DeactivateAccount,
                                                        ),
                                                    },
                                                    window,
                                                    cx,
                                                );
                                                cx.notify();
                                            })),
                                    ),
                            ),
                    ),
            )
            .child(
                dialog_box("edit-display-name")
                    .visible(self.edit_display_name_open)
                    .title(tr!("PROFILE_CHANGE_DISPLAY_NAME").into())
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
                            .child(icon_text(
                                "dialog-ok".into(),
                                tr!("PROFILE_CHANGE_DISPLAY_NAME").into(),
                            ))
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
    }
}
