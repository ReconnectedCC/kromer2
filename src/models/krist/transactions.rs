use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::database::transaction::{self, TransactionType};
// use transaction::TransactionNameData;

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize, ToSchema)]
pub struct TransactionListResponse {
    pub ok: bool,

    /// The count of results.
    pub count: usize,

    /// The total amount of transactions
    pub total: usize,

    pub transactions: Vec<TransactionJson>,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize, ToSchema)]
pub struct TransactionDetails {
    #[serde(rename = "privatekey")]
    pub private_key: String,
    pub to: String,
    #[schema(value_type = String, example = "10.00")]
    pub amount: Decimal,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<String>,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize, ToSchema)]
pub struct TransactionResponse {
    pub ok: bool,
    pub transaction: TransactionJson,
}

#[derive(Debug, Default, Clone, PartialEq, PartialOrd, Serialize, Deserialize, ToSchema, IntoParams)]
pub struct AddressTransactionQuery {
    pub limit: Option<i32>,
    pub offset: Option<i32>,
    #[serde(rename = "includeMined")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_mined: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize, ToSchema)]
pub struct TransactionJson {
    /// The ID of this transaction.
    pub id: i32,

    /// The sender of this transaction.
    pub from: Option<String>,

    /// The recipient of this transaction. This may be `name` if the transaction was a name purchase, or `a` if it was a name's data change.
    pub to: String,

    /// The amount of Krist transferred in this transaction. Can be 0, notably if the transaction was a name's data change.
    #[schema(value_type = String, example = "100.00")]
    pub value: Decimal,

    /// The time this transaction this was made, as an ISO-8601 string.
    pub time: String,

    /// The name associated with this transaction, without the `.kro` suffix.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sent_metaname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sent_name: Option<String>,
    #[serde(rename = "type")]
    pub transaction_type: TransactionType,
}

impl From<transaction::Model> for TransactionJson {
    fn from(transaction: transaction::Model) -> Self {
        // let name_data = TransactionNameData::parse_opt_ref(&transaction.metadata);

        Self {
            id: transaction.id,
            from: transaction.from,
            to: transaction.to,
            value: transaction.amount,
            time: transaction.date.to_rfc3339(),
            metadata: transaction.metadata,
            sent_metaname: transaction.sent_metaname,
            sent_name: transaction.sent_name,
            transaction_type: transaction.transaction_type,
            name: transaction.name,
        }
    }
}
