use actix_web::{HttpResponse, get, post, web};
use rust_decimal::Decimal;

use crate::database::ModelExt;
use crate::database::name::Model as Name;
use crate::database::transaction::{Model as Transaction, TransactionCreateData, TransactionType};
use crate::database::wallet::Model as Wallet;

use crate::errors::krist::address::AddressError;
use crate::errors::krist::generic::GenericError;
use crate::errors::krist::name::NameError;
use crate::errors::krist::transaction::TransactionError;
use crate::models::krist::motd::MINING_CONSTANTS;
use crate::models::krist::names::{
    NameAvailablityResponse, NameBonusResponse, NameCostResponse, NameDataUpdateBody, NameJson,
    NameListResponse, NameResponse, RegisterNameRequest, TransferNameRequest,
};
use crate::models::krist::websockets::{WebSocketEvent, WebSocketMessage};
use crate::utils::validation;
use crate::websockets::WebSocketServer;
use crate::{AppState, errors::krist::KristError, routes::PaginationParams};

#[get("")]
async fn name_list(
    state: web::Data<AppState>,
    query: web::Query<PaginationParams>,
) -> Result<HttpResponse, KristError> {
    let params = query.into_inner();
    let pool = &state.pool;

    let limit = params.limit.unwrap_or(50);
    let offset = params.offset.unwrap_or(0);

    let mut tx = pool.begin().await?;

    let total = Name::total_count(&mut *tx).await?;
    let names = Name::fetch_all(&mut *tx, limit, offset).await?;

    tx.commit().await?;

    let names: Vec<NameJson> = names.into_iter().map(|name| name.into()).collect();

    let response = NameListResponse {
        ok: true,
        count: names.len(),
        total,
        names,
    };

    Ok(HttpResponse::Ok().json(response))
}

#[get("/cost")]
async fn name_cost() -> Result<HttpResponse, KristError> {
    let response = NameCostResponse {
        ok: true,
        name_cost: MINING_CONSTANTS.name_cost,
    };
    Ok(HttpResponse::Ok().json(response))
}

#[get("/check/{name}")]
async fn name_check(
    state: web::Data<AppState>,
    name: web::Path<String>,
) -> Result<HttpResponse, KristError> {
    let name = name.into_inner();
    let pool = &state.pool;

    if !validation::is_valid_name(&name, true) {
        return Err(KristError::Generic(GenericError::InvalidParameter(
            "name".to_string(),
        )));
    }
    let name = name.trim().to_lowercase();

    let name = Name::fetch_by_name(pool, name).await?;

    let response = NameAvailablityResponse {
        ok: true,
        available: name.is_none(),
    };

    Ok(HttpResponse::Ok().json(response))
}

#[get("/bonus")]
async fn name_bonus(state: web::Data<AppState>) -> Result<HttpResponse, KristError> {
    let pool = &state.pool;
    let name_bonus = Name::count_unpaid(pool).await?;

    let response = NameBonusResponse {
        ok: true,
        name_bonus,
    };

    Ok(HttpResponse::Ok().json(response))
}

#[get("/new")]
async fn name_new(
    state: web::Data<AppState>,
    query: web::Query<PaginationParams>,
) -> Result<HttpResponse, KristError> {
    let params = query.into_inner();
    let pool = &state.pool;

    let mut tx = pool.begin().await?;

    let total = Name::total_count(&mut *tx).await?;
    let names = Name::all_unpaid(&mut *tx, &params).await?;

    tx.commit().await?;

    let names: Vec<NameJson> = names.into_iter().map(|name| name.into()).collect();

    let response = NameListResponse {
        ok: true,
        count: names.len(),
        total,
        names,
    };

    Ok(HttpResponse::Ok().json(response))
}

#[get("/{name}")]
async fn name_get(
    state: web::Data<AppState>,
    name: web::Path<String>,
) -> Result<HttpResponse, KristError> {
    let pool = &state.pool;
    let name = name.into_inner();

    let db_name = Name::fetch_by_name(pool, &name).await?;

    db_name
        .map(|name| NameResponse {
            ok: true,
            name: name.into(),
        })
        .map(|response| HttpResponse::Ok().json(response))
        .ok_or_else(|| KristError::Name(NameError::NameNotFound(name)))
}

