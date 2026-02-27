use async_ringbuf::traits::{AsyncProducer, Consumer, Producer, Split};
use std::collections::{HashMap, HashSet};
pub mod active_call_sidebar_alert;
pub mod call_manager;
pub(crate) mod sfx;

use crate::call_manager::LivekitCallManager;
use async_ringbuf::AsyncHeapRb;
use cancellation_token::CancellationTokenSource;
use cntp_i18n::{I18N_MANAGER, tr, tr_load};
use cpal::Host;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use gpui::http_client::anyhow;
use gpui::private::{anyhow, serde_json};
use gpui::{AppContext, AsyncApp, BorrowAppContext, Context, Entity, WeakEntity};
use livekit::id::TrackSid;
use livekit::options::TrackPublishOptions;
use livekit::prelude::LocalParticipant;
use livekit::track::{LocalAudioTrack, LocalTrack, RemoteTrack, TrackSource};
use livekit::webrtc::audio_frame::AudioFrame;
use livekit::webrtc::audio_source::native::NativeAudioSource;
use livekit::webrtc::audio_stream::native::NativeAudioStream;
use livekit::webrtc::prelude::RtcAudioSource;
use livekit::{Room, RoomError, RoomEvent, RoomOptions};
use log::{error, info, warn};
use matrix_sdk::deserialized_responses::RawAnySyncOrStrippedState;
use matrix_sdk::reqwest::StatusCode;
use matrix_sdk::room::RoomMember;
use matrix_sdk::ruma::api::client::account::request_openid_token;
use matrix_sdk::ruma::api::client::account::request_openid_token::v3::Response;
use matrix_sdk::ruma::api::client::discovery::discover_homeserver::RtcFocusInfo;
use matrix_sdk::ruma::events::call::member::{
    ActiveFocus, ActiveLivekitFocus, Application, CallApplicationContent, CallMemberEvent,
    CallMemberEventContent, CallMemberStateKey, CallScope, Focus, FocusSelection, LivekitFocus,
};
use matrix_sdk::ruma::events::rtc::notification::RtcNotificationEvent;
use matrix_sdk::ruma::events::{AnySyncStateEvent, StateEventType};
use matrix_sdk::ruma::exports::serde_json::json;
use matrix_sdk::ruma::serde::Raw;
use matrix_sdk::ruma::{OwnedDeviceId, OwnedRoomId, OwnedUserId, UserId};
use matrix_sdk::stream::StreamExt;
use matrix_sdk::{HttpError, reqwest};
use reqwest::header;
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use std::rc::Weak;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use thegrid_common::session::session_manager::SessionManager;
use thegrid_common::tokio_helper::TokioHelper;

pub fn setup_thegrid_rtc_livekit() {
    I18N_MANAGER.write().unwrap().load_source(tr_load!());
}

pub struct LivekitCall {
    room: OwnedRoomId,
    state: CallState,

    cpal_output_device: Option<cpal::Device>,
    cpal_input_device: Option<cpal::Device>,

    mic_track_sid: Option<TrackSid>,

    subscribed_streams: Vec<SubscribedStream>,
    active_call_participants_state: Vec<OwnedUserId>,
    cached_room_users: HashMap<OwnedUserId, Option<RoomMember>>,
    muted_streams: HashSet<TrackSid>,
    cached_call_members: Entity<Vec<CallMember>>,

    cancellation_source: CancellationTokenSource,
    started_at: Instant,
}

#[derive(Clone)]
pub struct CallMember {
    user_id: OwnedUserId,
    device_id: Option<OwnedDeviceId>,
    mic_state: StreamState,
    camera_state: StreamState,
    screenshare_state: StreamState,
}

#[derive(Copy, Clone, PartialEq)]
pub enum StreamState {
    Unavailable,
    Off,
    On,
}

