use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize, ToSchema)]
pub struct BlockJson {
    pub height: f64,
    pub address: String,
    pub hash: Option<String>,
    pub short_hash: Option<String>,
    pub value: f64,
    pub time: String,
    pub difficulty: f64,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize, ToSchema)]
pub struct SubmitBlockResponse {
    pub address: super::addresses::AddressJson,
    pub block: BlockJson,
    pub work: f64,
}
