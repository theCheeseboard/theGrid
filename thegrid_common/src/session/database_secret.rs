use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use gpui::private::anyhow;
use keyring::Credential;
use matrix_sdk::authentication::matrix::MatrixSession;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct DatabaseSecret {
    database_password: String,
    matrix_session: Option<MatrixSession>,
}

impl DatabaseSecret {
    pub fn new() -> anyhow::Result<Self> {
        let mut secret_key = [0u8; 256];
        getrandom::fill(&mut secret_key)?;
        Ok(Self {
            database_password: BASE64_STANDARD.encode(secret_key),
            matrix_session: None,
        })
    }

    pub fn database_password(&self) -> String {
        self.database_password.clone()
    }

    pub fn matrix_session(&self) -> Option<&MatrixSession> {
        self.matrix_session.as_ref()
    }

    pub fn set_matrix_session(&mut self, matrix_session: MatrixSession) {
        self.matrix_session = Some(matrix_session);
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
