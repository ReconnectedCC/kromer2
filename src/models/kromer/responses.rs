use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct ApiResponse<'a, T: Serialize + ToSchema> {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<ResponseMeta>,

    #[serde(borrow, default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ApiError<'a>>,

    #[serde(borrow, default, skip_serializing_if = "Option::is_none")]
    pub message: Option<&'a str>,
}

/// A struct with nothing, used as a default placeholder
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct None {}

#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct ResponseMeta {
    pub limit: i32,
    pub total: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct ApiError<'a> {
    pub code: &'a str,
    pub message: &'a str,
    pub details: &'a [ErrorDetail<'a>],
}

#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct ErrorDetail<'a> {
    pub field: &'a str,
    pub message: &'a str,
}

impl<'a, T: Serialize + ToSchema> Default for ApiResponse<'a, T> {
    fn default() -> Self {
        Self {
            data: None,
            meta: None,
            error: None,
            message: None,
        }
    }
}

/// A response from an endpoint that takes paginated parameters.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PaginatedResponse<T> {
    /// The numbe of entries returned in `items`.
    pub count: usize,
    /// The number of entries remaining after the final entry in `items`. If you passed additional
    /// filters to the endpoint, they are taken into account.
    pub remaining: usize,
    pub items: Vec<T>,
}
