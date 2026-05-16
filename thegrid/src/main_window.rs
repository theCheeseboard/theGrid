use crate::account_settings::deactivate_account::DeactivateSurface;
use crate::account_settings::security_settings::identity_reset::IdentityResetSurface;
use crate::account_settings::security_settings::password_change::PasswordChangeSurface;
use crate::account_settings::AccountSettingsSurface;
use crate::auth::auth_surface::AuthSurface;
use crate::chat::chat_surface::ChatSurface;
use crate::register::register_surface::RegisterSurface;
use contemporary::about_surface::about_surface;
use contemporary::components::pager::lift_animation::LiftAnimation;
use contemporary::components::pager::pager;
use contemporary::window::contemporary_window;
use gpui::{
    div, App, AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, Window,
};
use thegrid_common::session::session_manager::SessionManager;
use thegrid_common::surfaces::{
    AccountSettingsDeepLink, MainWindowSurface, SurfaceChange, SurfaceChangeEvent,
};
use thegrid_rtc_livekit::call_surface::CallSurface;

pub struct MainWindow {
    main_surface: Entity<ChatSurface>,
    auth_surface: Entity<AuthSurface>,
    register_surface: Entity<RegisterSurface>,
    account_settings_surface: Entity<AccountSettingsSurface>,
    identity_reset_surface: Entity<IdentityResetSurface>,
    password_change_surface: Entity<PasswordChangeSurface>,
    deactivate_account_surface: Entity<DeactivateSurface>,
    call_surface: Option<Entity<CallSurface>>,
    current_surface: Vec<MainWindowSurface>,
}

impl MainWindow {
    pub fn new(cx: &mut App) -> Entity<MainWindow> {
        cx.new(|cx| MainWindow {
            main_surface: {
                let handle_surface_change = cx.listener(Self::handle_surface_change);
                ChatSurface::new(cx, handle_surface_change)
            },
            auth_surface: {
                let handle_surface_change = cx.listener(Self::handle_surface_change);
                AuthSurface::new(cx, handle_surface_change)
            },
            register_surface: {
                let handle_surface_change = cx.listener(Self::handle_surface_change);
                cx.new(|cx| RegisterSurface::new(cx, handle_surface_change))
            },
            account_settings_surface: {
                let handle_surface_change = cx.listener(Self::handle_surface_change);
                AccountSettingsSurface::new(cx, handle_surface_change)
            },
            identity_reset_surface: {
                let handle_surface_change = cx.listener(Self::handle_surface_change);
                IdentityResetSurface::new(cx, handle_surface_change)
            },
            password_change_surface: {
                let handle_surface_change = cx.listener(Self::handle_surface_change);
                cx.new(|cx| PasswordChangeSurface::new(cx, handle_surface_change))
            },
            deactivate_account_surface: {
                let handle_surface_change = cx.listener(Self::handle_surface_change);
                DeactivateSurface::new(cx, handle_surface_change)
            },
            call_surface: None,
            current_surface: vec![MainWindowSurface::Main],
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

    pub fn open_settings(&mut self) {
        if matches!(
            self.current_surface.last().unwrap(),
            MainWindowSurface::Main
        ) {
            self.push_surface(MainWindowSurface::AccountSettings(
                AccountSettingsDeepLink::Profile,
            ));
        }
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
                if let MainWindowSurface::Call(room_id) = surface {
                    let handle_surface_change = cx.listener(Self::handle_surface_change);
                    this.call_surface =
                        Some(cx.new(|cx| {
                            CallSurface::new(cx, room_id.clone(), handle_surface_change)
                        }));
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
                    MainWindowSurface::Call(_) => 2,
                    MainWindowSurface::Register => 3,
                    MainWindowSurface::AccountSettings(_) => 4,
                    MainWindowSurface::IdentityReset => 5,
                    MainWindowSurface::PasswordChange => 6,
                    MainWindowSurface::DeactivateAccount => 7,
                    MainWindowSurface::About => 8,
                },
            )
            .w_full()
            .h_full()
            .animation(LiftAnimation::new())
            .page(self.main_surface.clone())
            .page(self.auth_surface.clone())
            .page(
                self.call_surface
                    .clone()
                    .map(|call_surface| call_surface.into_any_element())
                    .unwrap_or_else(|| div().into_any_element()),
            )
            .page(self.register_surface.clone())
            .page(self.account_settings_surface.clone())
            .page(self.identity_reset_surface.clone())
            .page(self.password_change_surface.clone())
            .page(self.deactivate_account_surface.clone())
            .page(about_surface().on_back_click(cx.listener(|this, _, _, cx| {
                this.current_surface.pop();
                cx.notify();
            }))),
        )
    }
}
