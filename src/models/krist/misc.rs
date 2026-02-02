use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize, ToSchema)]
pub struct WalletVersionResponse {
    pub ok: bool,
    #[serde(rename = "walletVersion")]
    pub wallet_version: u8,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize, ToSchema)]
pub struct MoneySupplyResponse {
    pub ok: bool,
    #[schema(value_type = f64, example = 100000.00)]
    pub money_supply: Decimal,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize, ToSchema)]
pub struct PrivateKeyAddressResponse {
    pub ok: bool,
    pub address: String,
}
