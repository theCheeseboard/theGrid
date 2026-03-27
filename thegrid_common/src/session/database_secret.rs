use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use gpui::private::anyhow;
use keyring::Credential;
use matrix_sdk::authentication::matrix::MatrixSession;
use matrix_sdk::authentication::oauth::{ClientId, OAuthSession, UserSession};
use matrix_sdk::SessionMeta;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct DatabaseSecret {
    database_password: String,
    matrix_session: SessionType,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum SessionType {
    None,
    LegacyMatrix(MatrixSession),
    OAuth(SerialisableOAuthSession),
}

impl DatabaseSecret {
    pub fn new() -> anyhow::Result<Self> {
        let mut secret_key = [0u8; 256];
        getrandom::fill(&mut secret_key)?;
        Ok(Self {
            database_password: BASE64_STANDARD.encode(secret_key),
            matrix_session: SessionType::None,
        })
    }

    pub fn database_password(&self) -> String {
        self.database_password.clone()
    }

    pub fn matrix_session(&self) -> Option<&MatrixSession> {
        if let SessionType::LegacyMatrix(session) = &self.matrix_session {
            Some(session)
        } else {
            None
        }
    }

    pub fn oauth_session(&self) -> Option<OAuthSession> {
        if let SessionType::OAuth(session) = &self.matrix_session {
            Some(session.clone().into())
        } else {
            None
        }
    }

    pub fn session_meta(&self) -> Option<&SessionMeta> {
        match &self.matrix_session {
            SessionType::None => None,
            SessionType::LegacyMatrix(session) => Some(&session.meta),
            SessionType::OAuth(session) => Some(&session.user.meta),
        }
    }

    pub fn set_matrix_session(&mut self, matrix_session: MatrixSession) {
        self.matrix_session = SessionType::LegacyMatrix(matrix_session);
    }

    pub fn set_oauth_session(&mut self, oauth_session: OAuthSession) {
        self.matrix_session = SessionType::OAuth(oauth_session.into());
    }

    pub fn set_session(&mut self, session: SessionType) {
        self.matrix_session = session;
    }
}

impl TryFrom<Vec<u8>> for DatabaseSecret {
    type Error = anyhow::Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        Ok(serde_json::from_slice(&value)?)
    }
}

pub trait DatabaseSecretExt {
    fn get_database_secret(&self) -> anyhow::Result<DatabaseSecret>;
}

impl DatabaseSecretExt for Box<Credential> {
    fn get_database_secret(&self) -> anyhow::Result<DatabaseSecret> {
        DatabaseSecret::try_from(self.get_secret()?)
    }
}

// HACK: This is not serialisable in the Matrix SDK for some reason.
#[derive(Clone, Serialize, Deserialize)]
pub struct SerialisableOAuthSession {
    /// The client ID obtained after registration.
    pub client_id: ClientId,

    /// The user session.
    pub user: UserSession,
}

impl From<OAuthSession> for SerialisableOAuthSession {
    fn from(value: OAuthSession) -> Self {
        Self {
            client_id: value.client_id,
            user: value.user,
        }
    }
}

impl From<SerialisableOAuthSession> for OAuthSession {
    fn from(value: SerialisableOAuthSession) -> Self {
        Self {
            client_id: value.client_id,
            user: value.user,
        }
    }
}
