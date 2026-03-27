use crate::session::account_cache::AccountCache;
use crate::session::caches::Caches;
use crate::session::database_secret::{DatabaseSecret, DatabaseSecretExt};
use crate::session::devices_cache::DevicesCache;
use crate::session::error_handling::{handle_error, ClientError, TerminalClientError};
use crate::session::ignored_users_cache::IgnoredUsersCache;
use crate::session::media_cache::MediaCache;
use crate::session::notifications::trigger_notification;
use crate::session::room_cache::RoomCache;
use crate::session::spaces_cache::SpacesCache;
use crate::session::sso_login::SsoLogin;
use crate::session::verification_requests_cache::VerificationRequestsCache;
use crate::tokio_helper::TokioHelper;
use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use contemporary::application::Details;
use gpui::http_client::anyhow;
use gpui::private::anyhow;
use gpui::{App, AppContext, AsyncApp, Context, Entity, Global, Task, WeakEntity};
use gpui_tokio::Tokio;
use imbl::hashmap::Entry;
use imbl::shared_ptr::DefaultSharedPtr;
use imbl::HashMap;
use keyring::default::default_credential_builder;
use keyring::Credential;
use log::{error, info};
use matrix_sdk::authentication::matrix::MatrixSession;
use matrix_sdk::config::SyncSettings;
use matrix_sdk::ruma::api::client::discovery::discover_homeserver::RtcFocusInfo;
use matrix_sdk::ruma::api::error::FromHttpResponseError;
use matrix_sdk::ruma::events::key::verification::request::ToDeviceKeyVerificationRequestEvent;
use matrix_sdk::ruma::OwnedUserId;
use matrix_sdk::store::RoomLoadSettings;
use matrix_sdk::sync::Notification;
use matrix_sdk::{Client, Error, HttpError, LoopCtrl, Room, RumaApiError};
use matrix_sdk_ui::spaces::{SpaceRoomList, SpaceService};
use matrix_sdk_ui::sync_service::{State, SyncService};
use std::cell::RefCell;
use std::fmt::Display;
use std::fs::{create_dir_all, read_dir, remove_dir_all};
use std::hash::RandomState;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

pub struct SessionManager {
    current_session: Option<Session>,
    current_session_client: Option<Entity<Client>>,
    current_caches: Option<Caches>,
    current_client_error: ClientError,
    secrets_cache: RefCell<HashMap<Uuid, DatabaseSecret>>,
    sso_login_entity: WeakEntity<Option<SsoLogin>>,
}

pub enum SessionSecretPurpose {
    Database,
    Session,
}

impl Display for SessionSecretPurpose {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            SessionSecretPurpose::Database => "database",
            SessionSecretPurpose::Session => "session",
        };
        write!(f, "{}", str)
    }
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

                let mut secrets_cache_borrow = self.secrets_cache.borrow_mut();
                let cache_entry = secrets_cache_borrow.entry(uuid);
                let secrets = match cache_entry {
                    Entry::Occupied(secret) => secret.get().clone(),
                    Entry::Vacant(vacant) => vacant
                        .insert(
                            self.session_secrets(&uuid, cx)
                                .ok()
                                .and_then(|secrets| secrets.get_database_secret().ok())?,
                        )
                        .clone(),
                };

                Some(Session {
                    uuid,
                    secrets,
                    session_dir: entry.path(),
                })
            } else {
                None
            }
        })
        .collect()
    }

    pub fn session_secrets(&self, session: &Uuid, cx: &App) -> keyring::Result<Box<Credential>> {
        let details = cx.global::<Details>();
        default_credential_builder().build(
            None,
            details.generatable.desktop_entry,
            &session.to_string(),
        )
    }

    pub fn set_session(&mut self, uuid: Uuid, cx: &mut App) {
        let session = self
            .sessions(cx)
            .into_iter()
            .find(|session| session.uuid == uuid);
        if let Some(session) = session {
            self.current_session = Some(session.clone());

            let user_id = session.secrets.session_meta().unwrap().user_id.clone();
            let store_dir = session.session_dir.join("store");

            let homeserver_file = session.session_dir.join("homeserver");
            let homeserver = std::fs::read_to_string(homeserver_file);

            cx.spawn(async move |cx: &mut AsyncApp| {
                if let Err(error) =
                    Self::setup_session(user_id, store_dir, session.secrets, homeserver.ok(), cx)
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
        secrets: DatabaseSecret,
        homeserver: Option<String>,
        cx: &mut AsyncApp,
    ) -> anyhow::Result<()> {
        let database_password = secrets.database_password();
        let client = cx
            .spawn_tokio(async move {
                {
                    if let Some(homeserver) = homeserver {
                        Client::builder().homeserver_url(homeserver)
                    } else {
                        Client::builder().server_name(user_id.server_name())
                    }
                }
                .sqlite_store(store_dir, Some(&database_password))
                .handle_refresh_tokens()
                .build()
                .await
            })
            .await?;

        if let Some(oauth_session) = secrets.oauth_session() {
            cx.spawn_tokio({
                let client = client.clone();
                async move {
                    client
                        .oauth()
                        .restore_session(oauth_session, RoomLoadSettings::All)
                        .await
                }
            })
            .await?;
        } else {
            let matrix_session = secrets.matrix_session().unwrap().clone();
            cx.spawn_tokio({
                let client = client.clone();
                async move {
                    client
                        .matrix_auth()
                        .restore_session(matrix_session, RoomLoadSettings::All)
                        .await
                }
            })
            .await?;
        }

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

        let client_clone = client.clone();
        cx.spawn(async move |cx: &mut AsyncApp| {
            let rtc_foci = cx
                .spawn_tokio(async move {
                    let _ = client_clone.reset_well_known().await;
                    client_clone.rtc_foci().await
                })
                .await;

            let _ = cx.update_global::<Self, ()>(|session_manager, _| {
                session_manager.current_caches.as_mut().unwrap().rtc_foci =
                    rtc_foci.unwrap_or_default();
            });
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

    pub fn ignored_users(&self) -> Entity<IgnoredUsersCache> {
        self.current_caches
            .as_ref()
            .unwrap()
            .ignored_users_cache
            .clone()
    }

    pub fn rtc_foci(&self) -> &Vec<RtcFocusInfo> {
        &self.current_caches.as_ref().unwrap().rtc_foci
    }

    pub fn set_sso_login_entity(&mut self, entity: WeakEntity<Option<SsoLogin>>) {
        self.sso_login_entity = entity;
    }

    pub fn insert_sso_login(&mut self, sso_login_value: SsoLogin, cx: &mut App) {
        let _ = self.sso_login_entity.update(cx, |sso_login, cx| {
            let _ = sso_login.insert(sso_login_value);
            cx.notify();
        });
        self.sso_login_entity = WeakEntity::new_invalid();
    }

    pub fn spaces(&self) -> Entity<SpacesCache> {
        self.current_caches.as_ref().unwrap().spaces_cache.clone()
    }
}

impl Global for SessionManager {}

pub fn setup_session_manager(cx: &mut App) {
    cx.set_global(SessionManager {
        current_session: None,
        current_session_client: None,
        current_caches: None,
        current_client_error: ClientError::None,
        secrets_cache: RefCell::new(HashMap::new()),
        sso_login_entity: WeakEntity::new_invalid(),
    });
}

#[derive(Clone)]
pub struct Session {
    pub uuid: Uuid,
    pub secrets: DatabaseSecret,
    pub session_dir: PathBuf,
}
