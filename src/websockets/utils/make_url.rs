use std::env;

use uuid::Uuid;

use crate::errors::{KromerError, websocket::WebSocketError};

pub fn make_url(uuid: Uuid) -> Result<String, KromerError> {
    let force_insecure = env::var("FORCE_WS_INSECURE").unwrap_or("true".to_owned());
    let schema = if force_insecure == "true" {
        "ws"
    } else {
        "wss"
    };

    let server_url = env::var("PUBLIC_URL")
        .map_err(|_| KromerError::WebSocket(WebSocketError::ServerConfigError))?;

    Ok(format!(
        "{schema}://{server_url}/api/krist/ws/gateway/{uuid}"
    ))
}
pub fn make_motd_urls() -> Result<Vec<String>, KromerError> {
    let force_insecure = env::var("FORCE_WS_INSECURE").unwrap_or("true".to_owned()); // Force WS Insecure probably means that you're using insecure http as well
    let schema = if force_insecure == "true" {
        "http"
    } else {
        "https"
    };

    let server_url = env::var("PUBLIC_URL")
        .map_err(|_| KromerError::WebSocket(WebSocketError::ServerConfigError))?; //TODO: do a proper error type here. its still a server config error with the same key, but this isn't a websocket thing

    Ok(vec![
        format!("{schema}://{server_url}"),
        format!("{schema}://{server_url}/api/krist/ws/"),
    ])
}
