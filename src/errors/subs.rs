#[derive(Debug, thiserror::Error)]
pub enum SubsError {
    #[error("Got bad cron expression. See https://docs.gitlab.com/topics/cron/")]
    InvalidCronExpr,

    #[error("Got bad address in allow list at position {0}")]
    InvalidAllowList(usize),

    #[error("Contract price must be positive")]
    InvalidPrice,

    #[error("Max subscriber count must be non-negative")]
    InvalidMaxSubscribers,

    #[error("'{0} is not a valid ID")]
    InvalidId(i32),

    #[error("Could not find contract with ID '{0}'")]
    ContractNotFound(i32),
}

use actix_web::{error, http::StatusCode};

impl error::ResponseError for SubsError {
    fn status_code(&self) -> actix_web::http::StatusCode {
        match self {
            Self::InvalidCronExpr => StatusCode::BAD_REQUEST,
            Self::InvalidAllowList(_) => StatusCode::BAD_REQUEST,
            Self::InvalidPrice => StatusCode::BAD_REQUEST,
            Self::InvalidMaxSubscribers => StatusCode::BAD_REQUEST,
            Self::InvalidId(_) => StatusCode::BAD_REQUEST,
            Self::ContractNotFound(_) => StatusCode::NOT_FOUND,
        }
    }
}
