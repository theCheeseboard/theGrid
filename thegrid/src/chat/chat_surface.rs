use crate::chat::main_chat_surface::{ChangeRoomHandler, MainChatSurface};
use crate::main_window::{SurfaceChangeEvent, SurfaceChangeHandler};
use cntp_i18n::tr;
use contemporary::application::Details;
use contemporary::components::application_menu::ApplicationMenu;
use contemporary::components::button::button;
use contemporary::components::icon_text::icon_text;
use contemporary::components::interstitial::interstitial;
use contemporary::components::pager::fade_animation::FadeAnimation;
use contemporary::components::pager::pager;
use contemporary::components::spinner::spinner;
use contemporary::styling::theme::Theme;
use contemporary::surface::surface;
use gpui::prelude::FluentBuilder;
use gpui::{
    App, AppContext, BorrowAppContext, Context, Entity, InteractiveElement, IntoElement, Menu,
    ParentElement, Render, Styled, Window, div, px,
};
use std::fs::remove_dir_all;
use std::rc::Rc;
use thegrid::session::error_handling::ClientError;
use thegrid::session::session_manager::SessionManager;

pub struct ChatSurface {
    application_menu: Entity<ApplicationMenu>,
    main_chat_surface: Entity<MainChatSurface>,
}

impl ChatSurface {
    pub fn new(
        cx: &mut App,
        on_surface_change: impl Fn(&SurfaceChangeEvent, &mut Window, &mut App) + 'static,
    ) -> Entity<ChatSurface> {
        cx.new(|cx| ChatSurface {
            application_menu: ApplicationMenu::new(
                cx,
                Menu {
                    name: "Application Menu".into(),
                    items: vec![],
                },
            ),

            main_chat_surface: MainChatSurface::new(cx, on_surface_change),
        })
    }
}

impl Render for ChatSurface {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>();
        let session_manager = cx.global::<SessionManager>();

        let Some(current_session) = session_manager.current_session() else {
            return div();
        };

        div().size_full().key_context("MainSurface").child(
            surface()
                .actions(
                    div()
                        .occlude()
                        .flex()
                        .flex_grow()
                        .gap(px(2.))
                        .content_stretch(),
                )
                .child(
                    pager("chat-surface-root-pager", {
                        match session_manager.error() {
                            ClientError::None | ClientError::Recoverable(_) => {
                                if session_manager.client().is_some() {
                                    1
                                } else {
                                    0
                                }
                            }
                            ClientError::Terminal(_) => 2,
                        }
                    })
                    .animation(FadeAnimation::new())
                    .page(
                        div()
                            .size_full()
                            .flex()
                            .flex_col()
                            .items_center()
                            .justify_center()
                            .gap(px(8.))
                            .child(spinner())
                            .child(div().text_size(theme.heading_font_size).child(tr!(
                                "MAIN_CHAT_WELCOME",
                                "Welcome back, {{user}}!",
                                user = current_session.matrix_session.meta.user_id.localpart()
                            )))
                            .into_any_element(),
                    )
                    .page(self.main_chat_surface.clone().into_any_element())
                    .page(match session_manager.error() {
                        ClientError::None => div().into_any_element(),
                        ClientError::Terminal(terminal_error) => interstitial()
                            .size_full()
                            .icon("network-disconnect".into())
                            .title(
                                tr!("MAIN_CHAT_ERROR_TERMINAL", "Disconnected from Matrix").into(),
                            )
                            .message(terminal_error.description().into())
                            .when_else(
                                terminal_error.should_logout(),
                                |david| {
                                    david.child(
                                        button("log-out-button")
                                            .child(icon_text(
                                                "system-log-out".into(),
                                                tr!("ACCOUNT_LOG_OUT").into(),
                                            ))
                                            .on_click(cx.listener(|_, _, _, cx| {
                                                cx.update_global::<SessionManager, ()>(
                                                    |session_manager, cx| {
                                                        let details = cx.global::<Details>();
                                                        let directories =
                                                            details.standard_dirs().unwrap();
                                                        let data_dir = directories.data_dir();
                                                        let session_dir = data_dir.join("sessions");
                                                        let this_session_dir = session_dir.join(
                                                            session_manager
                                                                .current_session()
                                                                .as_ref()
                                                                .unwrap()
                                                                .uuid
                                                                .to_string(),
                                                        );

                                                        // Delete the session
                                                        remove_dir_all(this_session_dir).unwrap();

                                                        session_manager.clear_session()
                                                    },
                                                );
                                            })),
                                    )
                                },
                                |david| {
                                    david.child(
                                        button("log-out-button")
                                            .child(icon_text(
                                                "system-switch-user".into(),
                                                tr!("ACCOUNT_SWITCHER_ERROR", "Account Switcher")
                                                    .into(),
                                            ))
                                            .on_click(cx.listener(|_, _, _, cx| {
                                                cx.update_global::<SessionManager, ()>(
                                                    |session_manager, cx| {
                                                        session_manager.clear_session()
                                                    },
                                                );
                                            })),
                                    )
                                },
                            )
                            .into_any_element(),
                        ClientError::Recoverable(_) => div().into_any_element(),
                    })
                    .size_full(),
                )
                .application_menu(self.application_menu.clone()),
        )
    }
}
