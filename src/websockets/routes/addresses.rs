use sqlx::{Pool, Postgres};

use crate::models::krist::websockets::{
    WebSocketMessage, WebSocketMessageInner, WebSocketMessageResponse,
};

use crate::database::wallet::Model as Wallet;

#[tracing::instrument(skip_all)]
pub async fn get_address(
    pool: &Pool<Postgres>,
    address: String,
    _fetch_names: bool,
    msg_id: Option<usize>,
) -> WebSocketMessage {
    let wallet = Wallet::fetch_by_address(pool, address.clone()).await;
    if wallet.is_err() {
        let err = wallet.err().unwrap(); // SAFETY: We made sure it's an error
        tracing::error!("Caught an error: {err}");

        return WebSocketMessage {
            ok: Some(false),
            id: msg_id,
            r#type: WebSocketMessageInner::Error {
                error: "internal_server_error".to_owned(),
                message: "Something went wrong while processing your message".to_owned(),
            },
        };
    }

    let wallet = wallet.unwrap();
    let wallet = match wallet {
        Some(wallet) => wallet,
        None => {
            return WebSocketMessage {
                ok: Some(false),
                id: msg_id,
                r#type: WebSocketMessageInner::Error {
                    error: "address_not_found".to_owned(),
                    message: format!("Address {} not found", address),
                },
            };
        }
    };

    WebSocketMessage {
        ok: Some(true),
        id: msg_id,
        r#type: WebSocketMessageInner::Response {
            data: WebSocketMessageResponse::Address {
                address: wallet.into(),
            },
        },
    }
}
