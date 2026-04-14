use gpui::{App, AppContext, Entity, Global, Window};
use thegrid_common::outbound_track::OutboundTrack;

mod background_rgb_yuv_thread;
#[cfg(target_os = "macos")]
mod mac;

#[cfg(target_os = "linux")]
mod xdg_portal;

#[cfg(target_os = "windows")]
mod win;

pub enum PickerRequired {
    SystemPicker,
    ApplicationPicker,
    UnsupportedPlatform,
}

pub struct ScreenShareStartEvent {
    pub frames: Entity<OutboundTrack>,
}

pub struct ScreenShareManager {
    #[cfg(target_os = "linux")]
    xdg_portal_screenshare_manager: Entity<xdg_portal::XdgPortalScreenshareManager>,
}

impl ScreenShareManager {
    pub fn new(cx: &mut App) -> Self {
        Self {
            #[cfg(target_os = "linux")]
            xdg_portal_screenshare_manager: cx.new(|cx| xdg_portal::XdgPortalScreenshareManager::new(cx)),
        }
    }

    pub fn picker_required(&self, cx: &App) -> PickerRequired {
        #[cfg(target_os = "macos")]
        return PickerRequired::SystemPicker;

        #[cfg(target_os = "linux")]
        {
            let xdg_portal = self.xdg_portal_screenshare_manager.read(cx);
            if xdg_portal.is_available() {
                return PickerRequired::SystemPicker;
            }
        }

        #[cfg(target_os = "windows")]
        return PickerRequired::SystemPicker;

        PickerRequired::UnsupportedPlatform
    }

    pub fn start_screen_share_session(
        &self,
        callback: impl Fn(&ScreenShareStartEvent, &mut Window, &mut App) + 'static,
        window: &mut Window,
        cx: &mut App,
    ) {
        #[cfg(target_os = "macos")]
        {
            return mac::start_screen_share_session(callback, window, cx);
        }

        #[cfg(target_os = "linux")]
        {
            if self
                .xdg_portal_screenshare_manager
                .update(cx, |xdg_portal, cx| {
                    if xdg_portal.is_available() {
                        xdg_portal.start_screen_share_session(callback, window, cx);
                        return true;
                    }
                    false
                })
            {
                return;
            }
        }

        #[cfg(target_os = "windows")]
        {
            return win::start_screen_share_session(callback, window, cx);
        }

        panic!("Unsupported platform")
    }
}

impl Global for ScreenShareManager {}

pub fn setup_screenshare_manager(cx: &mut App) {
    let screenshare_manager = ScreenShareManager::new(cx);
    cx.set_global(screenshare_manager)
}
