use actix_web::{HttpResponse, get, web};

use crate::database::transaction::Model as Transaction;
use crate::models::krist::transactions::TransactionListResponse;
use crate::models::krist::webserver::lookup::transactions::QueryParameters;
use crate::{AppState, errors::krist::KristError};

#[get("/{addresses}")]
async fn transactions_lookup(
    state: web::Data<AppState>,
    addresses: web::Path<String>,
    params: web::Query<QueryParameters>,
) -> Result<HttpResponse, KristError> {
    let pool = &state.pool;
    let addresses = addresses.into_inner();
    let params = params.into_inner();

    // Parse comma-separated addresses
    let addresses: Vec<String> = addresses
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    // Convert query parameters
    let limit = params.limit.unwrap_or(50) as i64;
    let offset = params.offset.unwrap_or(0) as i64;
    let order_by = params.order_by.as_deref().unwrap_or("id");
    let order = params.order.as_deref().unwrap_or("DESC");

    // Check for includeMined parameter
    let include_mined = params.include_mined.is_some();

    // Lookup transactions
    let address_list = if addresses.is_empty() {
        None
    } else {
        Some(addresses)
    };

    let (transactions, total) = Transaction::lookup_transactions(
        pool,
        address_list,
        limit,
        offset,
        order_by,
        order,
        include_mined,
    )
    .await?;

    let count = transactions.len();

    let response = TransactionListResponse {
        ok: true,
        count,
        total,
        transactions,
    };

    Ok(HttpResponse::Ok().json(response))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(transactions_lookup);
}
