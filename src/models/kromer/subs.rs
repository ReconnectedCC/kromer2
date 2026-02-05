// British people can fight me, I *will* be writing it as "canceled"

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, ToSchema)]
#[sqlx(type_name = "contract_status", rename_all = "lowercase")]
pub enum ContractStatus {
    Open,
    Closed,
    Canceled,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, ToSchema)]
#[sqlx(type_name = "subscription_status", rename_all = "lowercase")]
pub enum SubStatus {
    Active,
    Pending,
    Canceled,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ContractCreateRequest {
    pub title: String,
    pub description: Option<String>,
    pub price: Decimal,
    pub cron_expr: String,
    pub max_subscribers: Option<i32>,

    pub allow_list: Option<Vec<String>>,
}

/// Contract info returned from requests
#[derive(Debug, Clone, Serialize, sqlx::FromRow, ToSchema)]
pub struct ContractInfo {
    contract_id: i32,
    address: String,

    status: ContractStatus,

    title: String,
    description: Option<String>,

    price: Decimal,
    cron_expr: String,

    max_subscribers: Option<i32>,
    allow_list: Option<String>,

    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

/// Subscription info returned by the server
#[derive(Debug, Clone, Serialize, sqlx::FromRow, ToSchema)]
pub struct SubscriptionInfo {
    subscription_id: i32,
    address: String,
    status: SubStatus,

    /// The time the subscription will lapse at. Empty if no subscription is active
    lapsed_at: Option<DateTime<Utc>>,
    /// The time that the current term started at. If the subscription ends and then is restarted,
    /// this value will be reset.
    started_at: DateTime<Utc>,
}

// Yes, this is seperate from the main PaginationParams struct. This needs extra parameters, and is
// not bound by the ModelExt trait anyways so I can get away with it.

/// Pagination params passed to contract GET request. Filters eitheir subscriptions by their
#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema)]
pub struct ContractQueryParams {
    /// The maximum number of entries to return. Will be clamped between 0 and 500, defaulting to
    /// 50.
    pub limit: Option<i32>,
    /// The offset of the page, defaults to 0.
    pub offset: Option<i32>,
    /// Optional filter by address that owns the resource
    pub address: Option<String>,
    /// Optional filter based on the resource's status
    pub is_open: Option<bool>,
}

/// Pagination params used to list subscribers for a subscription.
#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct ListSubscribersParams {
    pub limit: Option<i32>,
    pub offset: Option<i32>,
    pub is_active: Option<bool>,
}
