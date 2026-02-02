// use crate::database::models::wallet;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::database::wallet;

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize, ToSchema)]
pub struct AddressListResponse {
    pub ok: bool,
    pub count: usize,
    pub total: usize,
    pub addresses: Vec<AddressJson>,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize, ToSchema)]
pub struct AddressResponse {
    pub ok: bool,
    pub address: AddressJson,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize, ToSchema)]
pub struct AddressCreationResponse {
    #[serde(rename = "privatekey")] // Dima you ask too much of me
    pub private_key: String,
    pub address: String,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize, ToSchema)]
pub struct AddressJson {
    pub address: String,
    #[schema(value_type = f64, example = 10.0)]
    pub balance: Decimal,
    #[serde(rename = "totalin")]
    #[schema(value_type = f64, example = 10.0)]
    pub total_in: Decimal,
    #[serde(rename = "totalout")]
    #[schema(value_type = f64, example = 10.0)]
    pub total_out: Decimal,
    #[serde(rename = "firstseen")]
    pub first_seen: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub names: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize, ToSchema)]
pub struct VerifyResponse {
    pub authed: bool,
    pub address: String,
}

impl From<wallet::Model> for AddressJson {
    fn from(wallet: wallet::Model) -> Self {
        Self {
            address: wallet.address,
            balance: wallet.balance,
            total_in: wallet.total_in,
            total_out: wallet.total_out,
            first_seen: wallet.created_at.to_rfc3339(), // Is this really the right thing?
            names: wallet.names,
        }
    }
}

#[derive(
    Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize, ToSchema, IntoParams, Default,
)]
pub struct AddressGetQuery {
    #[serde(alias = "fetchNames")]
    pub fetch_names: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::dec;

    #[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
    struct ExpectedResponse {
        pub ok: bool,
        pub address: AddressJson,
    }

    #[test]
    fn test_deserialize() {
        let response = r#"{"ok":true,"address":{"address":"kre3w0i79j","balance":86945,"totalin":123364,"totalout":38292,"firstseen":"2015-03-13T12:55:18.000Z"}}"#;
        let response: ExpectedResponse =
            serde_json::from_str(response).expect("Failed to deserialize");
        assert!(response.ok);
    }

    #[test]
    fn test_serialize() {
        let response = ExpectedResponse {
            ok: true,
            address: AddressJson {
                address: "kre3w0i79j".to_owned(),
                balance: dec!(86945.0),
                total_in: dec!(123364.0),
                total_out: dec!(38292.0),
                first_seen: "2015-03-13T12:55:18.000Z".to_owned(),
                names: None,
            },
        };
        let response_str = serde_json::to_string(&response).expect("Failed to serialize");
        let response_str_test = r#"{"ok":true,"address":{"address":"kre3w0i79j","balance":86945.0,"totalin":123364.0,"totalout":38292.0,"firstseen":"2015-03-13T12:55:18.000Z"}}"#;

        assert_eq!(response_str, response_str_test);
    }
}
