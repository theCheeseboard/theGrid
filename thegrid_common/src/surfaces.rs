use cntp_i18n::{I18nString, tr};
use gpui::{App, Window};
use matrix_sdk::ruma::OwnedRoomId;

#[derive(Clone)]
pub enum AccountSettingsDeepLink {
    Profile,
    Devices,
}

#[derive(Copy, Clone)]
pub enum NotReadyReason {
    SecretServiceManagerBroken,
}

impl NotReadyReason {
    pub fn reason(&self) -> I18nString {
        match self {
            NotReadyReason::SecretServiceManagerBroken => {
                tr!(
                    "NOT_READY_SECRET_SERVICE_MANAGER_BROKEN",
                    "Your secret service manager is not working correctly."
                )
            }
        }
    }
}

#[derive(Clone)]
pub enum MainWindowSurface {
    Main,
    Call(OwnedRoomId),
    AccountSettings(AccountSettingsDeepLink),
    Register,
    IdentityReset,
    PasswordChange,
    DeactivateAccount,
    About,
    NotReady(NotReadyReason),
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
