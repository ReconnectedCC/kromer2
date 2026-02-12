use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
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
    privatekey: Option<String>, // I hate our users, they cant follow any directions can they?
}

#[post("/start")]
#[tracing::instrument(name = "setup_ws_route", level = "debug", skip_all)]
pub async fn setup_ws(
    req: HttpRequest,
    state: web::Data<AppState>,
    server: web::Data<WebSocketServer>,
    details: Option<web::Json<WsConnDetails>>,
) -> Result<HttpResponse, KristError> {
    let pool = &state.pool;

    // I can not trust our users to be responsible, if they could not send a fucking json object
    // with fuck all in it, I would be so happy <3
    let private_key = details.and_then(|d| d.into_inner().privatekey);

    let computer_id = req
        .headers()
        .get("X-CC-ID")
        .and_then(|id| id.to_str().ok())
        .and_then(|s| s.parse::<i32>().ok());

    let uuid = match private_key {
        Some(private_key) => {
            let wallet = Wallet::verify_address(pool, &private_key)
                .await
                .map_err(|_| KristError::Address(AddressError::AuthFailed))?;
            let model = wallet.model;

            let token_data = WebSocketTokenData::new(model.address, Some(private_key), computer_id);

            server.obtain_token(token_data)
        }
        None => {
            let token_data = WebSocketTokenData::new("guest".into(), None, computer_id);

            server.obtain_token(token_data)
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
        .map_err(|_| KristError::WebSocket(WebSocketError::InvalidWebsocketToken))?;

    let (response, mut session, stream) = actix_ws::handle(&req, body)?;

    let mut stream = stream
        .max_frame_size(64 * 1024)
        .aggregate_continuations()
        .max_continuation_size(2 * 1024 * 1024);

    server.insert_session(uuid, session.clone(), data); // Not a big fan of cloning but here it is needed.

    let alive = Arc::new(Mutex::new(Instant::now()));
    let session_closed = Arc::new(AtomicBool::new(false));

    let mut session2 = session.clone();
    let server2 = server.clone();

    let alive2 = alive.clone();
    let session_closed2 = session_closed.clone();

    handler::send_hello_message(&mut session).await;

    let cleanup_session =
        |server: Arc<WebSocketServer>, uuid: Uuid, session_closed: Arc<AtomicBool>| {
            if session_closed
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                server.cleanup_session(&uuid);
            }
        };

    // Heartbeat handling
    let heartbeat_handle = actix_web::rt::spawn(async move {
        let mut interval = time::interval(HEARTBEAT_INTERVAL);

        loop {
            interval.tick().await;

            if session_closed2.load(Ordering::SeqCst) {
                tracing::debug!("Session was closed, breaking out of heartbeat handle");
                break;
            }

            let last_heartbeat = alive2.lock().await;
            if Instant::now().duration_since(*last_heartbeat) > CLIENT_TIMEOUT {
                tracing::info!("Session timed out");

                // Don't call close() - it hangs when client is unresponsive.
                // Just cleanup and let the connection die naturally.
                cleanup_session(server2, uuid, session_closed2);

                // Explicit drop MutexGuard, i do not fucking trust it.
                drop(last_heartbeat);

                break;
            }

            // Explicit drop MutexGuard, i do not fucking trust it.
            drop(last_heartbeat);

            if session2.ping(b"").await.is_err() {
                tracing::warn!("Failed to send ping message to session, cleaning it up");

                // Don't call close() - connection is already broken and close() can hang.
                // Just cleanup and let the connection die naturally.
                cleanup_session(server2, uuid, session_closed2);

                break;
            }

            let cur_time = convert_to_iso_string(Utc::now());
            let message = WebSocketMessage {
                ok: None,
                id: None,
                r#type: WebSocketMessageInner::Keepalive {
                    server_time: Some(cur_time),
                },
            };

            if let Ok(msg) = serde_json::to_string(&message) {
                // If keepalive send fails, just log it - the ping already succeeded
                // so the connection is still alive. Don't cleanup here.
                if session2.text(msg).await.is_err() {
                    tracing::debug!(
                        "Failed to send keepalive text message (connection may have just closed)"
                    );
                }
            }
        }
    });

    // Messgage handling code here
    actix_web::rt::spawn(async move {
        async {
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
                                handler::process_text_msg(&state.pool, &server, &uuid, &string)
                                    .await;

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
                        server.cleanup_session(&uuid);

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
            cleanup_session(server, uuid, session_closed);
        }
        .await;

        // Always abort the heartbeat task when message handling exits
        heartbeat_handle.abort();
    });

    Ok(response)
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/ws").service(setup_ws).service(gateway));
}