pub struct SubscribedStream {
    stream_sid: TrackSid,
    user_id: OwnedUserId,
    device_id: OwnedDeviceId,
    source: TrackSource,
}

#[derive(Clone)]
pub enum CallState {
    Connecting,
    Active { local_participant: LocalParticipant },
    Ended,
    Error(CallError),
}

#[derive(Copy, Clone, PartialEq)]
pub enum CallError {
    RoomError,
    NoRtcFocus,
    OpenIdTokenRequestFailed,
    LivekitJwtRequestFailed,
    LivekitRtcFailed,
    StateEventForbidden,
}

#[derive(Serialize, Deserialize)]
struct LivekitJwtResponse {
    url: String,
    jwt: String,
}

impl Display for CallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            CallError::RoomError => tr!("CALL_ERROR_ROOM_ERROR", "Room error"),
            CallError::NoRtcFocus => tr!("CALL_ERROR_NO_RTC_FOCUS", "No RTC focus available"),
            CallError::OpenIdTokenRequestFailed => tr!(
                "CALL_ERROR_OPENID_TOKEN_REQUEST_FAILED",
                "Failed to request OpenID token"
            ),
            CallError::LivekitJwtRequestFailed => tr!(
                "CALL_ERROR_LIVEKIT_JWT_REQUEST_FAILED",
                "Failed to request LiveKit JWT"
            ),
            CallError::LivekitRtcFailed => tr!(
                "CALL_ERROR_LIVEKIT_RTC_FAILED",
                "Failed to join LiveKit room"
            ),
            CallError::StateEventForbidden => tr!(
                "CALL_ERROR_STATE_EVENT_FORBIDDEN",
                "No permission to join call"
            ),
        };
        write!(f, "{}", str)
    }
}

impl LivekitCall {
    //noinspection RsRedundantElse
    pub fn new(room_id: OwnedRoomId, cx: &mut Context<Self>) -> Self {
        let session_manager = cx.global::<SessionManager>();
        let client = session_manager.client().unwrap().read(cx).clone();
        let user_id = client.user_id().unwrap().to_owned();
        let room = session_manager
            .rooms()
            .read(cx)
            .room(&room_id)
            .unwrap()
            .read(cx)
            .inner
            .clone();
        let active_call_participants_state = room.active_room_call_participants();
        let room_id_clone = room_id.clone();
        let device_id = client.device_id().unwrap().to_owned();

        let rtc_foci = session_manager.rtc_foci().clone();

        let room_clone = room.clone();
        cx.spawn(
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                loop {
                    let room = room_clone.clone();
                    let _ = cx
                        .spawn_tokio(async move {
                            room.sync_up().await;
                            Ok::<_, anyhow::Error>(())
                        })
                        .await;

                    let active_call_participants = room_clone.active_room_call_participants();
                    if weak_this
                        .update(cx, |this, cx| {
                            this.active_call_participants_state = active_call_participants;
                            cx.notify();
                        })
                        .is_err()
                    {
                        return;
                    }
                }
            },
        )
        .detach();

