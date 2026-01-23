use actix_web::{HttpResponse, get, post, web};
use rust_decimal::Decimal;

use crate::{
    AppState,
    database::wallet::Model as Wallet,
    errors::krist::KristError,
    models::krist::{
        auth::{AddressAuthenticationResponse, LoginDetails},
        misc::{MoneySupplyResponse, PrivateKeyAddressResponse, WalletVersionResponse},
        motd::{Constants, CurrencyInfo, DetailedMotd, DetailedMotdResponse, PackageInfo},
    },
    utils::crypto,
};

#[utoipa::path(
    post,
    path = "/api/krist/login",
    request_body = LoginDetails,
    responses(
        (status = 200, description = "Authenticate address", body = AddressAuthenticationResponse)
    )
)]
#[post("/login")]
async fn login_address(
    state: web::Data<AppState>,
    query: web::Json<LoginDetails>,
) -> Result<HttpResponse, KristError> {
    let db = &state.pool;
    let query = query.into_inner();

    let private_key = query.private_key;
    let result = Wallet::verify_address(db, private_key).await?;

    Ok(HttpResponse::Ok().json(AddressAuthenticationResponse {
        address: result.authed.then_some(result.model.address),
        authed: result.authed,
        ok: true,
    }))
}

#[utoipa::path(
    get,
    path = "/api/krist/motd",
    responses(
        (status = 200, description = "Get Message of the Day", body = DetailedMotdResponse)
    )
)]
#[get("/motd")]
async fn get_motd() -> HttpResponse {
    // This is by far the simplest fucking route in all of Kromer.
    // TODO: Make this actually better.
    let motd = DetailedMotd {
        server_time: "server_time".to_string(),
        motd: "Message of the day".to_string(),
        set: None,
        motd_set: None,
        public_url: "http://kromer.reconnected.cc".to_string(),
        public_ws_url: "http://kromer.reconnected.cc/api/krist/ws".to_string(),
        mining_enabled: false,
        transactions_enabled: true,
        debug_mode: true,
        work: 500,
        last_block: None,
        package: PackageInfo {
            name: "Kromer".to_string(),
            version: "0.2.0".to_string(),
            author: "ReconnectedCC Team".to_string(),
            license: "GPL-3.0".to_string(),
            repository: "https://github.com/ReconnectedCC/kromer/".to_string(),
            git_hash: crate::build_info::GIT_COMMIT_HASH.map(|s| s.to_string()),
        },
        constants: Constants {
            wallet_version: 3,
            nonce_max_size: 500,
            name_cost: 500,
            min_work: 50,
            max_work: 500,
            work_factor: 500.0,
            seconds_per_block: 5000,
        },
        currency: CurrencyInfo {
            address_prefix: "k".to_string(),
            name_suffix: "kro".to_string(),
            currency_name: "Kromer".to_string(),
            currency_symbol: "KRO".to_string(),
        },
        notice: "Some awesome notice will go here".to_string(),
    };

    let motd = DetailedMotdResponse { ok: true, motd };

    HttpResponse::Ok().json(motd)
}

#[utoipa::path(
    get,
    path = "/api/krist/walletversion",
    responses(
        (status = 200, description = "Get Wallet Version", body = WalletVersionResponse)
    )
)]
#[get("/walletversion")]
async fn get_walletversion() -> HttpResponse {
    let response = WalletVersionResponse {
        ok: true,
        wallet_version: 3,
    };

    HttpResponse::Ok().json(response)
}

#[utoipa::path(
    post,
    path = "/api/krist/v2",
    request_body = LoginDetails,
    responses(
        (status = 200, description = "Get V2 Address", body = PrivateKeyAddressResponse)
    )
)]
#[post("/v2")]
async fn get_v2_address(query: web::Json<LoginDetails>) -> Result<HttpResponse, KristError> {
    let query = query.into_inner();
    let key = query.private_key;

    let address = crypto::make_v2_address(&key, "k");
    let response = PrivateKeyAddressResponse { address, ok: true };

    Ok(HttpResponse::Ok().json(response))
}

#[utoipa::path(
    get,
    path = "/api/krist/supply",
    responses(
        (status = 200, description = "Get Money Supply", body = MoneySupplyResponse)
    )
)]
#[get("/supply")]
async fn get_kromer_supply(state: web::Data<AppState>) -> Result<HttpResponse, KristError> {
    let pool = &state.pool;

    let money_supply: Decimal = sqlx::query_scalar(
        "SELECT COALESCE(SUM(balance), 0) FROM wallets WHERE address != 'serverwelf'",
    )
    .fetch_one(pool)
    .await?;

    Ok(HttpResponse::Ok().json(MoneySupplyResponse {
        ok: true,
        money_supply,
    }))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("")
            .service(login_address)
            .service(get_motd)
            .service(get_kromer_supply)
            .service(get_v2_address),
    );
}
