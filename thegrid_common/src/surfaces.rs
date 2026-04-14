use gpui::{App, Window};
use matrix_sdk::ruma::OwnedRoomId;

#[derive(Clone)]
pub enum AccountSettingsDeepLink {
    Profile,
    Devices,
}

#[derive(Clone)]
pub enum MainWindowSurface {
    Main,
    Call(OwnedRoomId),
    AccountSettings(AccountSettingsDeepLink),
    Register,
    IdentityReset,
    DeactivateAccount,
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
