pub mod active_call_sidebar_alert;
pub mod call_manager;

use crate::call_manager::LivekitCallManager;
use cntp_i18n::{I18N_MANAGER, tr_load};
use gpui::{App, BorrowAppContext, Context, Entity};
use matrix_sdk::ruma::OwnedRoomId;

pub fn setup_thegrid_rtc_livekit() {
    I18N_MANAGER.write().unwrap().load_source(tr_load!());
}

pub struct LivekitCall {
    room: OwnedRoomId,
    state: CallState,
}

#[derive(Copy, Clone, PartialEq)]
pub enum CallState {
    Active,
    Ended,
}

impl LivekitCall {
    pub fn new(room: OwnedRoomId, cx: &mut Context<Self>) -> Self {
        LivekitCall {
            room,
            state: CallState::Active,
        }
    }

    pub fn end_call(&mut self, cx: &mut Context<Self>) {
        self.state = CallState::Ended;
        cx.notify();
    }

    pub fn state(&self) -> CallState {
        self.state
    }
}
