//! All kromer wallet related models

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use utoipa::ToSchema;

use crate::database::wallet;

#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct Wallet {
    pub id: i32,
    pub address: String,
    #[schema(value_type = f64, example = 100.50)]
    pub balance: Decimal,
    pub created_at: DateTime<Utc>,
    pub locked: bool,
    #[schema(value_type = f64, example = 500.00)]
    pub total_in: Decimal,
    #[schema(value_type = f64, example = 200.00)]
    pub total_out: Decimal,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub names: Option<i64>,
}

impl From<wallet::Model> for Wallet {
    fn from(value: wallet::Model) -> Self {
        Self {
            id: value.id,
            address: value.address,
            balance: value.balance,
            created_at: value.created_at,
            locked: value.locked,
            total_in: value.total_in,
            total_out: value.total_out,
            names: value.names,
        }
    }
}
