use crate::register::register_surface::{HomeserverOrServerUrl, RegisterSurface};
use cntp_i18n::tr;
use contemporary::components::button::button;
use contemporary::components::constrainer::constrainer;
use contemporary::components::grandstand::grandstand;
use contemporary::components::icon_text::icon_text;
use contemporary::components::layer::layer;
use contemporary::components::subtitle::subtitle;
use contemporary::components::text_field::TextField;
use contemporary::styling::theme::ThemeStorage;
use gpui::prelude::FluentBuilder;
use gpui::{
    div, px, AppContext, Context,
    Entity, IntoElement, ParentElement, Render, Styled, Window,
};
use matrix_sdk::OwnedServerName;

pub struct RegisterHomeserverPage {
    register_surface: Entity<RegisterSurface>,
    homeserver_field: Entity<TextField>,
}

impl RegisterHomeserverPage {
    pub fn new(cx: &mut Context<Self>, parent: Entity<RegisterSurface>) -> RegisterHomeserverPage {
        RegisterHomeserverPage {
            register_surface: parent,
            homeserver_field: cx.new(|cx| {
                let mut text_field = TextField::new("homeserver", cx);
                text_field.set_placeholder("matrix.org");
                text_field
            }),
        }
    }
}

impl Render for RegisterHomeserverPage {
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
                    .text(tr!("REGISTER_TITLE", "Open an account"))
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
                                "OPEN_ACCOUNT_HOMESERVER_TITLE",
                                "Welcome to Matrix!"
                            )))
                            .child(tr!(
                                "OPEN_ACCOUNT_TITLE_HOMESERVER_DESCRIPTION",
                                "Choose a homeserver to open your account on."
                            ))
                            // TODO: Help button
                            .child(self.homeserver_field.clone().into_any_element())
                            .child(
                                button("next")
                                    .child(icon_text("go-next".into(), tr!("NEXT", "Next").into()))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        let homeserver = OwnedServerName::try_from(
                                            this.homeserver_field.read(cx).text(),
                                        )
                                        .map(|homeserver| {
                                            HomeserverOrServerUrl::Homeserver(homeserver)
                                        })
                                        .unwrap_or(HomeserverOrServerUrl::ServerUrl(
                                            this.homeserver_field.read(cx).text().to_string(),
                                        ));
                                        this.register_surface.update(cx, |register_surface, cx| {
                                            register_surface.provide_homeserver(homeserver, cx);
                                        });
                                    })),
                            ),
                    ),
            )
            .into_any_element()
    }
}
