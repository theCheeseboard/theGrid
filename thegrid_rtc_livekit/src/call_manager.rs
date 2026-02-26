use crate::{sfx, CallState, LivekitCall};
use gpui::{App, AppContext, BorrowAppContext, Entity, Global};
use matrix_sdk::ruma::OwnedRoomId;

pub struct LivekitCallManager {
    current_call: Option<Entity<LivekitCall>>,
    mute: Entity<bool>,
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

                if matches!(call.read(cx).state, CallState::Ended) {
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
    
    pub fn mute(&self) -> Entity<bool> {
        self.mute.clone()
    }
}

impl Global for LivekitCallManager {}

pub fn setup_call_manager(cx: &mut gpui::App) {
    let mute = cx.new(|_| false);
    
    cx.observe(&mute, |mute, cx| {
        if *mute.read(cx) {
            sfx::play_sound_effect(include_bytes!("../assets/mute-on.ogg"));
        } else {
            sfx::play_sound_effect(include_bytes!("../assets/mute-off.ogg"));
        }
    }).detach();
    
    cx.set_global(LivekitCallManager {
        current_call: None,
        mute,
    });
}
