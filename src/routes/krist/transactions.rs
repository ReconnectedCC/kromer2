use actix_web::{HttpResponse, get, post, web};
use rust_decimal::dec;

use crate::database::ModelExt;
use crate::database::transaction::{
    Model as Transaction, TransactionCreateData, TransactionNameData, TransactionType,
};
use crate::database::wallet::Model as Wallet;

use crate::database::name::Model as Name;
use crate::errors::krist::address::AddressError;
use crate::errors::krist::generic::GenericError;
use crate::errors::krist::name::NameError;
use crate::errors::krist::transaction::TransactionError;
use crate::models::krist::transactions::{
    TransactionDetails, TransactionJson, TransactionListResponse, TransactionResponse,
};
use crate::models::krist::websockets::{WebSocketEvent, WebSocketMessage};
use crate::utils::validation::NAME_META_RE;

use crate::websockets::WebSocketServer;
use crate::{AppState, errors::krist::KristError, routes::PaginationParams};

#[utoipa::path(
    get,
    path = "/api/krist/transactions",
    params(PaginationParams),
    responses(
        (status = 200, description = "List transactions", body = TransactionListResponse)
    )
)]
#[get("")]
pub async fn transaction_list(
    state: web::Data<AppState>,
    query: web::Query<PaginationParams>,
) -> Result<HttpResponse, KristError> {
    let params = query.into_inner();
    let pool = &state.pool;

    let mut tx = pool.begin().await?;

    let total_transaction = Transaction::total_count_no_mined(&mut *tx, &params).await?;
    let transactions = Transaction::fetch_all_no_mined(&mut *tx, &params).await?;

    tx.commit().await?;

    let transactions: Vec<TransactionJson> =
        transactions.into_iter().map(|trans| trans.into()).collect();

    let response = TransactionListResponse {
        ok: true,
        count: transactions.len(),
        total: total_transaction,
        transactions,
    };

    Ok(HttpResponse::Ok().json(response))
}

#[utoipa::path(
    post,
    path = "/api/krist/transactions",
    request_body = TransactionDetails,
    responses(
        (status = 200, description = "Create transaction", body = TransactionResponse)
    )
)]
#[post("")]
async fn transaction_create(
    state: web::Data<AppState>,
    server: web::Data<WebSocketServer>,
    details: web::Json<TransactionDetails>,
) -> Result<HttpResponse, KristError> {
    let pool = &state.pool;
    let details = details.into_inner();
    let amount = details.amount.round_dp(2); // Do not allow more than 2 decimals after the dot.

    // Check if the `to` field is not empty and must be below or equal to 64.
    // The length check is for making sure there is enough space for metaname too.
    if details.to.is_empty() && details.to.len() <= 64 {
        return Err(KristError::Generic(GenericError::InvalidParameter(
            "to".to_string(),
        )));
    }

    // Check on the server so DB doesnt throw.
    if amount <= dec!(0.00) {
        return Err(KristError::Generic(GenericError::InvalidParameter(
            "amount".to_string(),
        )));
    }

    let mut tx = pool.begin().await?;

    let sender_verify_response = Wallet::verify_address(pool, details.private_key).await?;
    if !sender_verify_response.authed {
        return Err(KristError::Address(AddressError::AuthFailed));
    }

    let sender = sender_verify_response.model;

    let is_name = NAME_META_RE.is_match(&details.to);

    let name_data = is_name.then(|| TransactionNameData::parse(&details.to));
    let (sent_metaname, sent_name) = match name_data {
        Some(name_data) => (name_data.metaname, name_data.name),
        None => (None, None),
    };

    let recipient = match is_name {
        true => {
            // Cursed but makes borrow checker happy, lol.
            let name = sent_name.as_deref().unwrap_or_default();

            let name = Name::fetch_by_name(&mut *tx, name)
                .await?
                .ok_or_else(|| KristError::Name(NameError::NameNotFound(details.to.clone())))?;

            let owner = name.owner(&mut *tx).await?;
            owner.ok_or_else(|| KristError::Name(NameError::NameNotFound(details.to.clone())))?
        }
        false => Wallet::fetch_by_address(&mut *tx, &details.to)
            .await?
            .ok_or_else(|| KristError::Address(AddressError::NotFound(details.to.clone())))?,
    };

    // Make sure to check the request to see if the funds are available.
    if sender.balance < amount {
        return Err(KristError::Transaction(TransactionError::InsufficientFunds));
    }

    if sender.address == recipient.address {
        return Err(KristError::Transaction(
            TransactionError::SameWalletTransfer,
        ));
    }

    let creation_data = TransactionCreateData {
        from: sender.address,
        to: recipient.address,
        amount,
        sent_metaname,
        sent_name,
        metadata: details.metadata,
        transaction_type: TransactionType::Transfer,
        ..Default::default()
    };

    let transaction = Transaction::create(&mut *tx, creation_data).await?;
    let transaction_json: TransactionJson = transaction.into();

    tx.commit().await?;

    let event = WebSocketMessage::new_event(WebSocketEvent::Transaction {
        transaction: transaction_json.clone(),
    });
    server.broadcast_event(event).await;

    let final_response = TransactionResponse {
        ok: true,
        transaction: transaction_json,
    };

    Ok(HttpResponse::Ok().json(final_response))
}

#[utoipa::path(
    get,
    path = "/api/krist/transactions/latest",
    params(PaginationParams),
    responses(
        (status = 200, description = "Get latest transactions", body = TransactionListResponse)
    )
)]
#[get("/latest")]
async fn transaction_latest(
    state: web::Data<AppState>,
    query: web::Query<PaginationParams>,
) -> Result<HttpResponse, KristError> {
    let params = query.into_inner();
    let pool = &state.pool;

    let total = Transaction::total_count_no_mined(pool, &params).await?;
    let transactions = Transaction::sorted_by_date(pool, &params).await?;

    let transactions: Vec<TransactionJson> =
        transactions.into_iter().map(|trans| trans.into()).collect();

    let response = TransactionListResponse {
        ok: true,
        count: transactions.len(),
        total,
        transactions,
    };

    Ok(HttpResponse::Ok().json(response))
}

#[utoipa::path(
    get,
    path = "/api/krist/transactions/{id}",
    params(
        ("id", description = "Transaction ID")
    ),
    responses(
        (status = 200, description = "Get transaction by ID", body = TransactionResponse),
        (status = 404, description = "Transaction not found")
    )
)]
#[get("/{id}")]
async fn transaction_get(
    state: web::Data<AppState>,
    id: web::Path<i64>,
) -> Result<HttpResponse, KristError> {
    let id = id.into_inner();
    let pool = &state.pool;

    let slim = Transaction::fetch_by_id(pool, id).await?;

    slim.map(|trans| TransactionResponse {
        ok: true,
        transaction: trans.into(),
    })
    .map(|response| HttpResponse::Ok().json(response))
    .ok_or_else(|| KristError::Transaction(TransactionError::NotFound))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/transactions")
            .service(transaction_create)
            .service(transaction_latest)
            .service(transaction_get)
            .service(transaction_list),
    );
}
