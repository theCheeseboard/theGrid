use crate::account_settings::{AccountSettingsPage, AccountSettingsSurface};
use crate::auth::auth_surface::AuthSurface;
use crate::chat::chat_surface::ChatSurface;
use crate::identity_reset::IdentityResetSurface;
use contemporary::about_surface::about_surface;
use contemporary::components::pager::lift_animation::LiftAnimation;
use contemporary::components::pager::pager;
use contemporary::window::contemporary_window;
use gpui::{App, AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, Window};
use thegrid::session::session_manager::SessionManager;

pub struct MainWindow {
    main_surface: Entity<ChatSurface>,
    auth_surface: Entity<AuthSurface>,
    account_settings_surface: Entity<AccountSettingsSurface>,
    identity_reset_surface: Entity<IdentityResetSurface>,
    current_surface: Vec<MainWindowSurface>,
}

#[derive(Clone)]
pub enum MainWindowSurface {
    Main,
    AccountSettings(AccountSettingsPage),
    IdentityReset,
    About,
}

pub type SurfaceChangeHandler = dyn Fn(&SurfaceChangeEvent, &mut Window, &mut App) + 'static;

#[derive(Clone)]
pub struct SurfaceChangeEvent {
    pub change: SurfaceChange,
}

#[derive(Clone)]
pub enum SurfaceChange {
    Push(MainWindowSurface),
    Pop,
}

impl From<MainWindowSurface> for SurfaceChange {
    fn from(value: MainWindowSurface) -> Self {
        SurfaceChange::Push(value)
    }
}

impl MainWindow {
    pub fn new(cx: &mut App) -> Entity<MainWindow> {
        cx.new(|cx| {
            let handle_surface_change = cx.listener(Self::handle_surface_change);
            let handle_surface_change_2 = cx.listener(Self::handle_surface_change);
            let handle_surface_change_3 = cx.listener(Self::handle_surface_change);

            MainWindow {
                main_surface: ChatSurface::new(cx, handle_surface_change),
                auth_surface: AuthSurface::new(cx),
                account_settings_surface: AccountSettingsSurface::new(cx, handle_surface_change_2),
                identity_reset_surface: IdentityResetSurface::new(cx, handle_surface_change_3),
                current_surface: vec![MainWindowSurface::Main],
            }
        })
    }

    pub fn about_surface_open(&mut self, is_open: bool) -> &Self {
        if is_open {
            self.push_surface(MainWindowSurface::About);
        } else {
            self.pop_surface();
        }
        self
    }

    fn handle_surface_change(
        this: &mut MainWindow,
        event: &SurfaceChangeEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match &event.change {
            SurfaceChange::Push(surface) => {
                this.push_surface(surface.clone());
                if let MainWindowSurface::AccountSettings(page) = surface {
                    this.account_settings_surface
                        .update(cx, |account_settings_surface, cx| {
                            account_settings_surface.set_current_page(page.clone());
                            cx.notify();
                        })
                }
            }
            SurfaceChange::Pop => this.pop_surface(),
        }
    }

    pub fn push_surface(&mut self, surface: MainWindowSurface) {
        self.current_surface.push(surface);
    }

    pub fn pop_surface(&mut self) {
        self.current_surface.pop();
    }
}

impl Render for MainWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let session_manager = cx.global::<SessionManager>();
        contemporary_window().child(
            pager(
                "main-pager",
                match self.current_surface.last().unwrap() {
                    MainWindowSurface::Main => match session_manager.current_session() {
                        Some(_) => 0,
                        None => 1,
                    },
                    MainWindowSurface::AccountSettings(_) => 2,
                    MainWindowSurface::IdentityReset => 3,
                    MainWindowSurface::About => 4,
                },
            )
            .w_full()
            .h_full()
            .animation(LiftAnimation::new())
            .page(self.main_surface.clone().into_any_element())
            .page(self.auth_surface.clone().into_any_element())
            .page(self.account_settings_surface.clone().into_any_element())
            .page(self.identity_reset_surface.clone().into_any_element())
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
