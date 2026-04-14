use async_ringbuf::traits::{AsyncProducer, Consumer, Producer, Split};
use std::collections::{HashMap, HashSet};
pub mod active_call_sidebar_alert;
pub mod call_disconnect_confirmation_dialog;
pub mod call_manager;
pub mod call_surface;
mod focus;
mod mic;
mod webcam;

use crate::call_manager::LivekitCallManager;
use crate::focus::{get_focus_url, FocusUrlError};
use crate::mic::open_mic;
use crate::webcam::Webcam;
use async_ringbuf::consumer::AsyncConsumer;
use async_ringbuf::AsyncHeapRb;
use cancellation_token::CancellationTokenSource;
use cntp_i18n::{tr, tr_load, I18N_MANAGER};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::Host;
use gpui::http_client::anyhow;
use gpui::private::{anyhow, serde_json};
use gpui::{
    AppContext, AsyncApp, BorrowAppContext, Context, Entity, Image, RenderImage, WeakEntity,
};
use image::{Frame, RgbaImage};
use livekit::id::TrackSid;
use livekit::options::TrackPublishOptions;
use livekit::prelude::LocalParticipant;
use livekit::track::{
    LocalAudioTrack, LocalTrack, LocalVideoTrack, RemoteAudioTrack, RemoteTrack, RemoteVideoTrack,
    TrackSource,
};
use livekit::webrtc::audio_frame::AudioFrame;
use livekit::webrtc::audio_source::native::NativeAudioSource;
use livekit::webrtc::audio_stream::native::NativeAudioStream;
use livekit::webrtc::prelude::{RtcAudioSource, VideoBuffer, VideoBufferType};
use livekit::webrtc::video_frame::{I422Buffer, VideoRotation};
use livekit::webrtc::video_source::native::NativeVideoSource;
use livekit::webrtc::video_source::{RtcVideoSource, VideoResolution};
use livekit::webrtc::video_stream::native::NativeVideoStream;
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
use matrix_sdk::ruma::{OwnedDeviceId, OwnedRoomId, OwnedUserId, RoomId, UserId};
use matrix_sdk::stream::StreamExt;
use matrix_sdk::{reqwest, HttpError};
use nokhwa::utils::FrameFormat;
use reqwest::header;
use ringbuffer::RingBuffer;
use serde::{Deserialize, Serialize};
use smallvec::smallvec;
use std::fmt::Display;
use std::rc::Weak;
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use thegrid_common::outbound_track::{OutboundTrack, OutboundTrackStatus};
use thegrid_common::room::active_call_participants::track_active_call_participants;
use thegrid_common::session::session_manager::SessionManager;
use thegrid_common::sfx;
use thegrid_common::sfx::SoundEffect;
use thegrid_common::tokio_helper::TokioHelper;
use yuv::{
    yuv420_to_bgra, yuv420_to_rgba, yuv422_to_uyvy422, yuyv422_to_yuv422, BufferStoreMut,
    YuvChromaSubsampling, YuvPackedImage, YuvPackedImageMut, YuvPlanarImage, YuvPlanarImageMut,
    YuvRange, YuvStandardMatrix,
};

pub fn setup_thegrid_rtc_livekit() {
    I18N_MANAGER.write().unwrap().load_source(tr_load!());
}

pub struct LivekitCall {
    room: OwnedRoomId,
    state: CallState,

    our_track_sids: HashMap<TrackType, TrackSid>,
    active_devices: HashMap<TrackSid, Entity<OutboundTrack>>,

    subscribed_streams: Vec<SubscribedStream>,
    active_call_participants_state: Entity<Vec<RoomMember>>,
    muted_streams: HashSet<TrackSid>,
    active_speakers: HashSet<TrackSid>,
    cached_call_members: Entity<Vec<CallMember>>,
    video_stream_images: HashMap<TrackSid, Arc<RenderImage>>,

    cancellation_source: CancellationTokenSource,
    started_at: Instant,

    on_hold: bool,
}

#[derive(Clone)]
pub struct CallMember {
    room_member: RoomMember,
    device_id: Option<OwnedDeviceId>,
    mic_state: StreamState,
    camera_state: StreamState,
    screenshare_state: StreamState,

    mic_active: bool,
}

#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug)]
pub enum TrackType {
    Mic,
    Camera,
    Screenshare,
    ScreenshareAudio,
}

