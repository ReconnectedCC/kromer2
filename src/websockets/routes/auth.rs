use sqlx::{Pool, Postgres};
use uuid::Uuid;

use crate::database::wallet::Model as Wallet;
use crate::errors::KromerError;
use crate::errors::wallet::WalletError;
use crate::models::krist::websockets::{
    WebSocketMessage, WebSocketMessageInner, WebSocketMessageResponse,
};
use crate::websockets::WebSocketServer;

#[tracing::instrument(skip_all)]
pub async fn perform_login(
    pool: &Pool<Postgres>,
    server: &WebSocketServer,
    uuid: &Uuid,
    private_key: String,
    msg_id: Option<usize>,
) -> WebSocketMessage {
    let wallet = Wallet::verify_address(pool, private_key.clone())
        .await
        .map_err(|_| KromerError::Wallet(WalletError::AuthFailed));

    // TODO: Refactor this fuckass match statement so we dont have a billion nested structs, lol
    match wallet {
        Ok(response) => {
            if response.authed {
                let wallet = response.model;

                let mut session = server
                    .sessions
                    .get_mut(uuid)
                    .expect("Expected the session to exist, why doesn't it?");
                session.address = wallet.address.clone();
                session.private_key = Some(private_key);

                tracing::debug!("Session successfully logged in");

                WebSocketMessage {
                    ok: Some(true),
                    id: msg_id,
                    r#type: WebSocketMessageInner::Response {
                        data: WebSocketMessageResponse::Login {
                            is_guest: false,
                            address: Some(wallet.into()),
                        },
                    },
                }
            } else {
                WebSocketMessage {
                    ok: Some(true),
                    id: msg_id,
                    r#type: WebSocketMessageInner::Response {
                        data: WebSocketMessageResponse::Login {
                            is_guest: true,
                            address: None,
                        },
                    },
                }
            }
        }
        Err(_) => WebSocketMessage {
            ok: Some(true),
            id: msg_id,
            r#type: WebSocketMessageInner::Response {
                data: WebSocketMessageResponse::Login {
                    is_guest: true,
                    address: None,
                },
            },
        },
    }
}

pub async fn perform_logout(
    server: &WebSocketServer,
    uuid: &Uuid,
    msg_id: Option<usize>,
) -> WebSocketMessage {
    let mut session = server
        .sessions
        .get_mut(uuid)
        .expect("Expected the session to exist, why doesn't it?");
    session.address = String::from("guest");
    session.private_key = None;

    WebSocketMessage {
        ok: Some(true),
        id: msg_id,
        r#type: WebSocketMessageInner::Response {
            data: WebSocketMessageResponse::Logout { is_guest: true },
        },
    }
}
