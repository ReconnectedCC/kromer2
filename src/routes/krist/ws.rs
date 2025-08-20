use std::str::FromStr;
use std::sync::Arc;
use std::time::Instant;

use actix_web::rt::time;
use actix_web::{HttpRequest, get, post};
use actix_web::{HttpResponse, web};
use actix_ws::AggregatedMessage;
use chrono::Utc;
use serde_json::json;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::AppState;
use crate::database::wallet::Model as Wallet;
use crate::errors::krist::{KristError, address::AddressError, websockets::WebSocketError};
use crate::models::krist::websockets::{WebSocketMessage, WebSocketMessageInner};
use crate::websockets::types::common::WebSocketTokenData;
use crate::websockets::types::convert_to_iso_string;
use crate::websockets::{CLIENT_TIMEOUT, HEARTBEAT_INTERVAL, WebSocketServer, handler, utils};

#[derive(serde::Deserialize)]
struct WsConnDetails {
    privatekey: String,
}

#[post("/start")]
#[tracing::instrument(name = "setup_ws_route", level = "debug", skip_all)]
pub async fn setup_ws(
    state: web::Data<AppState>,
    server: web::Data<WebSocketServer>,
    details: Option<web::Json<WsConnDetails>>,
) -> Result<HttpResponse, KristError> {
    let pool = &state.pool;
    let private_key = details.map(|json_details| json_details.privatekey.clone());

    let uuid = match private_key {
        Some(private_key) => {
            let wallet = Wallet::verify_address(pool, &private_key)
                .await
                .map_err(|_| KristError::Address(AddressError::AuthFailed))?;
            let model = wallet.model;

            let token_data = WebSocketTokenData::new(model.address, Some(private_key));

            server.obtain_token(token_data).await
        }
        None => {
            let token_data = WebSocketTokenData::new("guest".into(), None);

            server.obtain_token(token_data).await
        }
    };

    // Make the URL and return it to the user.
    let url = match utils::make_url::make_url(uuid) {
        Ok(value) => value,
        Err(_) => return Err(KristError::Custom("server_config_error")),
    };

    Ok(HttpResponse::Ok().json(json!({
        "ok": true,
        "url": url,
        "expires": 30
    })))
}

#[get("/gateway/{token}")]
#[tracing::instrument(name = "ws_gateway_route", level = "info", fields(token = *token), skip_all)]
pub async fn gateway(
    req: HttpRequest,
    body: web::Payload,
    state: web::Data<AppState>,
    server: web::Data<WebSocketServer>,
    token: web::Path<String>,
) -> Result<HttpResponse, actix_web::Error> {
    let server = server.into_inner(); // lol
    let token = token.into_inner();

    // TODO: Actually do what krist does, which is:
    //       - Let websocket connect
    //       - Send error over
    //       - Close connection
    let uuid = Uuid::from_str(&token)
        .map_err(|_| KristError::WebSocket(WebSocketError::InvalidWebsocketToken))?;

    let data = server
        .use_token(&uuid)
        .await
        .map_err(|_| KristError::WebSocket(WebSocketError::InvalidWebsocketToken))?;

    let (response, mut session, stream) = actix_ws::handle(&req, body)?;

    let mut stream = stream
        .max_frame_size(64 * 1024)
        .aggregate_continuations()
        .max_continuation_size(2 * 1024 * 1024);

    server.insert_session(uuid, session.clone(), data).await; // Not a big fan of cloning but here it is needed.

    let alive = Arc::new(Mutex::new(Instant::now()));
    let mut session2 = session.clone();
    let server2 = server.clone();
    let alive2 = alive.clone();

    handler::send_hello_message(&mut session).await;

    // Heartbeat handling
    actix_web::rt::spawn(async move {
        let mut interval = time::interval(HEARTBEAT_INTERVAL);

        loop {
            interval.tick().await;
            if session2.ping(b"").await.is_err() {
                tracing::error!("Failed to send ping message to session");
                break;
            }

            let cur_time = convert_to_iso_string(Utc::now());
            let message = WebSocketMessage {
                ok: None,
                id: None,
                r#type: WebSocketMessageInner::Keepalive {
                    server_time: cur_time,
                },
            };

            let return_message =
                serde_json::to_string(&message).unwrap_or_else(|_| "{}".to_string()); // ...what
            let _ = session2.text(return_message).await;

            if Instant::now().duration_since(*alive2.lock().await) > CLIENT_TIMEOUT {
                tracing::info!("Session {uuid} timed out");
                let _ = session2.close(None).await;
                server2.cleanup_session(&uuid).await;

                break;
            }
        }
    });

    // Messgage handling code here
    actix_web::rt::spawn(async move {
        while let Some(Ok(msg)) = stream.recv().await {
            match msg {
                AggregatedMessage::Ping(bytes) => {
                    if session.pong(&bytes).await.is_err() {
                        tracing::error!("Failed to send pong back to session");
                        return;
                    }
                }

                AggregatedMessage::Text(string) => {
                    if string.chars().count() > 512 {
                        // TODO: Possibly use error message struct in models
                        // This isn't super necessary though and this shortcut saves some unnecessary error handling...
                        let error_msg = json!({
                            "ok": "false",
                            "error": "message_too_long",
                            "message": "Message larger than 512 characters",
                            "type": "error"
                        })
                        .to_string();
                        tracing::info!("Message received was larger than 512 characters");

                        let _ = session.text(error_msg).await;
                    } else {
                        tracing::debug!("Message received: {string}");

                        let process_result =
                            handler::process_text_msg(&state.pool, &server, &uuid, &string).await;

                        if let Ok(message) = process_result {
                            let msg = serde_json::to_string(&message)
                                .expect("Failed to serialize message into string");
                            let _ = session.text(msg).await;
                        } else {
                            tracing::error!("Error in processing message")
                        }
                    }
                }

                AggregatedMessage::Close(reason) => {
                    let _ = session.close(reason).await;

                    tracing::info!("Got close, cleaning up");
                    server.cleanup_session(&uuid).await;

                    return;
                }

                AggregatedMessage::Pong(_) => {
                    tracing::trace!("Received a pong back! :D");
                    *alive.lock().await = Instant::now();
                }

                _ => (), // Binary data is just ignored
            }
        }

        let _ = session.close(None).await;
        server.cleanup_session(&uuid).await;
    });

    Ok(response)
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/ws").service(setup_ws).service(gateway));
}
