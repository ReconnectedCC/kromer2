//! Responses and error types for the krist api routes
pub mod address;
pub mod generic;
pub mod name;
pub mod transaction;
pub mod websockets;

use actix_web::{
    HttpResponse,
    error::{self, JsonPayloadError, PathError},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct KristErrorResponse {
    pub ok: bool,
    pub error: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub info: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum KristError {
    #[error(transparent)]
    Address(#[from] address::AddressError),

    #[error(transparent)]
    Generic(#[from] generic::GenericError),

    #[error(transparent)]
    Name(#[from] name::NameError),

    #[error(transparent)]
    Transaction(#[from] transaction::TransactionError),

    #[error(transparent)]
    WebSocket(#[from] websockets::WebSocketError),

    #[error(transparent)]
    Database(#[from] sqlx::Error), // Do we really want to expose all of this to the client?

    #[error(transparent)]
    JsonPayload(#[from] JsonPayloadError),

    #[error(transparent)]
    Path(#[from] PathError),

    #[error("{0}")]
    Custom(&'static str),
}

pub trait KristErrorExt {
    /// Get the error type for the `message` field in a krist error response
    fn error_type(&self) -> &'static str;
}

impl KristErrorExt for KristError {
    fn error_type(&self) -> &'static str {
        match self {
            KristError::Address(e) => e.error_type(),
            KristError::Generic(e) => e.error_type(),
            KristError::Name(e) => e.error_type(),
            KristError::Transaction(e) => e.error_type(),
            KristError::WebSocket(e) => e.error_type(),
            KristError::Database(_) => "internal_server_error",
            KristError::JsonPayload(_) => "payload_error",
            KristError::Path(_) => "path_error",
            KristError::Custom(e) => e, // Same way as krist, where message is the error type when no message type is given
        }
    }
}

impl error::ResponseError for KristError {
    fn status_code(&self) -> StatusCode {
        // TODO: Evaluate whether or not programs might break when returning the correct error code or not.
        //       In Krist, responses are always error code 200 because of a long standing bug.
        //       For some reason, that bug was never fixed and is just set there for forever, pretty stupid if you ask me.
        match self {
            KristError::Address(e) => e.status_code(),
            KristError::Generic(e) => e.status_code(),
            KristError::Name(e) => e.status_code(),
            KristError::Transaction(e) => e.status_code(),
            KristError::WebSocket(e) => e.status_code(),
            KristError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
            KristError::JsonPayload(_) => StatusCode::BAD_REQUEST,
            KristError::Path(_) => StatusCode::BAD_REQUEST,
            KristError::Custom(_) => StatusCode::BAD_REQUEST,
        }
    }

    fn error_response(&self) -> HttpResponse<actix_web::body::BoxBody> {
        match self {
            KristError::Address(e) => e.error_response(),
            KristError::Generic(e) => e.error_response(),
            KristError::Name(e) => e.error_response(),
            KristError::Transaction(e) => e.error_response(),
            KristError::WebSocket(e) => e.error_response(),
            KristError::Custom(e) => {
                let error = KristErrorResponse {
                    ok: false,
                    error: e,
                    message: e.to_string(),
                    info: None,
                };

                HttpResponse::build(StatusCode::BAD_REQUEST).json(error)
            }
            _ => {
                let error = KristErrorResponse {
                    ok: false,
                    error: self.error_type(),
                    message: self.to_string(),
                    info: None,
                };

                HttpResponse::build(self.status_code()).json(error)
            }
        }
    }
}
