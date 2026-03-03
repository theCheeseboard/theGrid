use crate::focus::get_focus_url;
use crate::{CallState, LivekitCall, sfx};
use gpui::{App, AppContext, AsyncApp, BorrowAppContext, Context, Entity, Global, WeakEntity};
use matrix_sdk::ruma::OwnedRoomId;
use thegrid_common::session::session_manager::SessionManager;

pub struct LivekitCallManager {
    current_call: Option<Entity<LivekitCall>>,
    active_calls: Vec<Entity<LivekitCall>>,
    mute: Entity<bool>,
    deaf: Entity<bool>,

    active_output_device: Entity<Option<cpal::Device>>,
    active_input_device: Entity<Option<cpal::Device>>,
}

#[derive(Clone)]
pub enum FocusUrl {
    Url(String),
    Processing,
    NoAvailableFocus,
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

    pub fn deaf(&self) -> Entity<bool> {
        self.deaf.clone()
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

    pub fn best_focus_url_for_room(
        &mut self,
        room_id: OwnedRoomId,
        cx: &mut Context<FocusUrl>,
    ) -> FocusUrl {
        let session_manager = cx.global::<SessionManager>();
        let room = session_manager
            .rooms()
            .read(cx)
            .room(&room_id)
            .unwrap()
            .read(cx)
            .inner
            .clone();
        let rtc_foci = session_manager.rtc_foci().clone();

        cx.spawn(
            async move |weak_entity: WeakEntity<FocusUrl>, cx: &mut AsyncApp| {
                let focus_url = get_focus_url(room, rtc_foci, cx).await;
                let Some(entity) = weak_entity.upgrade() else {
                    return;
                };

                let _ = match focus_url {
                    Ok(focus_url) => entity.write(cx, FocusUrl::Url(focus_url)),
                    Err(_) => entity.write(cx, FocusUrl::NoAvailableFocus),
                };
            },
        )
        .detach();

        FocusUrl::Processing
    }

    pub fn active_output_device(&self) -> Entity<Option<cpal::Device>> {
        self.active_output_device.clone()
    }

    pub fn active_input_device(&self) -> Entity<Option<cpal::Device>> {
        self.active_input_device.clone()
    }
}

impl Global for LivekitCallManager {}

pub fn setup_call_manager(cx: &mut App) {
    let mute = cx.new(|_| false);
    let deaf = cx.new(|_| false);

    cx.observe(&mute, |mute, cx| {
        if *mute.read(cx) {
            sfx::play_sound_effect(include_bytes!("../assets/mute-on.ogg"));
        } else {
            sfx::play_sound_effect(include_bytes!("../assets/mute-off.ogg"));
        }
    })
    .detach();

    let active_input_device = cx.new(|_| None);
    let active_output_device = cx.new(|_| None);

    cx.set_global(LivekitCallManager {
        current_call: None,
        active_calls: Vec::new(),
        mute,
        deaf,
        active_input_device,
        active_output_device,
    });
}
