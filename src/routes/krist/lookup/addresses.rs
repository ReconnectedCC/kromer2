use std::collections::HashMap;

use actix_web::{HttpResponse, get, web};

use crate::database::wallet::Model as Wallet;

use crate::models::krist::addresses::AddressJson;
use crate::models::krist::webserver::lookup::addresses::{LookupResponse, QueryParameters};
use crate::{AppState, errors::krist::KristError};

#[utoipa::path(
    get,
    path = "/api/krist/lookup/addresses/{addresses}",
    params(
        ("addresses", description = "Comma separated list of addresses"),
        QueryParameters
    ),
    responses(
        (status = 200, description = "Lookup Addresses", body = LookupResponse)
    )
)]
#[get("/{addresses}")]
async fn addresses_lookup(
    state: web::Data<AppState>,
    addresses: web::Path<String>,
    params: web::Query<QueryParameters>,
) -> Result<HttpResponse, KristError> {
    let pool = &state.pool;
    let addresses = addresses.into_inner();
    let params = params.into_inner();

    let addresses: Vec<&str> = addresses.split(',').collect();
    let address_count = addresses.len();

    let fetch_names = params.fetch_names.unwrap_or(false);

    let looked_up = Wallet::lookup_addresses(pool, addresses, fetch_names).await?;
    let json_models: Vec<AddressJson> = looked_up.into_iter().map(|model| model.into()).collect();
    let len = json_models.len();

    let hashmap: HashMap<String, AddressJson> = json_models
        .into_iter()
        .map(|model| (model.address.clone(), model))
        .collect();

    let response = LookupResponse {
        ok: true,
        found: len,
        not_found: address_count - len,
        addresses: hashmap,
    };

    Ok(HttpResponse::Ok().json(response))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(addresses_lookup);
}
