use serde::{Deserialize, Serialize};

pub const MINING_CONSTANTS: Constants = Constants {
    wallet_version: 16,
    nonce_max_size: 24,
    name_cost: 500,
    min_work: 1,
    max_work: 100000,
    work_factor: 0.025,
    seconds_per_block: 300,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DetailedMotdResponse {
    pub ok: bool,
    #[serde(flatten)]
    pub motd: DetailedMotd,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Motd {
    pub motd: String,
    pub motd_set: String,
    pub debug_mode: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DetailedMotd {
    pub server_time: String,
    pub motd: String,
    pub set: Option<String>, // Support for backwards compatibility
    pub motd_set: Option<String>,

    pub public_url: String,
    pub public_ws_url: String,
    pub mining_enabled: bool,
    pub transactions_enabled: bool,
    pub debug_mode: bool,

    pub work: i64,
    pub last_block: Option<super::blocks::BlockJson>,
    pub package: PackageInfo,
    pub constants: Constants,
    pub currency: CurrencyInfo,

    pub notice: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PackageInfo {
    pub name: String,
    pub version: String,
    pub author: String,
    #[serde(rename = "licence")] // Fuck off, Krist
    pub license: String,
    pub repository: String,
    pub git_hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Constants {
    pub wallet_version: i64,
    pub nonce_max_size: i64,
    pub name_cost: i64,
    pub min_work: i64,
    pub max_work: i64,
    pub work_factor: f64,
    pub seconds_per_block: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CurrencyInfo {
    pub address_prefix: String,
    pub name_suffix: String,
    pub currency_name: String,
    pub currency_symbol: String,
}

#[inline]
pub fn get_currency_info() -> CurrencyInfo {
    CurrencyInfo {
        address_prefix: "k".to_string(),
        name_suffix: "kro".to_string(),
        currency_name: "Kromer".to_string(),
        currency_symbol: "KRO".to_string(),
    }
}
