use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::models::krist::{names::NameJson, transactions::TransactionJson};

/// All the names owned by the given address(es), or the whole network if no addresses are specified.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LookupResponse {
    pub ok: bool,
    pub count: usize,
    pub total: usize,
    pub names: Vec<NameJson>,
}

/// All the transactions directly involving the given name. This is any transaction with the type `name_purchase`, `name_a_record` or `name_transfer`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoryLookupResponse {
    pub ok: bool,
    pub found: usize,
    #[serde(rename = "notFound")]
    pub not_found: usize,
    pub transactions: HashMap<String, TransactionJson>,
}

/// All the transactions sent to the given name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransactionsLookupResponse {
    pub ok: bool,
    pub found: usize,
    #[serde(rename = "notFound")]
    pub not_found: usize,
    pub transactions: HashMap<String, TransactionJson>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryParameters {
    pub order_by: Option<String>,
    pub order: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}
