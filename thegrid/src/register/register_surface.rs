use crate::main_window::{
    MainWindowSurface, SurfaceChange, SurfaceChangeEvent, SurfaceChangeHandler,
};
use crate::register::register_homeserver_page::RegisterHomeserverPage;
use crate::register::register_matrix_auth_password_page::RegisterMatrixAuthPasswordPage;
use contemporary::components::pager::lift_animation::LiftAnimation;
use contemporary::components::pager::pager;
use contemporary::components::pager::slide_horizontal_animation::SlideHorizontalAnimation;
use contemporary::components::spinner::spinner;
use contemporary::surface::surface;
use gpui::{
    App, AppContext, AsyncApp, Context, Entity, IntoElement, ParentElement, Render, Styled,
    WeakEntity, Window, div, rgb,
};
use matrix_sdk::{Client, ClientBuildError, OwnedServerName, ServerName};
use std::rc::Rc;
use thegrid_common::tokio_helper::TokioHelper;

pub struct RegisterSurface {
    current_page: CurrentPage,

    on_surface_change: Rc<Box<SurfaceChangeHandler>>,

    homeserver_page: Entity<RegisterHomeserverPage>,
    matrix_auth_password_page: Entity<RegisterMatrixAuthPasswordPage>,
}

pub enum CurrentPage {
    Homeserver,
    ConnectingHomeserver,
    MatrixAuthPassword,
}

pub enum HomeserverOrServerUrl {
    Homeserver(OwnedServerName),
    ServerUrl(String),
}

impl RegisterSurface {
    pub fn new(
        cx: &mut Context<Self>,
        on_surface_change: impl Fn(&SurfaceChangeEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        let this_entity = cx.entity();

        RegisterSurface {
            current_page: CurrentPage::Homeserver,
            on_surface_change: Rc::new(Box::new(move |event, window, cx| {
                on_surface_change(event, window, cx)
            })),
            homeserver_page: cx.new(|cx| RegisterHomeserverPage::new(cx, this_entity.clone())),
            matrix_auth_password_page: cx
                .new(|cx| RegisterMatrixAuthPasswordPage::new(cx, this_entity.clone())),
        }
    }

    pub fn back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.current_page {
            CurrentPage::Homeserver => {
                (self.on_surface_change)(
                    &SurfaceChangeEvent {
                        change: SurfaceChange::Pop,
                    },
                    window,
                    cx,
                );
            }
            CurrentPage::ConnectingHomeserver => {
                // noop
            }
            CurrentPage::MatrixAuthPassword => {
                self.current_page = CurrentPage::Homeserver;
                cx.notify();
            }
        }
    }

    pub fn provide_homeserver(
        &mut self,
        homeserver: HomeserverOrServerUrl,
        cx: &mut Context<Self>,
    ) {
        self.current_page = CurrentPage::ConnectingHomeserver;

        cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let client = cx
                .spawn_tokio(async move {
                    match homeserver {
                        HomeserverOrServerUrl::Homeserver(homeserver) => {
                            Client::builder().server_name(&homeserver)
                        }
                        HomeserverOrServerUrl::ServerUrl(server_url) => {
                            Client::builder().homeserver_url(&server_url)
                        }
                    }
                    .build()
                    .await
                })
                .await;

            let _ = this.update(cx, |this, cx| {
                match client {
                    Ok(client) => {
                        // TODO: Find registration methods
                        this.current_page = CurrentPage::MatrixAuthPassword;
                        cx.notify();
                    }
                    Err(_) => {
                        this.current_page = CurrentPage::Homeserver;
                        // TODO: Show error message
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }
}

impl Render for RegisterSurface {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        surface().child(
            div().size_full().child(
                pager(
                    "main-pager",
                    match self.current_page {
                        CurrentPage::Homeserver => 0,
                        CurrentPage::ConnectingHomeserver => 1,
                        CurrentPage::MatrixAuthPassword => 2,
                    },
                )
                .size_full()
                .animation(SlideHorizontalAnimation::new())
                .page(self.homeserver_page.clone().into_any_element())
                .page(
                    div()
                        .size_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(spinner())
                        .into_any_element(),
                )
                .page(self.matrix_auth_password_page.clone().into_any_element()),
            ),
        )
    }
}
