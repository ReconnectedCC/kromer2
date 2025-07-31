use actix_web::{HttpResponse, get, web};

use crate::AppState;

use crate::database::{ModelExt, name::Model as Name, wallet::Model as Wallet};
use crate::errors::krist::KristError;
use crate::errors::krist::address::AddressError;
use crate::models::krist::addresses::{
    AddressGetQuery, AddressJson, AddressListResponse, AddressResponse,
};
use crate::models::krist::names::{NameJson, NameListResponse};
use crate::models::krist::transactions::{
    AddressTransactionQuery, TransactionJson, TransactionListResponse,
};
use crate::routes::PaginationParams;

#[get("")]
async fn wallet_list(
    state: web::Data<AppState>,
    pagination: web::Query<PaginationParams>,
) -> Result<HttpResponse, KristError> {
    let pool = &state.pool;

    let pagination = pagination.into_inner();
    let limit = pagination.limit.unwrap_or(50);
    let offset = pagination.offset.unwrap_or(0);

    let mut tx = pool.begin().await?;

    let total = Wallet::total_count(&mut *tx).await?;
    let wallets = Wallet::fetch_all(&mut *tx, limit, offset).await?;

    tx.commit().await?;

    let addresses: Vec<AddressJson> = wallets.into_iter().map(|wallet| wallet.into()).collect();

    let list_response = AddressListResponse {
        ok: true,
        count: addresses.len(),
        total,
        addresses,
    };

    Ok(HttpResponse::Ok().json(list_response))
}

#[get("/{address}")]
async fn wallet_get(
    state: web::Data<AppState>,
    address: web::Path<String>,
    query: web::Query<AddressGetQuery>,
) -> Result<HttpResponse, KristError> {
    let address = address.into_inner();

    let wallet = match query.0.fetch_names {
        Some(true) => Wallet::fetch_by_address_names(&state.pool, &address).await?,
        _ => Wallet::fetch_by_address(&state.pool, &address).await?,
    };

    wallet
        .map(|addr| AddressResponse {
            ok: true,
            address: addr.into(),
        })
        .map(|response| HttpResponse::Ok().json(response))
        .ok_or_else(|| KristError::Address(AddressError::NotFound(address)))
}

#[get("/rich")]
async fn wallet_richest(
    state: web::Data<AppState>,
    pagination: web::Query<PaginationParams>,
) -> Result<HttpResponse, KristError> {
    let pagination = pagination.into_inner();
    let limit = pagination.limit.unwrap_or(50);
    let offset = pagination.offset.unwrap_or(0);

    let total = Wallet::total_count(&state.pool).await?;
    let ordered_wallets = Wallet::fetch_richest(&state.pool, limit, offset).await?;
    let addresses: Vec<_> = ordered_wallets
        .into_iter()
        .map(|wallet| wallet.into())
        .collect();

    let response = AddressListResponse {
        ok: true,
        count: addresses.len(),
        total,
        addresses: addresses,
    };

    Ok(HttpResponse::Ok().json(response))
}

#[get("/{address}/transactions")]
async fn wallet_get_transactions(
    state: web::Data<AppState>,
    address: web::Path<String>,
    params: web::Query<AddressTransactionQuery>,
) -> Result<HttpResponse, KristError> {
    let address = address.into_inner();
    let params = params.into_inner();
    let pool = &state.pool;

    let mut tx = pool.begin().await?;

    let wallet = Wallet::fetch_by_address(&mut *tx, &address)
        .await?
        .ok_or_else(|| KristError::Address(AddressError::NotFound(address)))?;

    let total_transactions = wallet.total_transactions(&mut *tx).await?;
    let transactions = wallet.transactions(&mut *tx, &params).await?;

    tx.commit().await?;

    let transactions: Vec<TransactionJson> =
        transactions.into_iter().map(|trans| trans.into()).collect();

    let response = TransactionListResponse {
        ok: true,
        count: transactions.len(),
        total: total_transactions as usize,
        transactions,
    };

    Ok(HttpResponse::Ok().json(response))
}

#[get("/{address}/names")]
async fn wallet_get_names(
    state: web::Data<AppState>,
    address: web::Path<String>,
    query: web::Query<PaginationParams>,
) -> Result<HttpResponse, KristError> {
    let address = address.into_inner();
    let query = query.into_inner();
    let pool = &state.pool;

    let mut tx = pool.begin().await?;

    let wallet = Wallet::fetch_by_address(&mut *tx, &address)
        .await
        .map_err(|err| KristError::from(err))?
        .ok_or_else(|| KristError::Address(AddressError::NotFound(address)))?;

    let total_names = Name::total_count(&mut *tx).await?;
    let names = wallet.names(&mut *tx, &query).await?;

    tx.commit().await?;

    let names: Vec<NameJson> = names.into_iter().map(|trans| trans.into()).collect();
    let response = NameListResponse {
        ok: true,
        count: names.len(),
        total: total_names,
        names,
    };

    Ok(HttpResponse::Ok().json(response))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/addresses")
            .service(wallet_richest)
            .service(wallet_get)
            .service(wallet_get_transactions)
            .service(wallet_get_names)
            .service(wallet_list),
    );
}