        cx.spawn(
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                let room_clone = room.clone();
                let Ok(call_member_state_events) = cx
                    .spawn_tokio(async move {
                        room_clone
                            .get_state_events(StateEventType::CallMember)
                            .await
                    })
                    .await
                else {
                    let _ = weak_this.update(cx, |this, cx| {
                        this.state = CallState::Error(CallError::RoomError);
                        cx.notify();
                    });
                    return;
                };

                // Find the best focus URL
                let service_url = call_member_state_events
                    .iter()
                    .find_map(|state_event| {
                        let RawAnySyncOrStrippedState::Sync(event) = state_event else {
                            return None;
                        };

                        let Ok(AnySyncStateEvent::CallMember(event)) = event.deserialize() else {
                            return None;
                        };

                        let event = event.as_original()?;
                        let CallMemberEventContent::SessionContent(content) = &event.content else {
                            return None;
                        };

                        let ActiveFocus::Livekit(livekit_focus) = &content.focus_active else {
                            return None;
                        };

                        if livekit_focus.focus_selection != FocusSelection::OldestMembership {
                            return None;
                        };

                        content.foci_preferred.iter().find_map(|focus| {
                            let Focus::Livekit(lk_focus) = focus else {
                                return None;
                            };

                            if lk_focus.alias != room_id_clone {
                                return None;
                            }

                            Some(lk_focus.service_url.clone())
                        })
                    })
                    .or_else(|| {
                        let Some(RtcFocusInfo::LiveKit(livekit_focus)) = rtc_foci
                            .iter()
                            .find(|focus| matches!(focus, RtcFocusInfo::LiveKit(_)))
                        else {
                            return None;
                        };

                        Some(livekit_focus.service_url.clone())
                    });

                let Some(service_url) = service_url else {
                    let _ = weak_this.update(cx, |this, cx| {
                        this.state = CallState::Error(CallError::NoRtcFocus);
                        cx.notify();
                    });
                    return;
                };

                let openid_token_request =
                    request_openid_token::v3::Request::new(client.user_id().unwrap().to_owned());
                let openid_token_response = cx
                    .spawn_tokio(async move { client.send(openid_token_request).await })
                    .await;

                let openid_token = match openid_token_response {
                    Ok(token) => token,
                    Err(e) => {
                        let _ = weak_this.update(cx, |this, cx| {
                            this.state = CallState::Error(CallError::OpenIdTokenRequestFailed);
                            cx.notify();
                        });
                        return;
                    }
                };

                // Get the LiveKit JWT
                let client = reqwest::Client::new();
                let Ok(livekit_jwt_response) = client
                    .post(format!("{}/sfu/get", service_url))
                    .body(
                        json!({
                            "device_id": device_id.to_string(),
                            "openid_token": {
                                "access_token": openid_token.access_token,
                                "expires_in": openid_token.expires_in.as_secs(),
                                "matrix_server_name": openid_token.matrix_server_name.to_string(),
                                "token_type": openid_token.token_type.to_string()
                            },
                            "room": room_id_clone
                        })
                        .to_string(),
                    )
                    .header(header::CONTENT_TYPE, "application/json")
                    .send()
                    .await
                else {
                    let _ = weak_this.update(cx, |this, cx| {
                        this.state = CallState::Error(CallError::LivekitJwtRequestFailed);
                        cx.notify();
                    });
                    return;
                };

                let Ok(livekit_jwt_response) = livekit_jwt_response.text().await else {
                    let _ = weak_this.update(cx, |this, cx| {
                        this.state = CallState::Error(CallError::LivekitJwtRequestFailed);
                        cx.notify();
                    });
                    return;
                };

                let Ok(livekit_jwt) =
                    serde_json::from_str::<LivekitJwtResponse>(&livekit_jwt_response)
                else {
                    let _ = weak_this.update(cx, |this, cx| {
                        this.state = CallState::Error(CallError::LivekitJwtRequestFailed);
                        cx.notify();
                    });
                    return;
                };

                if let Err(e) = cx
                    .spawn_tokio(async move {
                        room.send_state_event_for_key(
                            &CallMemberStateKey::new(user_id, None, true),
                            CallMemberEventContent::new(
                                Application::Call(CallApplicationContent::new(
                                    "".to_string(),
                                    CallScope::Room,
                                )),
                                device_id,
                                ActiveFocus::Livekit(ActiveLivekitFocus::new()),
                                vec![Focus::Livekit(LivekitFocus::new(
                                    room_id_clone.to_string(),
                                    service_url,
                                ))],
                                None,
                                Some(Duration::from_millis(14400000)),
                            ),
                        )
                        .await
                    })
                    .await
                {
                    error!("Unable to send call state event: {:?}", e);
                    let _ = weak_this.update(cx, |this, cx| {
                        this.state = if let Some(client_api_error) = e.as_client_api_error()
                            && client_api_error.status_code == StatusCode::FORBIDDEN
                        {
                            CallState::Error(CallError::StateEventForbidden)
                        } else {
                            CallState::Error(CallError::LivekitRtcFailed)
                        };
                        cx.notify();
                    });
                    return;
                };

                let (livekit_room, mut room_events) = match cx
                    .spawn_tokio(async move {
                        let mut room_options = RoomOptions::default();
                        room_options.auto_subscribe = true;
                        room_options.adaptive_stream = true;
                        livekit::Room::connect(&livekit_jwt.url, &livekit_jwt.jwt, room_options)
                            .await
                    })
                    .await
                {
                    Ok(room) => room,
                    Err(e) => {
                        error!("LiveKit room connection failed: {:?}", e);
                        let _ = weak_this.update(cx, |this, cx| {
                            this.state = CallState::Error(CallError::LivekitRtcFailed);
                            cx.notify();
                        });
                        return;
                    }
                };

                let local_participant = livekit_room.local_participant();

                let weak_this_clone = weak_this.clone();
                cx.spawn(async move |cx: &mut AsyncApp| {
                    let x = livekit_room;

                    loop {
                        let Some(event) = room_events.recv().await else {
                            // TODO: End call?
                            return;
                        };

                        match &event {
                            RoomEvent::TrackSubscribed {
                                track,
                                publication,
                                participant,
                            } => {
                                let identity: String = participant.identity().into();
                                let Ok((user_id, device_id)) = decode_livekit_identity(&identity)
                                else {
                                    error!(
                                        "Subscribed to stream but failed to decode identity: {}. \
                                         The stream will not be played.",
                                        identity
                                    );
                                    return;
                                };

                                if weak_this_clone
                                    .update(cx, |this, cx| {
                                        this.subscribed_streams.push(SubscribedStream {
                                            stream_sid: track.sid(),
                                            user_id,
                                            device_id,
                                            source: track.source(),
                                        });
                                        if track.is_muted() {
                                            this.muted_streams.insert(track.sid());
                                        }
                                        cx.notify();

                                        this.start_track(track, cx);
                                    })
                                    .is_err()
                                {
                                    // TODO: End call?
                                    return;
                                }
                            }
                            RoomEvent::TrackUnsubscribed {
                                track,
                                publication,
                                participant,
                            } => {
                                if weak_this_clone
                                    .update(cx, |this, cx| {
                                        this.muted_streams.remove(&track.sid());
                                        this.subscribed_streams
                                            .retain(|stream| stream.stream_sid != track.sid());
                                    })
                                    .is_err()
                                {
                                    // TODO: End call?
                                    return;
                                }
                            }
                            RoomEvent::TrackMuted {
                                participant,
                                publication,
                            } => {
                                if weak_this_clone
                                    .update(cx, |this, cx| {
                                        this.muted_streams.insert(publication.sid());
                                    })
                                    .is_err()
                                {
                                    // TODO: End call?
                                    return;
                                }
                            }
                            RoomEvent::TrackUnmuted {
                                participant,
                                publication,
                            } => {
                                if weak_this_clone
                                    .update(cx, |this, cx| {
                                        this.muted_streams.remove(&publication.sid());
                                    })
                                    .is_err()
                                {
                                    // TODO: End call?
                                    return;
                                }
                            }
                            _ => {}
                        }

                        info!("LiveKit event: {:?}", event);
                    }
                })
                .detach();

                let _ = weak_this.update(cx, |this, cx| {
                    this.state = CallState::Active { local_participant };
                    this.started_at = Instant::now();
                    cx.notify();

                    this.setup_local_mic(cx);
                });

                // TODO: Delay a disconnection message
            },
        )
        .detach();

        let cached_call_members = cx.new(|_| Vec::new());
        cx.observe_self(|this, cx| {
            let old_call_members = this.cached_call_members.read(cx).len();
            let call_members = this.calculate_call_members(cx);

            if matches!(this.state, CallState::Active { .. }) {
                if old_call_members < call_members.len() {
                    sfx::play_sound_effect(include_bytes!("../assets/call-join.ogg"));
                } else if old_call_members > call_members.len() {
                    sfx::play_sound_effect(include_bytes!("../assets/call-leave.ogg"));
                }
            }

            this.cached_call_members.write(cx, call_members);
        })
        .detach();

        let cpal_host = cpal::default_host();

        sfx::play_sound_effect(include_bytes!("../assets/call-join.ogg"));

        LivekitCall {
            room: room_id,
            state: CallState::Connecting,
            cpal_output_device: cpal_host.default_output_device(),
            cpal_input_device: cpal_host.default_input_device(),
            cancellation_source: CancellationTokenSource::new(),
            started_at: Instant::now(),
            mic_track_sid: None,
            active_call_participants_state,
            subscribed_streams: Vec::new(),
            cached_room_users: HashMap::new(),
            muted_streams: HashSet::new(),
            cached_call_members,
        }
    }

    pub fn call_members(&self) -> Entity<Vec<CallMember>> {
        self.cached_call_members.clone()
    }

    fn calculate_call_members(&mut self, cx: &mut Context<Self>) -> Vec<CallMember> {
        let session_manager = cx.global::<SessionManager>();
        let this_user_id = session_manager
            .client()
            .unwrap()
            .read(cx)
            .user_id()
            .unwrap()
            .to_owned();

        let call_manager = cx.global::<LivekitCallManager>();
        let muted = *call_manager.mute().read(cx);

        let mut devices = self
            .subscribed_streams
            .iter()
            .map(|stream| (stream.user_id.clone(), stream.device_id.clone()))
            .collect::<HashSet<_>>();

        let mut call_members = Vec::new();
        let active_call_participants = self.active_call_participants_state.clone();
        let mut this_device_processed = false;
        for participant in active_call_participants.iter() {
            if let Some(tuple) = devices.iter().find(|(user_id, _)| user_id == participant) {
                self.cache_room_user(tuple.0.clone(), cx);
                let subscribed_streams = self
                    .subscribed_streams
                    .iter()
                    .filter(|stream| stream.user_id == tuple.0 && stream.device_id == tuple.1)
                    .collect::<Vec<_>>();

                let mut call_member = CallMember {
                    user_id: tuple.0.clone(),
                    device_id: Some(tuple.1.clone()),
                    mic_state: StreamState::Unavailable,
                    screenshare_state: StreamState::Unavailable,
                    camera_state: StreamState::Unavailable,
                };

                for stream in subscribed_streams {
                    let stream_state = if self.muted_streams.contains(&stream.stream_sid) {
                        StreamState::Off
                    } else {
                        StreamState::On
                    };

                    match stream.source {
                        TrackSource::Unknown => {}
                        TrackSource::Camera => {
                            call_member.camera_state = stream_state;
                        }
                        TrackSource::Microphone => {
                            call_member.mic_state = stream_state;
                        }
                        TrackSource::Screenshare => {
                            call_member.screenshare_state = stream_state;
                        }
                        TrackSource::ScreenshareAudio => {}
                    }
                }

                let tuple = tuple.clone();
                call_members.push(call_member);
                devices.remove(&tuple);
            } else {
                call_members.push(CallMember {
                    user_id: participant.clone(),
                    device_id: None,
                    mic_state: if !this_device_processed && participant == &this_user_id {
                        this_device_processed = true;
                        if muted {
                            StreamState::Off
                        } else {
                            StreamState::On
                        }
                    } else {
                        StreamState::Unavailable
                    },
                    camera_state: StreamState::Unavailable,
                    screenshare_state: StreamState::Unavailable,
                });
            };
        }
        call_members
    }

    fn cache_room_user(&mut self, user: OwnedUserId, cx: &mut Context<Self>) {
        if self.cached_room_users.contains_key(&user) {
            return;
        }

        self.cached_room_users.insert(user.clone(), None);

        let session_manager = cx.global::<SessionManager>();
        let room = session_manager
            .rooms()
            .read(cx)
            .room(&self.room)
            .unwrap()
            .read(cx)
            .inner
            .clone();
        cx.spawn(
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                let user_id = user.clone();
                let Ok(room_member) = cx
                    .spawn_tokio(async move { room.get_member(&user).await })
                    .await
                else {
                    return;
                };

                let _ = weak_this.update(cx, |this, cx| {
                    this.cached_room_users.insert(user_id, room_member);
                    cx.notify();
                });
            },
        )
        .detach();
    }

    pub fn get_cached_room_user(&self, user: &OwnedUserId) -> Option<RoomMember> {
        self.cached_room_users.get(user).cloned().flatten()
    }

    fn setup_local_mic(&mut self, cx: &mut Context<Self>) {
        let call_manager = cx.global::<LivekitCallManager>();
        let cancellation_token = self.cancellation_source.token();
        let local_participant = match &self.state {
            CallState::Active { local_participant } => local_participant.clone(),
            _ => return,
        };

        if let Some(mic_track_sid) = &self.mic_track_sid {
            let local_participant_clone = local_participant.clone();
            let mic_track_sid = mic_track_sid.clone();
            cx.spawn(
                async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                    let _ = cx
                        .spawn_tokio(async move {
                            local_participant_clone
                                .unpublish_track(&mic_track_sid)
                                .await
                        })
                        .await;
                },
            )
            .detach();

            self.mic_track_sid = None;
        }

        let Some(device) = &self.cpal_input_device else {
            warn!("No input device available for audio track: ignoring");
            return;
        };

        let (mut producer, mut consumer) = AsyncHeapRb::<Vec<i16>>::new(32).split();

        let mut supported_device_configs = device.supported_input_configs().unwrap();
        let supported_config = supported_device_configs
            .next()
            .unwrap()
            .with_sample_rate(48000);

        let input_stream = device
            .build_input_stream(
                &supported_config.config(),
                move |data: &[i16], _: &cpal::InputCallbackInfo| {
                    let _ = producer.try_push(data.to_vec());
                },
                move |err| {
                    // Errors? What errors!?
                    error!("cpal: error in input stream: {:?}", err)
                },
                None,
            )
            .unwrap();

        let source = NativeAudioSource::new(
            Default::default(),
            supported_config.sample_rate(),
            supported_config.channels() as u32,
            1000,
        );
        let track =
            LocalAudioTrack::create_audio_track("mic", RtcAudioSource::Native(source.clone()));

        if *call_manager.mute().read(cx) {
            track.mute();
        }

        let track_clone = track.clone();
        cx.observe(&call_manager.mute(), move |this, mute, cx| {
            if *mute.read(cx) {
                track_clone.mute();
            } else {
                track_clone.unmute();
            }
        })
        .detach();

        let num_channels = supported_config.channels() as u32;
        let sample_rate = supported_config.sample_rate();
        cx.spawn(
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                let Ok(publication) = cx
                    .spawn_tokio(async move {
                        local_participant
                            .publish_track(
                                LocalTrack::Audio(track),
                                TrackPublishOptions {
                                    source: TrackSource::Microphone,
                                    ..Default::default()
                                },
                            )
                            .await
                    })
                    .await
                else {
                    return;
                };

                let sid = publication.sid();
                let _ = weak_this.update(cx, |call, cx| {
                    call.mic_track_sid = Some(sid);
                });

                let _ = cx
                    .spawn_tokio(async move {
                        // Receive the audio frames in a new task
                        while let Some(audio_frame_data) = consumer.next().await {
                            if cancellation_token.is_canceled() {
                                return Ok(());
                            }

                            let audio_frame = AudioFrame {
                                num_channels,
                                sample_rate,
                                samples_per_channel: (audio_frame_data.len()
                                    / num_channels as usize)
                                    as u32,
                                data: audio_frame_data.into(),
                            };

                            if source.capture_frame(&audio_frame).await.is_err() {
                                return Ok(());
                            };
                        }

                        Ok::<_, anyhow::Error>(())
                    })
                    .await;

                let _ = input_stream.pause();
            },
        )
        .detach();
    }

    fn start_track(&mut self, track: &RemoteTrack, cx: &mut Context<Self>) {
        let cancellation_token = self.cancellation_source.token();
        match track {
            RemoteTrack::Audio(audio_track) => {
                let Some(device) = &self.cpal_output_device else {
                    warn!("No output device available for audio track: ignoring");
                    return;
                };

                let (mut producer, mut consumer) = AsyncHeapRb::<i16>::new(16384).split();

                let mut supported_device_configs = device.supported_output_configs().unwrap();
                let supported_config = supported_device_configs
                    .next()
                    .unwrap()
                    .with_sample_rate(48000);

                let output_stream = device
                    .build_output_stream(
                        &supported_config.config(),
                        move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                            consumer.pop_slice(data);
                        },
                        move |err| {
                            // Errors? What errors!?
                            error!("cpal: error in output stream: {:?}", err)
                        },
                        None,
                    )
                    .unwrap();

                let rtc_track = audio_track.rtc_track();
                let mut audio_stream = NativeAudioStream::new(
                    rtc_track,
                    supported_config.sample_rate() as i32,
                    supported_config.channels() as i32,
                );
                cx.spawn(
                    async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                        let _ = cx
                            .spawn_tokio(async move {
                                // Receive the audio frames in a new task
                                while let Some(audio_frame) = audio_stream.next().await {
                                    if cancellation_token.is_canceled() {
                                        return Ok(());
                                    }

                                    if producer.push_exact(&audio_frame.data).await.is_err() {
                                        return Ok(());
                                    };
                                }

                                Ok::<_, anyhow::Error>(())
                            })
                            .await;

                        output_stream.pause()
                    },
                )
                .detach();
            }
            RemoteTrack::Video(_) => {
                // TODO
            }
        }
    }

    pub fn end_call(&mut self, cx: &mut Context<Self>) {
        self.cancellation_source.cancel();

        let session_manager = cx.global::<SessionManager>();
        let user_id = session_manager
            .client()
            .unwrap()
            .read(cx)
            .user_id()
            .unwrap()
            .to_owned();
        let room = session_manager
            .rooms()
            .read(cx)
            .room(&self.room)
            .unwrap()
            .read(cx)
            .inner
            .clone();

        // Try to notify everyone that we have hung up
        cx.spawn(
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                if let Err(e) = cx
                    .spawn_tokio(async move {
                        room.send_state_event_for_key(
                            &CallMemberStateKey::new(user_id, None, true),
                            CallMemberEventContent::new_empty(None),
                        )
                        .await
                    })
                    .await
                {
                    error!("Unable to send hang up call state event: {:?}", e);
                };
            },
        )
        .detach();

        self.state = CallState::Ended;
        cx.notify();

        sfx::play_sound_effect(include_bytes!("../assets/call-leave.ogg"));
    }

    pub fn state(&self) -> &CallState {
        &self.state
    }
}

fn decode_livekit_identity(identity: &str) -> Result<(OwnedUserId, OwnedDeviceId), anyhow::Error> {
    let final_colon = identity
        .rfind(':')
        .ok_or(anyhow!("Identity does not contain a colon"))?;
    let user_part = &identity[..final_colon];
    let device_part = &identity[final_colon + 1..];

    Ok((UserId::parse(user_part)?, device_part.into()))
}
