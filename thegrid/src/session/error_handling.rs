use cntp_i18n::{I18nString, tr};
use matrix_sdk::ruma::api::client::error::{ErrorBody, ErrorKind};
use matrix_sdk::ruma::api::error::FromHttpResponseError;
use matrix_sdk::{HttpError, RumaApiError};

#[derive(Clone, Copy)]
pub enum ClientError {
    None,
    Terminal(TerminalClientError),
    Recoverable(RecoverableClientError),
}

#[derive(Clone, Copy)]
pub enum TerminalClientError {
    UnknownToken,
    UnknownError,
}

#[derive(Clone, Copy)]
pub enum RecoverableClientError {}

pub fn handle_error(error: &matrix_sdk::Error) -> ClientError {
    match &error {
        matrix_sdk::Error::Http(http_error) => match http_error.as_ref() {
            HttpError::Api(api_error) => match api_error.as_ref() {
                FromHttpResponseError::Server(RumaApiError::ClientApi(client_api_error)) => {
                    handle_client_api_error(client_api_error)
                }
                _ => ClientError::Terminal(TerminalClientError::UnknownError),
            },
            _ => ClientError::Terminal(TerminalClientError::UnknownError),
        },
        _ => ClientError::Terminal(TerminalClientError::UnknownError),
    }
}

fn handle_client_api_error(error: &matrix_sdk::ruma::api::client::Error) -> ClientError {
    match &error.body {
        ErrorBody::Standard { kind, message } => match kind {
            ErrorKind::UnknownToken { soft_logout } => {
                ClientError::Terminal(TerminalClientError::UnknownToken)
            }
            _ => ClientError::Terminal(TerminalClientError::UnknownError),
        },
        ErrorBody::Json(_) => ClientError::Terminal(TerminalClientError::UnknownError),
        ErrorBody::NotJson { .. } => ClientError::Terminal(TerminalClientError::UnknownError),
    }
}

impl TerminalClientError {
    pub fn description(&self) -> I18nString {
        match self {
            TerminalClientError::UnknownToken => tr!(
                "TERMINAL_ERROR_UNKNOWN_TOKEN",
                "This session was logged out by another device."
            ),
            TerminalClientError::UnknownError => {
                tr!("TERMINAL_ERROR_UNKNOWN_ERROR", "An unknown error occurred.")
            }
        }
    }

    pub fn should_logout(&self) -> bool {
        match self {
            TerminalClientError::UnknownToken => true,
            TerminalClientError::UnknownError => false,
        }
    }
}
