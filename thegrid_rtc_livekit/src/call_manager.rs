use crate::{CallState, LivekitCall};
use gpui::{App, AppContext, BorrowAppContext, Entity, Global};
use matrix_sdk::ruma::OwnedRoomId;

pub struct LivekitCallManager {
    current_call: Option<Entity<LivekitCall>>,
}

impl LivekitCallManager {
    pub fn start_call(&mut self, room: OwnedRoomId, cx: &mut App) {
        let call = cx.new(|cx| LivekitCall::new(room, cx));
        cx.observe(&call, |call, cx| {
            cx.update_global::<LivekitCallManager, _>(|call_manager, cx| {
                if call_manager
                    .current_call
                    .as_ref()
                    .is_none_or(|call_manager_call| call_manager_call != &call)
                {
                    return;
                }

                if call.read(cx).state == CallState::Ended {
                    // This call is over
                    call_manager.current_call = None;
                }
            });
        })
        .detach();
        self.current_call = Some(call)
    }

    pub fn current_call(&self) -> Option<Entity<LivekitCall>> {
        self.current_call.clone()
    }
}

impl Global for LivekitCallManager {}

pub fn setup_call_manager(cx: &mut gpui::App) {
    cx.set_global(LivekitCallManager { current_call: None });
}
