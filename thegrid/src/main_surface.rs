use cntp_i18n::tr;
use contemporary::components::application_menu::ApplicationMenu;
use contemporary::components::button::button;
use contemporary::components::icon::icon;
use contemporary::components::pager::pager;
use contemporary::components::pager::slide_horizontal_animation::SlideHorizontalAnimation;
use contemporary::styling::theme::Theme;
use contemporary::surface::surface;
use gpui::prelude::FluentBuilder;
use gpui::{
    App, AppContext, Context, Entity, InteractiveElement, IntoElement, Menu, MenuItem,
    ParentElement, Render, Styled, Window, div, px,
};
use std::rc::Rc;

pub struct MainSurface {
    application_menu: Entity<ApplicationMenu>,
    current_terminal_screen: usize,
}

impl MainSurface {
    pub fn new(cx: &mut App) -> Entity<MainSurface> {
        cx.new(|cx| {

            MainSurface {
                application_menu: ApplicationMenu::new(
                    cx,
                    Menu {
                        name: "Application Menu".into(),
                        items: vec![
                        ],
                    },
                ),

                current_terminal_screen: 0,
            }
        })
    }
}

impl Render for MainSurface {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>();

        div()
            .size_full()
            .key_context("MainSurface")
            .child(
                surface()
                    .actions(
                        div()
                            .occlude()
                            .flex()
                            .flex_grow()
                            .gap(px(2.))
                            .content_stretch()
                    )
                    .child(
                        div()
                    )
                    .application_menu(self.application_menu.clone()),
            )
    }
}
