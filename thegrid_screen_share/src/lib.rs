use crate::mac::MacScreenShareSetup;
use gpui::{App, Entity, Global, Window};
use thegrid_common::video_frame::VideoFrame;

#[cfg(target_os = "macos")]
mod mac;

pub enum PickerRequired {
    SystemPicker,
    ApplicationPicker,
    UnsupportedPlatform,
}

pub struct ScreenShareStartEvent {
    frames: Entity<VideoFrame>,
}

pub struct ScreenShareManager {}

impl ScreenShareManager {
    pub fn picker_required(&self) -> PickerRequired {
        #[cfg(target_os = "macos")]
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

        panic!("Unsupported platform")
    }
}

impl Global for ScreenShareManager {}

pub fn setup_screenshare_manager(cx: &mut App) {
    cx.set_global(ScreenShareManager {})
}
