// British people can fight me, I *will* be writing it as "canceled"

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::models::kromer::Patch;

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
    pub contract_id: i32,
    pub address: String,

    pub status: ContractStatus,

    pub title: String,
    pub description: Option<String>,

    pub price: Decimal,
    pub cron_expr: String,

    pub max_subscribers: Option<i32>,
    pub allow_list: Option<Vec<String>>,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
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

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct UpdateContractRequest {
    pub title: Option<String>,
    pub description: Patch<String>,
    pub status: Option<ContractStatus>,
    pub price: Option<Decimal>,
    pub cron_expr: Option<String>,
    pub max_subscribers: Patch<i32>,
    pub allow_list: Patch<Vec<String>>,
}

impl UpdateContractRequest {
    pub fn is_empty(&self) -> bool {
        self.title.is_none()
            && self.description.is_none()
            && self.price.is_none()
            && self.cron_expr.is_none()
            && self.max_subscribers.is_none()
            && self.allow_list.is_none()
    }
}
