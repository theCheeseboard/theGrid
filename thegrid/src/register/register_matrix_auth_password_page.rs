use crate::auth::auth_surface::AuthSurface;
use crate::main_window::{SurfaceChange, SurfaceChangeEvent};
use crate::register::register_surface::{HomeserverOrServerUrl, RegisterSurface};
use cntp_i18n::tr;
use contemporary::components::button::button;
use contemporary::components::constrainer::constrainer;
use contemporary::components::dialog_box::{StandardButton, dialog_box};
use contemporary::components::grandstand::grandstand;
use contemporary::components::icon_text::icon_text;
use contemporary::components::layer::layer;
use contemporary::components::pager::pager;
use contemporary::components::pager::slide_horizontal_animation::SlideHorizontalAnimation;
use contemporary::components::popover::popover;
use contemporary::components::spinner::spinner;
use contemporary::components::subtitle::subtitle;
use contemporary::components::text_field::{MaskMode, TextField};
use contemporary::styling::theme::{Theme, ThemeStorage};
use contemporary::surface::surface;
use gpui::prelude::FluentBuilder;
use gpui::{
    App, AppContext, BorrowAppContext, Context, ElementId, Entity, InteractiveElement, IntoElement,
    ParentElement, Render, RenderOnce, Styled, Window, div, img, px, rgb,
};
use matrix_sdk::OwnedServerName;
use matrix_sdk::encryption::CrossSigningResetAuthType;
use thegrid_common::session::session_manager::SessionManager;

pub struct RegisterMatrixAuthPasswordPage {
    register_surface: Entity<RegisterSurface>,
    username_field: Entity<TextField>,
    password_field: Entity<TextField>,
    confirm_password_field: Entity<TextField>,
}

impl RegisterMatrixAuthPasswordPage {
    pub fn new(
        cx: &mut Context<Self>,
        parent: Entity<RegisterSurface>,
    ) -> RegisterMatrixAuthPasswordPage {
        RegisterMatrixAuthPasswordPage {
            register_surface: parent,
            username_field: cx.new(|cx| {
                let mut text_field = TextField::new("homeserver", cx);
                text_field.set_placeholder(tr!("USERNAME", "Username").to_string().as_str());
                text_field
            }),
            password_field: cx.new(|cx| {
                let mut text_field = TextField::new("homeserver", cx);
                text_field.set_mask_mode(MaskMode::password_mask());
                text_field.set_placeholder(tr!("PASSWORD").to_string().as_str());
                text_field
            }),
            confirm_password_field: cx.new(|cx| {
                let mut text_field = TextField::new("homeserver", cx);
                text_field.set_mask_mode(MaskMode::password_mask());
                text_field.set_placeholder(tr!("PASSWORD_CONFIRM").to_string().as_str());
                text_field
            }),
        }
    }
}

impl Render for RegisterMatrixAuthPasswordPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        div()
            .bg(theme.background)
            .size_full()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(
                grandstand("register-homeserver-grandstand")
                    .text(tr!("REGISTER_TITLE"))
                    .pt(px(36.))
                    .on_back_click(cx.listener(|this, _, window, cx| {
                        this.register_surface.update(cx, |register_surface, cx| {
                            register_surface.back(window, cx);
                        })
                    })),
            )
            .child(
                constrainer("content")
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
                                "OPEN_ACCOUNT_MATRIX_AUTH_TITLE",
                                "User Details"
                            )))
                            .child(tr!(
                                "NEW_USERNAME_DESCRIPTION",
                                "Choose a username for your account."
                            ))
                            .child(self.username_field.clone().into_any_element())
                            .child(tr!(
                                "PASSWORD_DESCRIPTION",
                                "Make it a good password and save it for this account. \
                                You don't want to be reusing this password."
                            ))
                            .child(self.password_field.clone().into_any_element())
                            .child(self.confirm_password_field.clone().into_any_element())
                            .child(
                                button("next")
                                    .child(icon_text("go-next".into(), tr!("NEXT", "Next").into()))
                                    .on_click(cx.listener(|this, _, window, cx| {})),
                            ),
                    ),
            )
            .into_any_element()
    }
}
