use crate::{decode_livekit_identity, CallError, CallState, LivekitJwtResponse, SubscribedStream};
use contemporary::tokio::tokio_helper::TokioHelper;
use gpui::http_client::http::header;
use gpui::http_client::StatusCode;
use gpui::private::serde_json;
use gpui::private::serde_json::json;
use gpui::{App, AsyncApp, Context, WeakEntity};
use livekit::id::TrackSid;
use livekit::prelude::LocalParticipant;
use livekit::track::RemoteTrack;
use livekit::{participant, publication, track, Room, RoomEvent, RoomOptions};
use log::{error, info};
use matrix_sdk::ruma::api::client::account::request_openid_token;
use matrix_sdk::ruma::events::call::member::{
    ActiveFocus, ActiveLivekitFocus, Application, CallApplicationContent, CallMemberEventContent,
    CallMemberStateKey, CallScope, Focus, LivekitFocus,
};
use matrix_sdk::ruma::{OwnedDeviceId, OwnedRoomId};
use std::collections::HashSet;
use std::rc::Rc;
use std::time::Duration;
use thegrid_common::session::session_manager::SessionManager;
use thegrid_common::surfaces::MainWindowSurface::Call;

#[derive(Clone, Debug)]
pub enum SfuSessionState {
    Connecting,
    Connected { local_participant: LocalParticipant },
    Error(SfuSessionError),
}

impl From<&SfuSessionState> for CallState {
    fn from(value: &SfuSessionState) -> Self {
        match value {
            SfuSessionState::Connecting => CallState::Connecting,
            SfuSessionState::Connected { local_participant } => CallState::Active {
                local_participant: local_participant.clone(),
            },
            SfuSessionState::Error(e) => CallState::Error(e.into()),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum SfuSessionError {
    LivekitJwtRequestFailed,
    LivekitRtcFailed,
}

impl From<&SfuSessionError> for CallError {
    fn from(value: &SfuSessionError) -> Self {
        match value {
            SfuSessionError::LivekitJwtRequestFailed => CallError::LivekitJwtRequestFailed,
            SfuSessionError::LivekitRtcFailed => CallError::LivekitRtcFailed,
        }
    }
}

pub struct SfuSession {
    state: SfuSessionState,
    pub subscribed_streams: Vec<SubscribedStream>,
    pub muted_streams: HashSet<TrackSid>,
    pub active_speakers: HashSet<TrackSid>,
}

impl SfuSession {
    pub fn new(
        room_id: OwnedRoomId,
        service_url: &str,
        openid_token: request_openid_token::v3::Response,
        start_track: impl Fn(&RemoteTrack, &mut App) + 'static,
        cx: &mut Context<Self>,
    ) -> Self {
        let start_track = Rc::new(start_track);
        let session_manager = cx.global::<SessionManager>();
        let client = session_manager.client().unwrap().read(cx).clone();
        let device_id = client.device_id().unwrap().to_owned();

        let service_url = format!("{}/sfu/get", service_url);

        cx.spawn(async move |weak_this: WeakEntity<Self>, cx: &mut AsyncApp| {
            // Get the LiveKit JWT
            let client = zed_reqwest::Client::new();
            let livekit_jwt_response = cx
                .spawn_tokio({
                    let client = client.clone();
                    let device_id = device_id.clone();
                    let room_id = room_id.clone();
                    async move {
                        client
                            .post(service_url)
                            .body(
                                json!({
                                    "device_id": device_id.to_string(),
                                    "openid_token": {
                                        "access_token": openid_token.access_token,
                                        "expires_in": openid_token.expires_in.as_secs(),
                                        "matrix_server_name": openid_token.matrix_server_name.to_string(),
                                        "token_type": openid_token.token_type.to_string()
                                    },
                                    "room": room_id
                                })
                                    .to_string(),
                            )
                            .header(header::CONTENT_TYPE, "application/json")
                            .send()
                            .await
                    }
                })
                .await;
            let livekit_jwt = match livekit_jwt_response {
                Ok(livekit_jwt) => livekit_jwt,
                Err(e) => {
                    error!("Call failed: {}: {e:?}", CallError::LivekitJwtRequestFailed);
                    let _ = weak_this.update(cx, |this, cx| {
                        this.state = SfuSessionState::Error(SfuSessionError::LivekitJwtRequestFailed);
                        cx.notify();
                    });
                    return;
                }
            };

            let Ok(livekit_jwt) = livekit_jwt.text().await else {
                error!(
                    "Call failed: {}: response could not be interpreted as text",
                    CallError::LivekitJwtRequestFailed
                );
                let _ = weak_this.update(cx, |this, cx| {
                    this.state = SfuSessionState::Error(SfuSessionError::LivekitJwtRequestFailed);
                    cx.notify();
                });
                return;
            };

            let Ok(livekit_jwt) = serde_json::from_str::<LivekitJwtResponse>(&livekit_jwt) else {
                error!(
                    "Call failed: {}: response could not be interpreted as JSON",
                    CallError::LivekitJwtRequestFailed
                );
                error!("The response was: {:?}", livekit_jwt);
                let _ = weak_this.update(cx, |this, cx| {
                    this.state = SfuSessionState::Error(SfuSessionError::LivekitJwtRequestFailed);
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
                        this.state = SfuSessionState::Error(SfuSessionError::LivekitRtcFailed);
                        cx.notify();
                    });
                    return;
                }
            };

            let local_participant = livekit_room.local_participant();
            let _ = weak_this.update(cx, |this, cx| {
                this.state = SfuSessionState::Connected {
                    local_participant,
                };
                cx.notify();
            });

            let weak_this_clone = weak_this.clone();
            cx.spawn(async move |cx: &mut AsyncApp| {
                loop {
                    let Some(event) = tokio::task::unconstrained(room_events.recv()).await else {
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

                                    cx.defer({let track = track.clone(); let start_track = start_track.clone(); move |cx| {
                                        start_track(&track, cx);
                                    }});
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
                                    cx.notify();
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
                                    cx.notify();
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
                                    cx.notify();
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
                drop(livekit_room);
            })
            .detach();
        })
        .detach();

        Self {
            state: SfuSessionState::Connecting,
            subscribed_streams: Default::default(),
            muted_streams: Default::default(),
            active_speakers: Default::default(),
        }
    }

    pub fn state(&self) -> &SfuSessionState {
        &self.state
    }
}
