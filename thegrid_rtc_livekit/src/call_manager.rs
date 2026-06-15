use crate::focus::get_focus_url;
use crate::{CallState, LivekitCall, TrackType};
use gpui::{App, AppContext, AsyncApp, BorrowAppContext, Context, Entity, Global, WeakEntity};
use livekit::track::TrackSource;
use matrix_sdk::ruma::{OwnedDeviceId, OwnedRoomId, OwnedUserId};
use rodio::DeviceSinkBuilder;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use thegrid_common::outbound_track::OutboundTrack;
use thegrid_common::session::session_manager::SessionManager;
use thegrid_common::sfx::SoundEffect;
use thegrid_screen_share::setup_screenshare_manager;

#[derive(Eq, PartialEq, Clone, Debug)]
pub struct VolumeKey {
    pub user_id: OwnedUserId,
    pub device_id: OwnedDeviceId,
    pub track_source: TrackSource,
}

impl VolumeKey {
    pub fn new(user_id: OwnedUserId, device_id: OwnedDeviceId, track_source: TrackSource) -> Self {
        Self {
            user_id,
            device_id,
            track_source,
        }
    }
}

impl Hash for VolumeKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.user_id.hash(state);
        self.device_id.hash(state);
        state.write(&[self.track_source as u8]);
    }
}

pub struct LivekitCallManager {
    current_call: Option<Entity<LivekitCall>>,
    active_calls: Vec<Entity<LivekitCall>>,
    mute: Entity<bool>,
    deaf: Entity<bool>,

    volumes: Entity<HashMap<VolumeKey, f32>>,

    active_output_device: Entity<Option<rodio::MixerDeviceSink>>,
    active_input_device: Entity<Option<cpal::Device>>,
}

#[derive(Clone)]
pub enum FocusUrl {
    Url(String),
    Processing,
    NoAvailableFocus,
}

impl LivekitCallManager {
    pub fn start_call(
        &mut self,
        room: OwnedRoomId,
        initial_streams: HashMap<TrackType, Entity<OutboundTrack>>,
        cx: &mut App,
    ) -> Option<Entity<LivekitCall>> {
        if self
            .active_calls
            .iter()
            .any(|call| call.read(cx).room() == room)
        {
            // This room is already in a call
            return None;
        }

        let call = cx.new(|cx| LivekitCall::new(room, initial_streams, cx));
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
        self.current_call = Some(call.clone());

        Some(call)
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

    pub fn volumes(&self) -> Entity<HashMap<VolumeKey, f32>> {
        self.volumes.clone()
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

    pub fn active_output_device(&self) -> Entity<Option<rodio::MixerDeviceSink>> {
        self.active_output_device.clone()
    }

    pub fn active_input_device(&self) -> Entity<Option<cpal::Device>> {
        self.active_input_device.clone()
    }

    pub fn set_active_output_device(&mut self, output_device: Option<cpal::Device>, cx: &mut App) {
        self.active_output_device.update(cx, |device, cx| {
            *device = output_device
                .and_then(|device| DeviceSinkBuilder::from_device(device).ok())
                .and_then(|device| device.open_stream().ok());
            cx.notify();
        });
    }
}

impl Global for LivekitCallManager {}

pub fn setup_call_manager(cx: &mut App) {
    let mute = cx.new(|_| false);
    let deaf = cx.new(|_| false);

    // TODO: Load and save these
    let volumes = cx.new(|_| HashMap::new());

    cx.observe(&mute, |mute, cx| {
        if *mute.read(cx) {
            SoundEffect::MuteOn.play()
        } else {
            SoundEffect::MuteOff.play()
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
        volumes,
        active_input_device,
        active_output_device,
    });

    setup_screenshare_manager(cx);
}
