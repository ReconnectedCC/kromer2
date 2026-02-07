use chrono::Utc;
use sqlx::{Pool, Postgres};
use uuid::Uuid;

use super::{WebSocketServer, types::convert_to_iso_string};
use crate::{
    errors::{KromerError, websocket::WebSocketError},
    models::krist::{
        motd::{Constants, CurrencyInfo, DetailedMotd, PackageInfo},
        websockets::{WebSocketMessage, WebSocketMessageInner},
    },
    websockets::routes,
};

#[tracing::instrument(skip_all, fields(uuid = ?uuid))]
pub async fn process_text_msg(
    pool: &Pool<Postgres>,
    server: &WebSocketServer,
    uuid: &Uuid,
    text: &str,
) -> Result<WebSocketMessage, KromerError> {
    // strip leading and trailing whitespace (spaces, newlines, etc.)
    let msg = text.trim();

    // TODO: potentially change how this serialization is handled, so that we can properly extract "Invalid Parameter" errors.
    let parsed_msg_result: Result<WebSocketMessage, serde_json::Error> = serde_json::from_str(msg);

    let parsed_msg = match parsed_msg_result {
        Ok(value) => value,
        Err(err) => {
            tracing::error!("Could not parse JSON: {err}");
            return Err(KromerError::WebSocket(WebSocketError::JsonParseRead));
        }
    };

    let msg_type = parsed_msg.r#type;
    tracing::debug!("Message type was: {:?}", msg_type);
    let msg_id = parsed_msg.id; // NOTE: This is probably gonna error, lol

    let msg: WebSocketMessage = match msg_type {
        WebSocketMessageInner::Address {
            address,
            fetch_names,
        } => {
            let fetch_names = fetch_names.unwrap_or(false);
            routes::addresses::get_address(pool, address, fetch_names, msg_id).await
        }
        WebSocketMessageInner::Login { private_key } => {
            routes::auth::perform_login(pool, server, uuid, private_key, msg_id).await
        }
        WebSocketMessageInner::Logout => routes::auth::perform_logout(server, uuid, msg_id).await,
        WebSocketMessageInner::Me => routes::me::get_myself(pool, server, uuid, msg_id).await,
        WebSocketMessageInner::Subscribe { event } => {
            routes::subscriptions::subscribe(server, uuid, event, msg_id).await
        }
        WebSocketMessageInner::GetSubscriptionLevel => {
            routes::subscriptions::get_subscription_level(server, uuid, msg_id).await
        }
        WebSocketMessageInner::GetValidSubscriptionLevels => {
            routes::subscriptions::get_valid_subscription_levels(msg_id).await
        }
        WebSocketMessageInner::Unsubscribe { event } => {
            routes::subscriptions::unsubscribe(server, uuid, event, msg_id).await
        }
        WebSocketMessageInner::MakeTransaction {
            private_key,
            to,
            amount,
            metadata,
        } => {
            let private_key = match private_key {
                Some(key) => key,
                None => {
                    let session_data = server.fetch_session_data(uuid).await;

                    if let Some(session_data) = session_data
                        && let Some(private_key) = session_data.private_key
                    {
                        private_key
                    } else {
                        return Ok(WebSocketMessage {
                            ok: Some(false),
                            id: msg_id,
                            r#type: WebSocketMessageInner::Error {
                                error: "unauthorized".into(),
                                message: "You are not logged in.".into(),
                            },
                        });
                    }
                }
            };

            routes::transactions::make_transaction(
                pool,
                private_key,
                to,
                amount,
                metadata,
                msg_id,
                server,
            )
            .await
        }
        WebSocketMessageInner::Work => WebSocketMessage {
            ok: Some(true),
            id: msg_id,
            r#type: WebSocketMessageInner::Error {
                error: "mining_disabled".to_owned(),
                message: "Mining disabled".to_owned(),
            },
        },
        _ => WebSocketMessage {
            ok: Some(true),
            id: msg_id,
            r#type: WebSocketMessageInner::Error {
                error: "invalid_message_type".to_owned(),
                message: "Invalid message type".to_owned(),
            },
        }, // Responses not sent by client or unimplemented
    };

    Ok(msg)
}

pub async fn send_hello_message(session: &mut actix_ws::Session) {
    let cur_time = convert_to_iso_string(Utc::now());

    let hello_message = WebSocketMessage {
        ok: Some(true),
        id: None,
        r#type: WebSocketMessageInner::Hello {
            motd: Box::new(DetailedMotd {
                server_time: cur_time,
                motd: "Message of the day".to_string(),
                set: None,
                motd_set: None,
                public_url: "http://kromer.reconnected.cc".to_string(),
                public_ws_url: "http://kromer.reconnected.cc/api/krist/ws".to_string(),
                mining_enabled: false,
                transactions_enabled: true,
                debug_mode: true,
                work: 500,
                last_block: None,
                package: PackageInfo {
                    name: crate::build_info::PKG_NAME.to_string(),
                    version: crate::build_info::PKG_VERSION.to_string(),
                    author: "ReconnectedCC Team".to_string(),
                    license: "GPL-3.0".to_string(),
                    repository: "https://github.com/ReconnectedCC/kromer/".to_string(),
                    git_hash: crate::build_info::GIT_COMMIT_HASH.map(|s| s.to_string()),
                },
                constants: Constants {
                    wallet_version: 3,
                    nonce_max_size: 500,
                    name_cost: 500,
                    min_work: 50,
                    max_work: 500,
                    work_factor: 500.0,
                    seconds_per_block: 5000,
                },
                currency: CurrencyInfo {
                    address_prefix: "k".to_string(),
                    name_suffix: "kro".to_string(),
                    currency_name: "Kromer".to_string(),
                    currency_symbol: "KRO".to_string(),
                },
                notice: "Some awesome notice will go here".to_string(),
            }),
        },
    };

    if let Ok(msg) = serde_json::to_string(&hello_message)
        && session.text(msg).await.is_err()
    {
        tracing::debug!("Failed to send hello message (connection may have just closed)");
    }
}
