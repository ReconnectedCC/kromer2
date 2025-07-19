use rust_decimal::{Decimal, dec};
use sqlx::{Pool, Postgres};

use crate::{
    database::transaction::{TransactionCreateData, TransactionType},
    models::websockets::{
        WebSocketEvent, WebSocketMessage, WebSocketMessageInner, WebSocketMessageResponse,
    },
    websockets::WebSocketServer,
};

use crate::database::transaction::Model as Transaction;
use crate::database::wallet::Model as Wallet;

pub async fn make_transaction(
    pool: &Pool<Postgres>,
    private_key: String,
    to: String,
    amount: Decimal,
    metadata: Option<String>,
    msg_id: Option<usize>,
    server: &WebSocketServer,
) -> WebSocketMessage {
    let amount = amount.round_dp(2); // Make sure we do not support 2 decimals after the dot.

    if amount < dec!(0.00) {
        return WebSocketMessage {
            ok: Some(false),
            id: msg_id,
            r#type: WebSocketMessageInner::Error {
                error: "invalid_parameter".to_owned(),
                message: "Invalid parameter amount".to_owned(),
            },
        };
    }

    let resp = match Wallet::verify_address(pool, private_key).await {
        Ok(resp) => resp,
        Err(_) => {
            return WebSocketMessage {
                ok: Some(false),
                id: msg_id,
                r#type: WebSocketMessageInner::Error {
                    error: "database_error".to_owned(),
                    message: "An error occured in the database".to_owned(),
                },
            };
        }
    };
    if !resp.authed {
        return WebSocketMessage {
            ok: Some(false),
            id: msg_id,
            r#type: WebSocketMessageInner::Error {
                error: "invalid_parameter".to_owned(),
                message: "Invalid parameter privatekey".to_owned(),
            },
        };
    }

    let sender = resp.model;

    let recipient = match Wallet::fetch_by_address(pool, to.clone()).await {
        Ok(model) => model,
        Err(_) => {
            return WebSocketMessage {
                ok: Some(false),
                id: msg_id,
                r#type: WebSocketMessageInner::Error {
                    error: "database_error".to_owned(),
                    message: "An error occured in the database".to_owned(),
                },
            };
        }
    };

    let recipient = match recipient {
        Some(wallet) => wallet,
        None => {
            return WebSocketMessage {
                ok: Some(false),
                id: msg_id,
                r#type: WebSocketMessageInner::Error {
                    error: "address_not_found".to_owned(),
                    message: format!("Address {} not found", to),
                },
            };
        }
    };

    if sender.balance < amount {
        return WebSocketMessage {
            ok: Some(false),
            id: msg_id,
            r#type: WebSocketMessageInner::Error {
                error: "insufficient_funds".to_owned(),
                message: "Insufficient funds".to_owned(),
            },
        };
    }

    let creation_data = TransactionCreateData {
        from: sender.address.clone(),
        to: recipient.address.clone(),
        amount,
        metadata: metadata.clone(),
        transaction_type: TransactionType::Transfer,
    };

    let response = Transaction::create(pool, creation_data).await;

    if response.is_err() {
        return WebSocketMessage {
            ok: Some(false),
            id: msg_id,
            r#type: WebSocketMessageInner::Error {
                error: "database_error".to_owned(),
                message: "An error occured in the database".to_owned(),
            },
        };
    }
    let transaction = response.unwrap();
    let event = WebSocketMessage::new_event(WebSocketEvent::Transaction {
        transaction: transaction.clone().into(),
    });
    server.broadcast_event(event).await;

    WebSocketMessage {
        ok: Some(true),
        id: msg_id,
        r#type: WebSocketMessageInner::Response {
            responding_to: "make_transaction".to_owned(),
            data: WebSocketMessageResponse::MakeTransaction {
                transaction: transaction.into(),
            },
        },
    }
}
