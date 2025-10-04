use crate::main_surface::MainSurface;
use contemporary::about_surface::about_surface;
use contemporary::components::pager::lift_animation::LiftAnimation;
use contemporary::components::pager::pager;
use contemporary::window::contemporary_window;
use gpui::{
    App, AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled,
    Window,
};
use std::rc::Rc;
use crate::auth::auth_surface::AuthSurface;

pub struct MainWindow {
    main_surface: Entity<MainSurface>,
    auth_surface: Entity<AuthSurface>,
    current_surface: Vec<MainWindowSurface>,
}

enum MainWindowSurface {
    Main,
    Auth,
    About,
}

impl MainWindow {
    pub fn new(cx: &mut App) -> Entity<MainWindow> {
        cx.new(|cx| {
            MainWindow {
                main_surface: MainSurface::new(cx),
                auth_surface: AuthSurface::new(cx),
                current_surface: vec![MainWindowSurface::Auth],
            }
        })
    }

    pub fn about_surface_open(&mut self, is_open: bool) -> &Self {
        if is_open {
            self.current_surface.push(MainWindowSurface::About);
        } else {
            self.current_surface.pop();
        }
        self
    }
}

impl Render for MainWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        contemporary_window()
            .child(
                pager(
                    "main-pager",
                    match self.current_surface.last().unwrap() {
                        MainWindowSurface::Main => 0,
                        MainWindowSurface::Auth => 1,
                        MainWindowSurface::About => 2,
                    },
                )
                    .w_full()
                    .h_full()
                    .animation(LiftAnimation::new())
                    .page(self.main_surface.clone().into_any_element())
                    .page(self.auth_surface.clone().into_any_element())
                    .page(
                        about_surface()
                            .on_back_click(cx.listener(|this, _, _, cx| {
                                this.current_surface.pop();
                                cx.notify();
                            }))
                            .into_any_element(),
                    ),
            )

    }
}
