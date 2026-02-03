use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Returned by kromer API on successful authorization.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AuthenticatedResponse {
    pub token: Uuid,
    pub expires: DateTime<Utc>,
    #[schema(example = "kabcdefghi")]
    pub address: String,
}
