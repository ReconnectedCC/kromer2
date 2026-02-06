use actix_web::{HttpResponse, get, web};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::errors::KromerError;
use crate::websockets::WebSocketServer;

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionQuery {
    pub session: String,
}

#[get("/session")]
async fn get_session(
    server: web::Data<WebSocketServer>,
    params: web::Query<SessionQuery>,
) -> Result<HttpResponse, KromerError> {
    let target_uuid = match params.session.parse::<Uuid>() {
        Ok(uuid) => uuid,
        Err(err) => {
            tracing::error!("Parse error: {}", err);
            return Err(KromerError::Internal("Failed to parse UUID"));
        }
    };

    let session_ref = match server.sessions.get(&target_uuid) {
        Some(data) => data,
        None => {
            tracing::error!("Invalid session: {}", target_uuid);
            return Err(KromerError::Internal("Session not found"));
        }
    };

    let session_data = session_ref.value();

    Ok(HttpResponse::Ok().json(session_data))
}

#[get("/sessions")]
async fn get_sessions(server: web::Data<WebSocketServer>) -> Result<HttpResponse, KromerError> {
    let sessions = server.sessions.as_ref();

    Ok(HttpResponse::Ok().json(sessions))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/ws").service(get_session).service(get_sessions));
}
