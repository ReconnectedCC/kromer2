use actix_web::{error, http::StatusCode};

#[derive(Debug, Clone, Copy, thiserror::Error)]
pub enum AuthError {
    #[error("Missing bearer auth token in header")]
    MissingBearer,
    #[error("This session is not authorized to operate on this resource")]
    Unauthorized,
    #[error("Attempted to operate on a wallet not associated with this session")]
    BadWallet,
    #[error("Failed to start session, please try again")]
    AuthFailed,
    #[error("The provided token either does not exist, or has expired")]
    InvalidSession,
}

impl error::ResponseError for AuthError {
    fn status_code(&self) -> StatusCode {
        StatusCode::UNAUTHORIZED
    }
}
