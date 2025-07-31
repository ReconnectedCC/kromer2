use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};

use crate::database::name;
// use utoipa::ToResponse;

// use crate::database::models::name;

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct NameListResponse {
    pub ok: bool,
    /// The count of results.
    pub count: usize,
    /// The total amount of transactions
    pub total: usize,
    pub names: Vec<NameJson>,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct NameResponse {
    pub ok: bool,
    pub name: NameJson,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct NameCostResponse {
    pub ok: bool,
    pub name_cost: i64,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct DetailedUnpaidResponseRow {
    pub count: i64,
    pub unpaid: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NameAvailablityResponse {
    pub ok: bool,
    pub available: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NameBonusResponse {
    pub ok: bool,
    pub name_bonus: i64,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct RegisterNameRequest {
    //#[serde(rename = "desiredName")]
    //pub desired_name: String,
    #[serde(rename = "privatekey")]
    pub private_key: String,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct TransferNameRequest {
    pub address: String,
    #[serde(rename = "privatekey")]
    pub private_key: String,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct NameDataUpdateBody {
    /// The data you want to set for the name.
    /// You may pass an empty string (`""`), `null` (in JSON requests), or omit the a parameter entirely to remove the data.
    pub a: Option<String>,
    #[serde(rename = "privatekey")]
    pub private_key: String,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct NameJson {
    pub name: String,
    pub owner: String,
    pub registered: String,
    pub updated: Option<String>,
    pub transferred: Option<String>,
    pub a: Option<String>,
    pub unpaid: i64,
}

impl From<name::Model> for NameJson {
    fn from(name: name::Model) -> Self {
        Self {
            name: name.name,
            owner: name.owner,
            registered: name.time_registered.to_rfc3339(),
            updated: name.last_updated.map(|dt| dt.to_rfc3339()),
            transferred: name.last_transfered.map(|dt| dt.to_rfc3339()),
            a: name.metadata,
            unpaid: name.unpaid.to_i64().unwrap_or(0),
        }
    }
}
