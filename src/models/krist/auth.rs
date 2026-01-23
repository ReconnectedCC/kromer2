use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize, ToSchema)]
pub struct LoginDetails {
    #[serde(rename = "privatekey")]
    pub private_key: String,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize, ToSchema)]
pub struct AddressAuthenticationResponse {
    pub ok: bool,
    pub authed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
}