#[post("/{name}")]
async fn name_register(
    state: web::Data<AppState>,
    websocket_server: web::Data<WebSocketServer>,
    name: web::Path<String>,
    details: Option<web::Json<RegisterNameRequest>>,
) -> Result<HttpResponse, KristError> {
    let pool = &state.pool;
    let websocket_server = websocket_server.into_inner();

    let name = name.into_inner().trim().to_lowercase();
    let new_name_cost = Decimal::new(MINING_CONSTANTS.name_cost, 0);

    let private_key = details.map(|request| request.0.private_key);
    let private_key = match private_key {
        Some(key) => key,
        None => {
            return Err(KristError::Generic(GenericError::MissingParameter(
                "privatekey".to_string(),
            )));
        }
    };

    if !validation::is_valid_name(&name, false) {
        return Err(KristError::Generic(GenericError::InvalidParameter(
            "name".to_string(),
        )));
    }

    let mut tx = pool.begin().await?;

    if let Some(name) = Name::fetch_by_name(&mut *tx, &name).await? {
        return Err(KristError::Name(NameError::NameTaken(name.name)));
    }

    let verify_addr_resp = Wallet::verify_address(&mut *tx, &private_key).await?;

    if !verify_addr_resp.authed {
        tracing::info!(
            "Name registration REJECTED for {}",
            verify_addr_resp.model.address
        );
        return Err(KristError::Address(AddressError::AuthFailed));
    }

    // Reject insufficient funds
    if verify_addr_resp.model.balance < new_name_cost {
        return Err(KristError::Transaction(TransactionError::InsufficientFunds));
    }

    // Update the wallet balance first (deduct the name cost)
    let updated_wallet = verify_addr_resp
        .model
        .update_balance(pool, -new_name_cost)
        .await?;

    // Create the transaction
    let creation_data = TransactionCreateData {
        from: verify_addr_resp.model.address.clone(),
        to: "serverwelf".to_string(),
        name: Some(name.clone()),
        amount: new_name_cost,
        transaction_type: TransactionType::NamePurchase,
        ..Default::default()
    };

    let transaction = Transaction::create_no_update(&mut *tx, creation_data).await?;
    tracing::info!(
        "Created transaction for name purchase with ID {}",
        transaction.id
    );

    let event = WebSocketMessage::new_event(WebSocketEvent::Transaction {
        transaction: transaction.into(),
    });
    websocket_server.broadcast_event(event).await;

    // Create the new name
    let name = Name::create(&mut *tx, name.clone(), verify_addr_resp.model.address).await?;
    let response = NameResponse {
        ok: true,
        name: name.into(),
    };

    tx.commit().await?;

    Ok(HttpResponse::Ok().json(response))
}

async fn name_update_data(
    state: web::Data<AppState>,
    name: web::Path<String>,
    body: web::Json<NameDataUpdateBody>,
) -> Result<HttpResponse, KristError> {
    let pool = &state.pool;
    let name = name.into_inner();
    let body = body.into_inner();

    let model = Name::ctrl_update_metadata(pool, name, body).await?;

    let name: NameJson = model.into();
    let resp = NameResponse { ok: true, name };

    Ok(HttpResponse::Ok().json(resp))
}

#[post("/{name}/transfer")]
async fn name_transfer(
    state: web::Data<AppState>,
    websocket_server: web::Data<WebSocketServer>,
    name: web::Path<String>,
    details: web::Json<TransferNameRequest>,
) -> Result<HttpResponse, KristError> {
    let pool = &state.pool;
    let server = websocket_server.into_inner();
    let details = details.into_inner();
    let name = name.into_inner();

    if !validation::is_valid_name(&name, false) {
        return Err(KristError::Generic(GenericError::InvalidParameter(
            "name".to_owned(),
        )));
    }

    let name = name.trim().to_lowercase();

    let current_owner_response = Wallet::verify_address(pool, details.private_key).await?;
    if !current_owner_response.authed {
        return Err(KristError::Address(AddressError::AuthFailed));
    }

    let name = Name::fetch_by_name(pool, &name)
        .await?
        .ok_or_else(|| KristError::Name(NameError::NameNotFound(name)))?;
    if name.owner == details.address {
        tracing::debug!("Disallowed bumping name, returning original data");
        let response = NameResponse {
            ok: true,
            name: name.into(),
        };

        return Ok(HttpResponse::Ok().json(response));
    }

    let updated_name = name
        .transfer_ownership(pool, &server, details.address)
        .await?;

    let response = NameResponse {
        ok: true,
        name: updated_name.into(),
    };

    return Ok(HttpResponse::Ok().json(response));
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/names")
            .service(name_list)
            .service(name_cost)
            .service(name_check)
            .service(name_bonus)
            .service(name_new)
            .service(name_get)
            .service(name_register)
            .service(name_transfer)
            .service(
                web::resource("/{name}/update")
                    .put(name_update_data)
                    .post(name_update_data),
            ),
    );
}
