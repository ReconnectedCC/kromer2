use actix_web::{HttpResponse, get, web};

use crate::{
    errors::KromerError,
    models::kromer::{responses::ApiResponse, websockets::SessionCountResponse},
    websockets::WebSocketServer,
};

#[utoipa::path(
    get,
    path = "/api/v1/ws/session/count",
    responses(
        (status = 200, description = "Total connected websockets", body = ApiResponse<SessionCountResponse>),
    )
)]
#[get("/session/count")]
async fn ws_session_get_count(
    server: web::Data<WebSocketServer>,
) -> Result<HttpResponse, KromerError> {
    let sessions = &server.inner.lock().await.sessions;

    let response = ApiResponse {
        data: Some(SessionCountResponse {
            count: sessions.len(),
        }),
        ..Default::default()
    };

    Ok(HttpResponse::Ok().json(response))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/ws").service(ws_session_get_count));
}
