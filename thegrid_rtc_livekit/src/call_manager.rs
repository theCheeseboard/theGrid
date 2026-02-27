use crate::{CallState, LivekitCall, sfx};
use gpui::{App, AppContext, BorrowAppContext, Entity, Global};
use matrix_sdk::ruma::OwnedRoomId;

pub struct LivekitCallManager {
    current_call: Option<Entity<LivekitCall>>,
    active_calls: Vec<Entity<LivekitCall>>,
    mute: Entity<bool>,
}

impl LivekitCallManager {
    pub fn start_call(&mut self, room: OwnedRoomId, cx: &mut App) {
        if self
            .active_calls
            .iter()
            .any(|call| call.read(cx).room() == room)
        {
            // This room is already in a call
            return;
        }

        let call = cx.new(|cx| LivekitCall::new(room, cx));
        cx.observe(&call, |call, cx| {
            cx.update_global::<LivekitCallManager, _>(|call_manager, cx| {
                if matches!(call.read(cx).state, CallState::Ended) {
                    // This call is over
                    call_manager
                        .active_calls
                        .retain(|active_call| active_call != &call);

                    let next_active_call = call_manager.active_calls.first();

                    if call_manager
                        .current_call
                        .as_ref()
                        .is_some_and(|call_manager_call| call_manager_call == &call)
                    {
                        call_manager.current_call = next_active_call.cloned();
                    }
                }
            });
        })
        .detach();

        self.active_calls.push(call.clone());
        self.current_call = Some(call)
    }

    pub fn current_call(&self) -> Option<Entity<LivekitCall>> {
        self.current_call.clone()
    }

    pub fn mute(&self) -> Entity<bool> {
        self.mute.clone()
    }

    pub fn calls(&self) -> &Vec<Entity<LivekitCall>> {
        &self.active_calls
    }

    pub fn switch_to_call(&mut self, call: Entity<LivekitCall>, cx: &mut App) {
        self.current_call = Some(call.clone());
        call.update(cx, |call, cx| {
            call.set_on_hold(false, cx);
        });
    }
}

impl Global for LivekitCallManager {}

pub fn setup_call_manager(cx: &mut App) {
    let mute = cx.new(|_| false);

    cx.observe(&mute, |mute, cx| {
        if *mute.read(cx) {
            sfx::play_sound_effect(include_bytes!("../assets/mute-on.ogg"));
        } else {
            sfx::play_sound_effect(include_bytes!("../assets/mute-off.ogg"));
        }
    })
    .detach();

    cx.set_global(LivekitCallManager {
        current_call: None,
        active_calls: Vec::new(),
        mute,
    });
}
