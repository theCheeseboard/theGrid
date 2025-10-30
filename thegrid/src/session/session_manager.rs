use crate::session::account_cache::AccountCache;
use crate::session::caches::Caches;
use crate::session::devices_cache::DevicesCache;
use crate::session::error_handling::{
    ClientError, RecoverableClientError, TerminalClientError, handle_error,
};
use crate::session::media_cache::MediaCache;
use crate::session::notifications::trigger_notification;
use crate::session::room_cache::RoomCache;
use crate::session::verification_requests_cache::VerificationRequestsCache;
use crate::tokio_helper::TokioHelper;
use contemporary::application::Details;
use gpui::http_client::anyhow;
use gpui::private::anyhow;
use gpui::{App, AppContext, AsyncApp, Context, Entity, Global, Task, WeakEntity};
use gpui_tokio::Tokio;
use log::{error, info};
use matrix_sdk::authentication::matrix::MatrixSession;
use matrix_sdk::config::SyncSettings;
use matrix_sdk::ruma::OwnedUserId;
use matrix_sdk::ruma::api::error::FromHttpResponseError;
use matrix_sdk::ruma::events::key::verification::request::ToDeviceKeyVerificationRequestEvent;
use matrix_sdk::store::RoomLoadSettings;
use matrix_sdk::sync::Notification;
use matrix_sdk::{Client, Error, HttpError, LoopCtrl, Room, RumaApiError};
use matrix_sdk_ui::sync_service::{State, SyncService};
use std::fs::{create_dir_all, read_dir, remove_dir_all};
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

pub struct SessionManager {
    current_session: Option<Session>,
    current_session_client: Option<Entity<Client>>,
    current_caches: Option<Caches>,
    current_client_error: ClientError,
}

impl SessionManager {
    pub fn sessions(&self, cx: &App) -> Vec<Session> {
        let details = cx.global::<Details>();
        let directories = details.standard_dirs().unwrap();
        let data_dir = directories.data_dir();
        let session_dir = data_dir.join("sessions");

        create_dir_all(&session_dir).unwrap();

        let dir = read_dir(&session_dir).unwrap();
        dir.filter_map(|entry| {
            let entry = entry.ok()?;
            if entry.metadata().ok()?.is_dir() {
                let uuid = Uuid::parse_str(entry.file_name().to_str()?).ok()?;
                let session_file = entry.path().join("session.json");
                let session_file = std::fs::read_to_string(session_file).ok()?;
                let session_file = serde_json::from_str::<MatrixSession>(&session_file).ok()?;
                Some(Session {
                    uuid,
                    matrix_session: session_file,
                    session_dir: entry.path(),
                })
            } else {
                None
            }
        })
        .collect()
    }

    pub fn set_session(&mut self, uuid: Uuid, cx: &mut App) {
        let session = self
            .sessions(cx)
            .into_iter()
            .find(|session| session.uuid == uuid);
        if let Some(session) = session {
            self.current_session = Some(session.clone());

            let user_id = session.matrix_session.meta.user_id.clone();
            let store_dir = session.session_dir.join("store");
            let matrix_session = session.matrix_session;

            let homeserver_file = session.session_dir.join("homeserver");
            let homeserver = std::fs::read_to_string(homeserver_file);

            cx.spawn(async move |cx: &mut AsyncApp| {
                if let Err(error) =
                    Self::setup_session(user_id, store_dir, matrix_session, homeserver.ok(), cx)
                        .await
                {
                    error!("Unable to create client: {error:?}");
                    let _ = cx.update_global::<Self, ()>(|session_manager, cx| {
                        session_manager.current_client_error =
                            ClientError::Terminal(TerminalClientError::UnknownError);
                    });
                };
            })
            .detach();
        }
    }

