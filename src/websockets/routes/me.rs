use sqlx::{Pool, Postgres};
use uuid::Uuid;

use crate::{
    database::wallet::Model as Wallet,
    models::krist::{
        addresses::AddressJson,
        websockets::{WebSocketMessage, WebSocketMessageInner, WebSocketMessageResponse},
    },
    websockets::WebSocketServer,
};

#[tracing::instrument(skip_all)]
pub async fn get_myself(
    pool: &Pool<Postgres>,
    server: &WebSocketServer,
    uuid: &Uuid,
    msg_id: Option<usize>,
) -> WebSocketMessage {
    let session_data = server
        .sessions
        .get_async(uuid)
        .await
        .expect("Expected session to exist, somehow it does not");

    if session_data.is_guest() {
        return WebSocketMessage {
            ok: Some(true),
            id: msg_id,
            r#type: WebSocketMessageInner::Response {
                data: WebSocketMessageResponse::Me {
                    is_guest: true,
                    address: None,
                },
            },
        };
    }

    let wallet = Wallet::fetch_by_address(pool, session_data.address.clone()).await;
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

    let wallet = wallet.unwrap(); // SAFETY: We made sure the database did not error.
    let wallet = match wallet {
        Some(wallet) => wallet,
        None => {
            return WebSocketMessage {
                ok: Some(false),
                id: msg_id,
                r#type: WebSocketMessageInner::Error {
                    error: "address_not_found".to_owned(),
                    message: format!("Address {} not found", session_data.address),
                },
            };
        }
    };

    let wallet_resp: AddressJson = wallet.into();

    WebSocketMessage {
        ok: Some(true),
        id: msg_id,
        r#type: WebSocketMessageInner::Response {
            data: WebSocketMessageResponse::Me {
                is_guest: false,
                address: Some(wallet_resp),
            },
        },
    }
}
