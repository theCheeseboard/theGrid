use async_ringbuf::traits::{AsyncProducer, Consumer, Producer, Split};
pub mod active_call_sidebar_alert;
pub mod call_manager;

use crate::call_manager::LivekitCallManager;
use async_ringbuf::AsyncHeapRb;
use cancellation_token::CancellationTokenSource;
use cntp_i18n::{I18N_MANAGER, tr, tr_load};
use cpal::Host;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use gpui::private::{anyhow, serde_json};
use gpui::{AppContext, AsyncApp, BorrowAppContext, Context, WeakEntity};
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
use matrix_sdk::ruma::OwnedRoomId;
use matrix_sdk::ruma::api::client::account::request_openid_token;
use matrix_sdk::ruma::api::client::account::request_openid_token::v3::Response;
use matrix_sdk::ruma::api::client::discovery::discover_homeserver::RtcFocusInfo;
use matrix_sdk::ruma::events::call::member::{
    ActiveFocus, ActiveLivekitFocus, Application, CallApplicationContent, CallMemberEvent,
    CallMemberEventContent, CallMemberStateKey, CallScope, Focus, LivekitFocus,
};
use matrix_sdk::ruma::events::rtc::notification::RtcNotificationEvent;
use matrix_sdk::ruma::exports::serde_json::json;
use matrix_sdk::stream::StreamExt;
use matrix_sdk::{HttpError, reqwest};
use reqwest::header;
use serde::{Deserialize, Serialize};
use std::fmt::Display;
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

    cancellation_source: CancellationTokenSource,
    started_at: Instant,
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
    NoRtcFocus,
    OpenIdTokenRequestFailed,
    LivekitJwtRequestFailed,
    LivekitRtcFailed,
}

#[derive(Serialize, Deserialize)]
struct LivekitJwtResponse {
    url: String,
    jwt: String,
}

impl Display for CallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match self {
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
        };
        write!(f, "{}", str)
    }
}

impl LivekitCall {
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
        let room_id_clone = room_id.clone();
        let device_id = client.device_id().unwrap().to_owned();

        let rtc_foci = session_manager.rtc_foci().clone();

        cx.spawn(
            async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
                // TODO: What if there exists an active call on a different LiveKit server?
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

                let Some(RtcFocusInfo::LiveKit(livekit_focus)) = rtc_foci
                    .iter()
                    .find(|focus| matches!(focus, RtcFocusInfo::LiveKit(_)))
                else {
                    let _ = weak_this.update(cx, |this, cx| {
                        this.state = CallState::Error(CallError::NoRtcFocus);
                        cx.notify();
                    });
                    return;
                };

                // Get the LiveKit JWT
                let client = reqwest::Client::new();
                let service_url = livekit_focus.service_url.clone();
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
                        this.state = CallState::Error(CallError::LivekitRtcFailed);
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
                                if weak_this_clone
                                    .update(cx, |this, cx| {
                                        this.start_track(track, cx);
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
            },
        )
        .detach();

        let cpal_host = cpal::default_host();

        LivekitCall {
            room: room_id,
            state: CallState::Connecting,
            cpal_output_device: cpal_host.default_output_device(),
            cpal_input_device: cpal_host.default_input_device(),
            cancellation_source: CancellationTokenSource::new(),
            started_at: Instant::now(),
            mic_track_sid: None,
        }
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

        let mut supported_device_configs = device.supported_output_configs().unwrap();
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
        }).detach();

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
    }

    pub fn state(&self) -> &CallState {
        &self.state
    }
}
