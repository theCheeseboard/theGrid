mod devices_settings;
mod ignored_users_settings;
mod notifications_settings;
mod profile_settings;
pub mod security_settings;

use crate::account_settings::devices_settings::DevicesSettings;
use crate::account_settings::ignored_users_settings::IgnoredUsersSettings;
use crate::account_settings::notifications_settings::NotificationsSettings;
use crate::account_settings::profile_settings::ProfileSettings;
use crate::account_settings::security_settings::SecuritySettings;
use crate::main_window::{SurfaceChange, SurfaceChangeEvent, SurfaceChangeHandler};
use cntp_i18n::tr;
use contemporary::components::grandstand::grandstand;
use contemporary::components::layer::layer;
use contemporary::components::pager::lift_animation::LiftAnimation;
use contemporary::components::pager::pager;
use contemporary::components::pager::pager_animation::PagerAnimationDirection;
use contemporary::styling::theme::Theme;
use contemporary::surface::surface;
use gpui::prelude::FluentBuilder;
use gpui::{
    App, AppContext, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render,
    StatefulInteractiveElement, Styled, Window, div, px, uniform_list,
};
use std::rc::Rc;
use thegrid::session::session_manager::SessionManager;

#[derive(Clone)]
pub enum AccountSettingsPage {
    Profile,
    Devices,
}

pub struct AccountSettingsSurface {
    current_page: usize,

    profile_settings: Entity<ProfileSettings>,
    security_settings: Entity<SecuritySettings>,
    notifications_settings: Entity<NotificationsSettings>,
    devices_settings: Entity<DevicesSettings>,
    ignored_users_settings: Entity<IgnoredUsersSettings>,

    on_surface_change: Rc<Box<SurfaceChangeHandler>>,
}

impl AccountSettingsSurface {
    pub fn new(
        cx: &mut App,
        on_surface_change: impl Fn(&SurfaceChangeEvent, &mut Window, &mut App) + 'static,
    ) -> Entity<Self> {
        cx.new(|cx| Self {
            current_page: 0,

            profile_settings: ProfileSettings::new(cx),
            security_settings: SecuritySettings::new(cx),
            notifications_settings: NotificationsSettings::new(cx),
            devices_settings: DevicesSettings::new(cx),
            ignored_users_settings: IgnoredUsersSettings::new(cx),

            on_surface_change: Rc::new(Box::new(on_surface_change)),
        })
    }

    pub fn set_current_page(&mut self, page: AccountSettingsPage) {
        self.current_page = match page {
            AccountSettingsPage::Profile => 0,
            AccountSettingsPage::Devices => 3,
        }
    }
}

impl Render for AccountSettingsSurface {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let session_manager = cx.global::<SessionManager>();

        // Stop rendering here because we shouldn't get to see this page
        if session_manager.client().is_none() {
            return div().into_any_element();
        }

        surface()
            .child(
                div()
                    .id("account-settings")
                    .flex()
                    .w_full()
                    .h_full()
                    .gap(px(2.))
                    .child(
                        layer()
                            .w(px(300.))
                            .flex()
                            .flex_col()
                            .child(
                                grandstand("account-settings-grandstand")
                                    .text(tr!("ACCOUNT_SETTINGS_TITLE", "Account Settings"))
                                    .pt(px(36.))
                                    .on_back_click(cx.listener(|this, _, window, cx| {
                                        (this.on_surface_change)(
                                            &SurfaceChangeEvent {
                                                change: SurfaceChange::Pop,
                                            },
                                            window,
                                            cx,
                                        )
                                    })),
                            )
                            .child(
                                div().flex_grow().p(px(2.)).child(
                                    uniform_list(
                                        "sidebar-items",
                                        5,
                                        cx.processor(|this, range, _, cx| {
                                            let theme = cx.global::<Theme>();
                                            let mut items = Vec::new();
                                            for ix in range {
                                                let item = ix + 1;

                                                items.push(
                                                    div()
                                                        .id(ix)
                                                        .p(px(2.))
                                                        .rounded(theme.border_radius)
                                                        .on_click(cx.listener(
                                                            move |this, _, _, cx| {
                                                                this.current_page = ix;
                                                                cx.notify()
                                                            },
                                                        ))
                                                        .child(match ix {
                                                            0 => tr!(
                                                                "ACCOUNT_SETTINGS_PROFILE",
                                                                "Profile"
                                                            ),
                                                            1 => tr!(
                                                                "ACCOUNT_SETTINGS_SECURITY",
                                                                "Security"
                                                            ),
                                                            2 => tr!(
                                                                "ACCOUNT_SETTINGS_NOTIFICATIONS",
                                                                "Notifications"
                                                            ),
                                                            3 => tr!(
                                                                "ACCOUNT_SETTINGS_DEVICES",
                                                                "Devices"
                                                            ),
                                                            4 => tr!(
                                                                "ACCOUNT_SETTINGS_IGNORED_USERS",
                                                                "Ignored Users"
                                                            ),
                                                            _ => format!("Item {item}").into(),
                                                        })
                                                        .when(this.current_page == ix, |div| {
                                                            div.bg(theme.button_background)
                                                        }),
                                                );
                                            }
                                            items
                                        }),
                                    )
                                    .h_full()
                                    .w_full(),
                                ),
                            ),
                    )
                    .child(
                        pager("main-area", self.current_page)
                            .flex_grow()
                            .animation(LiftAnimation::new())
                            .animation_direction(PagerAnimationDirection::Forward)
                            .page(self.profile_settings.clone().into_any_element())
                            .page(self.security_settings.clone().into_any_element())
                            .page(self.notifications_settings.clone().into_any_element())
                            .page(self.devices_settings.clone().into_any_element())
                            .page(self.ignored_users_settings.clone().into_any_element()),
                    ),
            )
            .into_any_element()
    }
}
