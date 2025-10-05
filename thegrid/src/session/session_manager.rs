use crate::session::caches::Caches;
use crate::session::verification_requests_cache::VerificationRequestsCache;
use contemporary::application::Details;
use gpui::http_client::anyhow;
use gpui::{App, AppContext, AsyncApp, Entity, Global, WeakEntity};
use gpui_tokio::Tokio;
use log::error;
use matrix_sdk::Client;
use matrix_sdk::authentication::matrix::MatrixSession;
use matrix_sdk::config::SyncSettings;
use matrix_sdk::ruma::events::key::verification::request::ToDeviceKeyVerificationRequestEvent;
use matrix_sdk::store::RoomLoadSettings;
use std::fs::{create_dir_all, read_dir};
use std::path::PathBuf;
use uuid::Uuid;

pub struct SessionManager {
    current_session: Option<Session>,
    current_session_client: Option<Entity<Client>>,
    current_caches: Option<Caches>,
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

            cx.spawn(async move |cx: &mut AsyncApp| {
                let client = Tokio::spawn_result(cx, async move {
                    Client::builder()
                        .server_name(user_id.server_name())
                        .sqlite_store(store_dir, None)
                        .build()
                        .await
                        .map_err(|e| anyhow!(e))
                })
                .unwrap()
                .await
                .unwrap();

                let client_clone = client.clone();
                Tokio::spawn_result(cx, async move {
                    client_clone
                        .matrix_auth()
                        .restore_session(matrix_session, RoomLoadSettings::All)
                        .await
                        .map_err(|e| anyhow!(e))
                })
                .unwrap()
                .await
                .unwrap();

                client.event_cache().subscribe().unwrap();

                let client_clone = client.clone();
                cx.spawn(async move |cx: &mut AsyncApp| {
                    let sync_result = Tokio::spawn_result(cx, async move {
                        client_clone
                            .sync(SyncSettings::default())
                            .await
                            .map_err(|e| anyhow!(e))
                    })
                    .unwrap()
                    .await
                    .unwrap_err();

                    cx.update_global::<Self, ()>(|session_manager, cx| {
                        error!("Sync error: {:?}", sync_result);
                        session_manager.current_session = None;
                        session_manager.current_session_client = None;
                        // TODO: Explain to the user why we logged out
                    })
                    .unwrap();
                })
                .detach();

                cx.update_global::<Self, ()>(|session_manager, cx| {
                    session_manager.current_caches = Some(Caches::new(&client, cx));
                    session_manager.current_session_client = Some(cx.new(|_| client));
                })
                .unwrap();
            })
            .detach();
        }
    }

    pub fn current_session(&self) -> Option<Session> {
        self.current_session.clone()
    }

    pub fn client(&self) -> Option<Entity<Client>> {
        self.current_session_client.clone()
    }

    pub fn verification_requests(&self) -> Entity<VerificationRequestsCache> {
        self.current_caches
            .as_ref()
            .unwrap()
            .verification_requests
            .clone()
    }
}

impl Global for SessionManager {}

pub fn setup_session_manager(cx: &mut App) {
    cx.set_global(SessionManager {
        current_session: None,
        current_session_client: None,
        current_caches: None,
    });
}

#[derive(Clone)]
pub struct Session {
    pub uuid: Uuid,
    pub matrix_session: MatrixSession,
    pub session_dir: PathBuf,
}