    async fn setup_session(
        user_id: OwnedUserId,
        store_dir: PathBuf,
        matrix_session: MatrixSession,
        homeserver: Option<String>,
        cx: &mut AsyncApp,
    ) -> anyhow::Result<()> {
        let client = cx
            .spawn_tokio(async move {
                {
                    if let Some(homeserver) = homeserver {
                        Client::builder().homeserver_url(homeserver)
                    } else {
                        Client::builder().server_name(user_id.server_name())
                    }
                }
                .sqlite_store(store_dir, None)
                .build()
                .await
            })
            .await?;

        let client_clone = client.clone();
        cx.spawn_tokio(async move {
            client_clone
                .matrix_auth()
                .restore_session(matrix_session, RoomLoadSettings::All)
                .await
        })
        .await?;

        client.event_cache().subscribe()?;

        let (tx_notification, rx_notification) = async_channel::bounded(1);

        let client_clone = client.clone();
        cx.spawn(async move |cx: &mut AsyncApp| {
            let tx_notification = tx_notification.clone();
            cx.spawn_tokio::<_, _, anyhow::Error>(async move {
                client_clone
                    .register_notification_handler(move |notification, room, client| {
                        let tx_notification = tx_notification.clone();
                        async move {
                            let _ = tx_notification.send((notification, room)).await;
                        }
                    })
                    .await;
                Ok(())
            })
            .await
        })
        .detach();
        cx.spawn(async move |cx: &mut AsyncApp| {
            while let Ok((notification, room)) = rx_notification.recv().await {
                if cx
                    .update(|cx| {
                        trigger_notification(notification, room, cx);
                    })
                    .is_err()
                {
                    return;
                };
            }
        })
        .detach();

        let (tx_clear_error, rx_clear_error) = async_channel::bounded(1);
        cx.spawn(async move |cx: &mut AsyncApp| {
            loop {
                if rx_clear_error.recv().await.is_err() {
                    return;
                };

                if cx
                    .update_global::<Self, ()>(|session_manager, cx| {
                        session_manager.current_client_error = ClientError::None;
                    })
                    .is_err()
                {
                    return;
                }
            }
        })
        .detach();

        let client_clone = client.clone();
        cx.spawn(async move |cx: &mut AsyncApp| {
            loop {
                let client_clone = client_clone.clone();
                let tx_clear_error = tx_clear_error.clone();
                let sync_result = cx
                    .spawn_tokio(async move {
                        client_clone
                            .sync_with_callback(SyncSettings::default(), |_| {
                                let tx_clear_error = tx_clear_error.clone();
                                async move {
                                    if tx_clear_error.send(()).await.is_err() {
                                        return LoopCtrl::Break;
                                    };

                                    LoopCtrl::Continue
                                }
                            })
                            .await
                    })
                    .await
                    .unwrap_err();

                let error = handle_error(&sync_result);
                match error {
                    ClientError::None => {}
                    ClientError::Terminal(_) => {
                        error!("Sync error: {sync_result:?}");
                        let _ = cx.update_global::<Self, ()>(|session_manager, cx| {
                            session_manager.current_client_error = error;
                        });

                        return;
                    }
                    ClientError::Recoverable(_) => {
                        let _ = cx.update_global::<Self, ()>(|session_manager, cx| {
                            session_manager.current_client_error = error;
                        });
                    }
                }
            }
        })
        .detach();

        cx.update_global::<Self, ()>(|session_manager, cx| {
            session_manager.current_caches = Some(Caches::new(&client, cx));
            session_manager.current_session_client = Some(cx.new(|_| client));
        })?;

        Ok(())
    }

    pub fn clear_session(&mut self) {
        self.current_session = None;
        self.current_session_client = None;
        self.current_caches = None;
        self.current_client_error = ClientError::None;
    }

    pub fn current_session(&self) -> Option<Session> {
        self.current_session.clone()
    }

    pub fn client(&self) -> Option<Entity<Client>> {
        self.current_session_client.clone()
    }

    pub fn error(&self) -> ClientError {
        self.current_client_error
    }

    pub fn verification_requests(&self) -> Entity<VerificationRequestsCache> {
        self.current_caches
            .as_ref()
            .unwrap()
            .verification_requests
            .clone()
    }

    pub fn current_account(&self) -> Entity<AccountCache> {
        self.current_caches.as_ref().unwrap().account_cache.clone()
    }

    pub fn devices(&self) -> Entity<DevicesCache> {
        self.current_caches.as_ref().unwrap().devices_cache.clone()
    }

    pub fn media(&self) -> &MediaCache {
        &self.current_caches.as_ref().unwrap().media_cache
    }

    pub fn rooms(&self) -> Entity<RoomCache> {
        self.current_caches.as_ref().unwrap().room_cache.clone()
    }
}

impl Global for SessionManager {}

pub fn setup_session_manager(cx: &mut App) {
    cx.set_global(SessionManager {
        current_session: None,
        current_session_client: None,
        current_caches: None,
        current_client_error: ClientError::None,
    });
}

#[derive(Clone)]
pub struct Session {
    pub uuid: Uuid,
    pub matrix_session: MatrixSession,
    pub session_dir: PathBuf,
}
