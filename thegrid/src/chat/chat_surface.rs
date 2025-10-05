use crate::chat::main_chat_surface::MainChatSurface;
use cntp_i18n::tr;
use contemporary::components::application_menu::ApplicationMenu;
use contemporary::components::pager::fade_animation::FadeAnimation;
use contemporary::components::pager::pager;
use contemporary::components::spinner::spinner;
use contemporary::styling::theme::Theme;
use contemporary::surface::surface;
use gpui::{
    App, AppContext, Context, Entity, InteractiveElement, IntoElement, Menu, ParentElement, Render,
    Styled, Window, div, px,
};
use thegrid::session::session_manager::SessionManager;

pub struct ChatSurface {
    application_menu: Entity<ApplicationMenu>,
    main_chat_surface: Entity<MainChatSurface>,
}

impl ChatSurface {
    pub fn new(cx: &mut App) -> Entity<ChatSurface> {
        cx.new(|cx| ChatSurface {
            application_menu: ApplicationMenu::new(
                cx,
                Menu {
                    name: "Application Menu".into(),
                    items: vec![],
                },
            ),

            main_chat_surface: MainChatSurface::new(cx),
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
                    pager(
                        "chat-surface-root-pager",
                        if session_manager.client().is_some() {
                            1
                        } else {
                            0
                        },
                    )
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
                    .size_full(),
                )
                .application_menu(self.application_menu.clone()),
        )
    }
}
