//! All kromer wallet related models

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;

use crate::database::wallet;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Wallet {
    pub id: i32,
    pub address: String,
    pub balance: Decimal,
    pub created_at: DateTime<Utc>,
    pub locked: bool,
    pub total_in: Decimal,
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
