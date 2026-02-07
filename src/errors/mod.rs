pub mod auth;
pub mod krist;
pub mod name;
pub mod player;
pub mod subs;
pub mod transaction;
pub mod wallet;
pub mod websocket;

use actix_web::{
    HttpResponse,
    body::BoxBody,
    error::{self, JsonPayloadError},
    http::StatusCode,
};

use crate::models::kromer::responses::{ApiError, ApiResponse, None};

#[derive(Debug, thiserror::Error)]
pub enum KromerError {
    #[error("Resource not found")]
    NotFound,

    #[error("Validation error: {0}")]
    Validation(String),

    #[error(transparent)]
    Database(#[from] sqlx::Error),

    #[error(transparent)]
    Wallet(#[from] wallet::WalletError),

    #[error(transparent)]
    Name(#[from] name::NameError),

    #[error(transparent)]
    Player(#[from] player::PlayerError),

    #[error("Transaction error: {0}")]
    Transaction(#[from] transaction::TransactionError),

    #[error("WebSocket error: {0}")]
    WebSocket(#[from] websocket::WebSocketError),

    #[error("Something went wrong: {0}")]
    Internal(&'static str),

    #[error(transparent)]
    IO(#[from] std::io::Error),

    #[error(transparent)]
    JsonPayload(#[from] JsonPayloadError),

    #[error("Authorization error: {0}")]
    AuthError(#[from] auth::AuthError),

    #[error("Subscriptions Error: {0}")]
    SubsError(#[from] subs::SubsError),
}

impl error::ResponseError for KromerError {
    fn status_code(&self) -> StatusCode {
        match self {
            KromerError::NotFound => StatusCode::NOT_FOUND,
            KromerError::Database(..) => StatusCode::INTERNAL_SERVER_ERROR,
            KromerError::Wallet(e) => e.status_code(),
            KromerError::Transaction(e) => e.status_code(),
            KromerError::Name(e) => e.status_code(),
            KromerError::Player(e) => e.status_code(),
            KromerError::Validation(_) => StatusCode::BAD_REQUEST,
            KromerError::WebSocket(_) => StatusCode::INTERNAL_SERVER_ERROR,
            KromerError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
            KromerError::IO(_) => StatusCode::INTERNAL_SERVER_ERROR,
            KromerError::JsonPayload(e) => e.status_code(),
            KromerError::AuthError(e) => e.status_code(),
            KromerError::SubsError(e) => e.status_code(),
        }
    }

    fn error_response(&self) -> HttpResponse<BoxBody> {
        let message = self.to_string();

        let error = ApiError {
            code: match self {
                KromerError::NotFound => "resource_not_found_error",
                KromerError::Database(..) => "database_error",
                KromerError::Wallet(..) => "wallet_error",
                KromerError::Transaction(..) => "transaction_error",
                KromerError::Player(..) => "player_error",
                KromerError::Validation(_) => "validation_error",
                KromerError::Name(_) => "name_error",
                KromerError::WebSocket(_) => "websocket_error",
                KromerError::Internal(_) => "internal_error",
                KromerError::IO(_) => "io_error",
                KromerError::JsonPayload(_) => "json_payload_error",
                KromerError::AuthError(_) => "authorization_error",
                KromerError::SubsError(_) => "subscription_error",
            },
            message: &message,
            details: &[],
        };

        let response: ApiResponse<'_, None> = ApiResponse {
            error: Some(error),
            ..Default::default()
        };

        HttpResponse::build(self.status_code()).json(response)
    }
}
