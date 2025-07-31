use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::models::krist::transactions::TransactionJson;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LookupResponse {
    pub ok: bool,
    pub found: usize,
    #[serde(rename = "notFound")]
    pub not_found: usize,
    pub transactions: HashMap<String, TransactionJson>,
}

// NOTE: We have one minor incompatibility within our database, kristweb expects there to be
//       a `time` field which we don't have, do we just replace this with `timestamp` when we
//       get the request?
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryParameters {
    pub order_by: Option<String>,
    pub order: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub include_mined: Option<String>,
}
