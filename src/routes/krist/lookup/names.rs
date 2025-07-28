use actix_web::{HttpResponse, get, web};

use crate::database::name::Model as Name;
use crate::database::transaction::Model as Transaction;
use crate::models::krist::names::NameJson;
use crate::models::krist::transactions::{TransactionJson, TransactionListResponse};
use crate::models::krist::webserver::lookup::names::{LookupResponse, QueryParameters};
use crate::{AppState, errors::krist::KristError};

#[get("/{addresses}")]
async fn names_lookup(
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

    let _address_count = addresses.len();

    // Convert query parameters
    let limit = params.limit.unwrap_or(50) as i64;
    let offset = params.offset.unwrap_or(0) as i64;
    let order_by = params.order_by.as_deref().unwrap_or("name");
    let order = params.order.as_deref().unwrap_or("ASC");

    // Lookup names
    let address_list = if addresses.is_empty() {
        None
    } else {
        Some(addresses)
    };

    let paginated_result = Name::lookup_names(pool, address_list, limit, offset, order_by, order)
        .await
        .map_err(|e| KristError::Database(e))?;

    // Convert to JSON format
    let json_models: Vec<NameJson> = paginated_result
        .rows
        .into_iter()
        .map(|model| model.into())
        .collect();

    let count = json_models.len();

    let response = LookupResponse {
        ok: true,
        count,
        total: paginated_result.total as usize,
        names: json_models,
    };

    Ok(HttpResponse::Ok().json(response))
}

#[get("/{name}/history")]
async fn name_history(
    state: web::Data<AppState>,
    name: web::Path<String>,
    params: web::Query<QueryParameters>,
) -> Result<HttpResponse, KristError> {
    let pool = &state.pool;
    let name = name.into_inner();
    let params = params.into_inner();

    let limit = params.limit.unwrap_or(50) as i64;
    let offset = params.offset.unwrap_or(0) as i64;

    let (transactions, total) = Transaction::fetch_name_history(pool, &name, limit, offset)
        .await
        .map_err(|e| KristError::Database(e))?;

    let json_transactions: Vec<TransactionJson> =
        transactions.into_iter().map(|model| model.into()).collect();

    let count = json_transactions.len();

    let response = TransactionListResponse {
        ok: true,
        count,
        total,
        transactions: json_transactions,
    };

    Ok(HttpResponse::Ok().json(response))
}

#[get("/{name}/transactions")]
async fn name_transactions(
    state: web::Data<AppState>,
    name: web::Path<String>,
    params: web::Query<QueryParameters>,
) -> Result<HttpResponse, KristError> {
    let pool = &state.pool;
    let name = name.into_inner();
    let params = params.into_inner();

    let limit = params.limit.unwrap_or(50) as i64;
    let offset = params.offset.unwrap_or(0) as i64;
    let order_by = params.order_by.as_deref().unwrap_or("id");
    let order = params.order.as_deref().unwrap_or("DESC");

    let (transactions, total) =
        Transaction::fetch_by_sent_name(pool, &name, limit, offset, order_by, order)
            .await
            .map_err(|e| KristError::Database(e))?;

    let json_transactions: Vec<TransactionJson> =
        transactions.into_iter().map(|model| model.into()).collect();

    let count = json_transactions.len();

    let response = TransactionListResponse {
        ok: true,
        count,
        total,
        transactions: json_transactions,
    };

    Ok(HttpResponse::Ok().json(response))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(names_lookup)
        .service(name_history)
        .service(name_transactions);
}