#[derive(Clone, PartialEq)]
pub enum StreamState {
    Unavailable,
    Off,
    On(TrackSid),
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
    pub fn new(
        room_id: OwnedRoomId,
        initial_streams: HashMap<TrackType, Entity<OutboundTrack>>,
        cx: &mut Context<Self>,
    ) -> Self {
        let active_call_participants_state = track_active_call_participants(room_id.clone(), cx);

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
        let room_id_clone = room_id.clone();
        let device_id = client.device_id().unwrap().to_owned();

        let rtc_foci = session_manager.rtc_foci().clone();

        let cancellation_source = CancellationTokenSource::new();
        let cancellation_token = cancellation_source.token();

        cx.spawn(
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                let room_clone = room.clone();

                let service_url = match get_focus_url(room_clone, rtc_foci, cx).await {
                    Ok(url) => url,
                    Err(FocusUrlError::RoomError) => {
                        let _ = weak_this.update(cx, |this, cx| {
                            this.state = CallState::Error(CallError::RoomError);
                            cx.notify();
                        });
                        return;
                    }
                    Err(FocusUrlError::NoRtcFocus) => {
                        let _ = weak_this.update(cx, |this, cx| {
                            this.state = CallState::Error(CallError::NoRtcFocus);
                            cx.notify();
                        });
                        return;
                    }
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
                        Room::connect(&livekit_jwt.url, &livekit_jwt.jwt, room_options).await
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

                cx.spawn(async move |cx: &mut AsyncApp| {
                    cancellation_token.wait().await;
                    let _ = livekit_room.close().await;
                })
                .detach();

                let weak_this_clone = weak_this.clone();
                cx.spawn(async move |cx: &mut AsyncApp| {
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
                            RoomEvent::ActiveSpeakersChanged { speakers } => {
                                if weak_this_clone
                                    .update(cx, |this, cx| {
                                        this.active_speakers = speakers
                                            .iter()
                                            .flat_map(|participant| {
                                                participant
                                                    .track_publications()
                                                    .keys()
                                                    .cloned()
                                                    .collect::<Vec<_>>()
                                            })
                                            .collect();
                                        cx.notify();
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

                    for (track_type, track_device) in initial_streams {
                        this.publish_track(track_type, Some(track_device), cx);
                    }
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
                    SoundEffect::CallJoin.play();
                } else if old_call_members > call_members.len() {
                    SoundEffect::CallLeave.play();
                }
            }

            this.cached_call_members.write(cx, call_members);
        })
        .detach();

        SoundEffect::CallJoin.play();

        cx.observe_global::<LivekitCallManager>(|this, cx| {
            let call_manager = cx.global::<LivekitCallManager>();
            if call_manager
                .current_call()
                .is_none_or(|current_call| current_call != cx.entity())
                && !this.on_hold
            {
                this.on_hold = true;
                cx.notify();
            }
        })
        .detach();

        LivekitCall {
            room: room_id,
            state: CallState::Connecting,
            cancellation_source,
            started_at: Instant::now(),
            our_track_sids: HashMap::new(),
            active_call_participants_state,
            subscribed_streams: Vec::new(),
            muted_streams: HashSet::new(),
            active_speakers: HashSet::new(),
            video_stream_images: HashMap::new(),
            on_hold: false,
            cached_call_members,
            active_devices: HashMap::new(),
        }
    }

    pub fn room(&self) -> &RoomId {
        &self.room
    }

    pub fn on_hold(&self) -> bool {
        self.on_hold
    }

    pub fn set_on_hold(&mut self, on_hold: bool, cx: &mut Context<Self>) {
        self.on_hold = on_hold;
        cx.notify();
    }

    pub fn active_camera(&self) -> Option<Entity<OutboundTrack>> {
        self.our_track_sids
            .get(&TrackType::Camera)
            .and_then(|sid| self.active_devices.get(sid))
            .cloned()
    }

    pub fn active_screenshare(&self) -> Option<Entity<OutboundTrack>> {
        self.our_track_sids
            .get(&TrackType::Screenshare)
            .and_then(|sid| self.active_devices.get(sid))
            .cloned()
    }

    fn publish_track(
        &mut self,
        track_type: TrackType,
        track_device: Option<Entity<OutboundTrack>>,
        cx: &mut Context<Self>,
    ) {
        let local_participant = match &self.state {
            CallState::Active { local_participant } => local_participant.clone(),
            _ => return,
        };

        if let Some(track_sid) = self.our_track_sids.remove(&track_type) {
            self.active_devices.remove(&track_sid);
            cx.notify();

            let local_participant_clone = local_participant.clone();
            cx.spawn(
                async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                    let _ = cx
                        .spawn_tokio(async move {
                            local_participant_clone.unpublish_track(&track_sid).await
                        })
                        .await;
                },
            )
            .detach();
        }

        let Some(track_device) = track_device else {
            return;
        };

        let device = track_device.read(cx);
        match track_type {
            TrackType::Camera | TrackType::Screenshare => {
                if !device.has_video() {
                    panic!(
                        "Tried to publish a video track but the device does not have a \
                        video source. TrackType: {:?}",
                        track_type
                    )
                }

                let frame_width = device.width();
                let frame_height = device.height();
                let source = NativeVideoSource::new(
                    VideoResolution {
                        width: frame_width,
                        height: frame_height,
                    },
                    match track_type {
                        TrackType::Screenshare => true,
                        TrackType::Camera => false,
                        _ => unreachable!(),
                    },
                );
                let track = LocalVideoTrack::create_video_track(
                    match track_type {
                        TrackType::Screenshare => "screenshare",
                        TrackType::Camera => "camera",
                        _ => unreachable!(),
                    },
                    RtcVideoSource::Native(source.clone()),
                );

                let device_entity_clone = track_device.clone();
                cx.spawn(
                    async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                        let Ok(publication) = cx
                            .spawn_tokio(async move {
                                local_participant
                                    .publish_track(
                                        LocalTrack::Video(track),
                                        TrackPublishOptions {
                                            source: match track_type {
                                                TrackType::Screenshare => TrackSource::Screenshare,
                                                TrackType::Camera => TrackSource::Camera,
                                                _ => unreachable!(),
                                            },
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
                        let timestamp = Instant::now();
                        let _ = weak_this.update(cx, |call, cx| {
                            call.our_track_sids.insert(track_type, sid.clone());
                            call.active_devices
                                .insert(sid.clone(), device_entity_clone.clone());
                            cx.notify();

                            cx.observe(&device_entity_clone, move |call, device_entity, cx| {
                                let device = device_entity.read(cx);

                                if matches!(
                                    device.status(),
                                    OutboundTrackStatus::Error(_) | OutboundTrackStatus::Terminated
                                ) {
                                    // Unpublish the track
                                    if call
                                        .our_track_sids
                                        .get(&track_type)
                                        .is_some_and(|sid_option| sid_option == &sid)
                                    {
                                        call.publish_track(track_type, None, cx);
                                    }
                                    return;
                                }

                                let time_passed = timestamp.elapsed().as_millis();

                                let Some(i422_frame_buffer) = device.i422_frame_buffer() else {
                                    return;
                                };

                                let video_frame =
                                    livekit::webrtc::video_frame::VideoFrame::<I422Buffer> {
                                        rotation: VideoRotation::VideoRotation0,
                                        timestamp_us: time_passed as i64,
                                        buffer: i422_frame_buffer,
                                    };

                                source.capture_frame(&video_frame);
                            })
                            .detach()
                        });
                    },
                )
                .detach();
            }
            TrackType::ScreenshareAudio | TrackType::Mic => {
                if !device.has_audio() {
                    panic!(
                        "Tried to publish an audio track but the device does not have a \
                        audio source. TrackType: {:?}",
                        track_type
                    )
                }

                let sample_rate = device.sample_rate();
                let channels = device.channels();

                let source =
                    NativeAudioSource::new(Default::default(), sample_rate, channels as u32, 1000);
                let track = LocalAudioTrack::create_audio_track(
                    match track_type {
                        TrackType::Mic => "mic",
                        TrackType::ScreenshareAudio => "screenshare-audio",
                        _ => unreachable!(),
                    },
                    RtcAudioSource::Native(source.clone()),
                );

                if matches!(track_type, TrackType::Mic) {
                    let call_manager = cx.global::<LivekitCallManager>();
                    if *call_manager.mute().read(cx) {
                        track.mute();
                    }

                    let track_clone = track.clone();
                    cx.observe(&call_manager.mute(), move |this, mute, cx| {
                        this.update_audio_track_mute_status(track_clone.clone(), cx);
                    })
                    .detach();
                    let track_clone = track.clone();
                    cx.observe_self(move |this, cx| {
                        this.update_audio_track_mute_status(track_clone.clone(), cx);
                    })
                    .detach();
                }

                let device_entity_clone = track_device.clone();
                cx.spawn(
                    async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                        let Ok(publication) = cx
                            .spawn_tokio(async move {
                                local_participant
                                    .publish_track(
                                        LocalTrack::Audio(track),
                                        TrackPublishOptions {
                                            source: match track_type {
                                                TrackType::Mic => TrackSource::Microphone,
                                                TrackType::ScreenshareAudio => {
                                                    TrackSource::ScreenshareAudio
                                                }
                                                _ => unreachable!(),
                                            },
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
                            call.our_track_sids.insert(track_type, sid.clone());
                            call.active_devices
                                .insert(sid.clone(), device_entity_clone.clone());
                            cx.notify();

                            cx.observe(&device_entity_clone, move |call, device_entity, cx| {
                                let Some(samples): Option<Vec<_>> =
                                    device_entity.update(cx, |device, cx| {
                                        let buffer = device.audio_sample_buffer();
                                        if buffer.len() >= 2048 {
                                            Some(buffer.drain().collect())
                                        } else {
                                            None
                                        }
                                    })
                                else {
                                    return;
                                };

                                let audio_frame = AudioFrame {
                                    num_channels: channels as u32,
                                    sample_rate,
                                    samples_per_channel: (samples.len() / channels as usize) as u32,
                                    data: samples.into(),
                                };

                                let source = source.clone();
                                cx.spawn(async move |_: WeakEntity<Self>, cx: &mut AsyncApp| {
                                    cx.spawn_tokio(async move {
                                        source.capture_frame(&audio_frame).await
                                    })
                                    .await
                                })
                                .detach();
                            })
                            .detach();
                        });
                    },
                )
                .detach();
            }
        }
    }

    pub fn call_members(&self) -> Entity<Vec<CallMember>> {
        self.cached_call_members.clone()
    }

    pub fn image_for_track(&self, track_sid: TrackSid) -> Option<Arc<RenderImage>> {
        self.video_stream_images.get(&track_sid).cloned()
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
        let active_call_participants = self.active_call_participants_state.read(cx).clone();
        let mut this_device_processed = false;
        for participant in active_call_participants.iter() {
            if let Some((tuple, participant)) = devices.iter().find_map(|tuple| {
                if tuple.0 == participant.user_id() {
                    Some((tuple, participant))
                } else {
                    None
                }
            }) {
                let subscribed_streams = self
                    .subscribed_streams
                    .iter()
                    .filter(|stream| stream.user_id == tuple.0 && stream.device_id == tuple.1)
                    .collect::<Vec<_>>();

                let mut call_member = CallMember {
                    room_member: participant.clone(),
                    device_id: Some(tuple.1.clone()),
                    mic_state: StreamState::Unavailable,
                    screenshare_state: StreamState::Unavailable,
                    camera_state: StreamState::Unavailable,
                    mic_active: false,
                };

                for stream in subscribed_streams {
                    let stream_state = if self.muted_streams.contains(&stream.stream_sid) {
                        StreamState::Off
                    } else {
                        StreamState::On(stream.stream_sid.clone())
                    };

                    if self.active_speakers.contains(&stream.stream_sid) {
                        call_member.mic_active = true;
                    }

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
                let (mic_state, camera_state, screenshare_state) = if !this_device_processed
                    && participant.user_id() == this_user_id
                {
                    this_device_processed = true;
                    (
                        if muted {
                            StreamState::Off
                        } else if let Some(track_sid) = self.our_track_sids.get(&TrackType::Mic) {
                            StreamState::On(track_sid.clone())
                        } else {
                            StreamState::Unavailable
                        },
                        if let Some(track_sid) = self.our_track_sids.get(&TrackType::Camera) {
                            StreamState::On(track_sid.clone())
                        } else {
                            StreamState::Unavailable
                        },
                        if let Some(track_sid) = self.our_track_sids.get(&TrackType::Screenshare) {
                            StreamState::On(track_sid.clone())
                        } else {
                            StreamState::Unavailable
                        },
                    )
                } else {
                    (
                        StreamState::Unavailable,
                        StreamState::Unavailable,
                        StreamState::Unavailable,
                    )
                };

                call_members.push(CallMember {
                    room_member: participant.clone(),
                    device_id: None,
                    mic_state,
                    camera_state,
                    screenshare_state,
                    mic_active: false,
                });
            };
        }
        call_members
    }

    fn setup_local_mic(&mut self, cx: &mut Context<Self>) {
        let call_manager = cx.global::<LivekitCallManager>();
        let track = call_manager.active_input_device().read(cx).clone();

        let outbound_track = track.map(|device| open_mic(&device, cx));
        self.publish_track(TrackType::Mic, outbound_track, cx);
    }

    fn start_track(&mut self, track: &RemoteTrack, cx: &mut Context<Self>) {
        let call_manager = cx.global::<LivekitCallManager>();
        match track {
            RemoteTrack::Audio(audio_track) => {
                let audio_track_clone = audio_track.clone();
                let output_device = call_manager.active_output_device();
                cx.observe(&output_device, move |this, output_device, cx| {
                    this.route_audio(audio_track_clone.clone(), output_device, cx);
                })
                .detach();
                self.route_audio(audio_track.clone(), output_device, cx);
            }
            RemoteTrack::Video(video_track) => {
                self.route_video(video_track.clone(), cx);
            }
        }
    }

    fn route_audio(
        &mut self,
        audio_track: RemoteAudioTrack,
        device_entity: Entity<Option<cpal::Device>>,
        cx: &mut Context<Self>,
    ) {
        let call_manager = cx.global::<LivekitCallManager>();
        let call_manager_deaf = call_manager.deaf();
        let cancellation_token = self.cancellation_source.token();

        let (mut producer, mut consumer) = AsyncHeapRb::<i16>::new(16384).split();

        let device = device_entity.read(cx).clone();
        let (sample_rate, channels, output_stream) = if let Some(device) = device {
            let mut supported_device_configs = device.supported_output_configs().unwrap();
            let supported_config = supported_device_configs
                .find_map(|config| {
                    config
                        .try_with_sample_rate(48000)
                        .or_else(|| config.try_with_sample_rate(44100))
                })
                .unwrap();

            let deaf = Arc::new(RwLock::new(false));
            let deaf_clone = deaf.clone();
            cx.observe_self(move |this, cx| {
                *deaf_clone.write().unwrap() = this.is_deaf(cx);
            })
            .detach();
            let deaf_clone = deaf.clone();
            cx.observe(&call_manager_deaf, move |this, _, cx| {
                *deaf_clone.write().unwrap() = this.is_deaf(cx);
            })
            .detach();

            let output_stream = device
                .build_output_stream(
                    &supported_config.config(),
                    move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                        consumer.pop_slice(data);
                        if *deaf.read().unwrap() {
                            data.fill(0);
                        }
                    },
                    move |err| {
                        // Errors? What errors!?
                        error!("cpal: error in output stream: {:?}", err)
                    },
                    None,
                )
                .unwrap();

            (
                supported_config.sample_rate() as i32,
                supported_config.channels() as i32,
                Some(output_stream),
            )
        } else {
            cx.spawn(
                async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                    loop {
                        consumer.pop_until_end(&mut Default::default()).await;
                        if consumer.is_closed() {
                            return;
                        }
                    }
                },
            )
            .detach();
            (44100, 2, None)
        };

        let cancellation_source = CancellationTokenSource::new();
        let cancellation_source_2 = cancellation_source.clone();

        let rtc_track = audio_track.rtc_track();
        let mut audio_stream = NativeAudioStream::new(rtc_track, sample_rate, channels);
        cx.spawn(
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                let _ = cx
                    .spawn_tokio(async move {
                        // Receive the audio frames in a new task
                        while let Some(audio_frame) = audio_stream.next().await {
                            if cancellation_token.is_canceled() {
                                cancellation_source_2.cancel();
                                return Ok(());
                            }

                            if producer.push_exact(&audio_frame.data).await.is_err() {
                                cancellation_source_2.cancel();
                                return Ok(());
                            };
                        }

                        cancellation_source_2.cancel();
                        Ok::<_, anyhow::Error>(())
                    })
                    .await;
            },
        )
        .detach();

        let cancellation_token_2 = cancellation_source.token();
        cx.spawn(
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                cancellation_token_2.wait().await;
                drop(output_stream);
            },
        )
        .detach();

        cx.observe(&device_entity, move |this, device, cx| {
            cancellation_source.cancel();
        })
        .detach();
    }

    fn route_video(&mut self, video_track: RemoteVideoTrack, cx: &mut Context<Self>) {
        let rtc_track = video_track.rtc_track();
        let mut video_stream = NativeVideoStream::new(rtc_track);

        let (tx, rx) = async_channel::bounded(1);

        cx.spawn(
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                let _ = cx
                    .spawn_tokio(async move {
                        // Receive the audio frames in a new task
                        while let Some(video_frame) = video_stream.next().await {
                            let rgba_image = match video_frame.buffer.buffer_type() {
                                VideoBufferType::I420 => {
                                    let i420_frame = video_frame.buffer.to_i420();

                                    let (y_plane, u_plane, v_plane) = i420_frame.data();
                                    let (y_stride, u_stride, v_stride) = i420_frame.strides();

                                    let mut rgba_data = vec![
                                        0;
                                        (i420_frame.width() * i420_frame.height() * 4)
                                            as usize
                                    ];

                                    let yuv_image = YuvPlanarImage {
                                        height: i420_frame.height(),
                                        width: i420_frame.width(),
                                        y_plane,
                                        u_plane,
                                        v_plane,
                                        y_stride,
                                        u_stride,
                                        v_stride,
                                    };
                                    if let Err(e) = yuv420_to_bgra(
                                        &yuv_image,
                                        &mut rgba_data,
                                        i420_frame.width() * 4,
                                        YuvRange::Limited,
                                        YuvStandardMatrix::Bt2020,
                                    ) {
                                        error!("Error converting YUV to RGBA: {}", e);
                                        continue;
                                    }

                                    let Some(rgba_image) = RgbaImage::from_vec(
                                        i420_frame.width(),
                                        i420_frame.height(),
                                        rgba_data,
                                    ) else {
                                        error!("Failed to create RGBA image from YUV data");
                                        continue;
                                    };

                                    rgba_image
                                }
                                _ => {
                                    warn!(
                                        "Unsupported video format: {:?}",
                                        video_frame.buffer.buffer_type()
                                    );
                                    continue;
                                }
                            };

                            if tx.send(rgba_image).await.is_err() {
                                return Ok::<_, anyhow::Error>(());
                            }
                        }

                        Ok::<_, anyhow::Error>(())
                    })
                    .await;
            },
        )
        .detach();

        let track_sid = video_track.sid();
        cx.spawn(
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                while let Ok(frame) = rx.recv().await {
                    let track_sid = track_sid.clone();
                    if weak_this
                        .update(cx, |this, cx| {
                            let render_image =
                                Arc::new(RenderImage::new(smallvec![Frame::new(frame.clone())]));
                            if let Some(old_image) =
                                this.video_stream_images.insert(track_sid, render_image)
                            {
                                // Drop this image from all windows
                                cx.defer(move |cx| {
                                    for window in cx.windows() {
                                        let image = old_image.clone();
                                        let _ = window.update(cx, move |_, window, _| {
                                            let _ = window.drop_image(image);
                                        });
                                    }
                                });
                            }
                            cx.notify()
                        })
                        .is_err()
                    {
                        return;
                    }
                }
            },
        )
        .detach();
    }

    pub fn end_call(&mut self, cx: &mut Context<Self>) {
        self.cancellation_source.cancel();
        self.our_track_sids.clear();
        self.active_devices.clear();

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

        SoundEffect::CallLeave.play();
    }

    pub fn state(&self) -> &CallState {
        &self.state
    }

    fn update_audio_track_mute_status(&mut self, track: LocalAudioTrack, cx: &mut Context<Self>) {
        let call_manager = cx.global::<LivekitCallManager>();
        let mute = call_manager.mute();
        if *mute.read(cx) || self.on_hold {
            track.mute();
        } else {
            track.unmute();
        }
    }

    fn is_deaf(&self, cx: &mut Context<Self>) -> bool {
        if self.on_hold {
            return true;
        }

        let call_manager = cx.global::<LivekitCallManager>();
        *call_manager.deaf().read(cx)
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
