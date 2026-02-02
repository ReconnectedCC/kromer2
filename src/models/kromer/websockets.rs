use serde::Serialize;

/// Response containing the count of active sessions.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct SessionCountResponse {
    pub count: usize,
}
