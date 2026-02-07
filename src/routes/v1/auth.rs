use actix_web::{HttpResponse, post, web};
use actix_web_httpauth::extractors::bearer::BearerAuth;

use crate::{
    AppState,
    auth::{AuthSessions, check_bearer},
    errors::KromerError,
    models::{
        krist::auth::LoginDetails,
        kromer::{auth::AuthenticatedResponse, responses::ApiResponse},
    },
};

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("").service(login).service(logout));
}

/// Begin a session for a given wallet by passing in the user ID. Sessions last for 1 hour and then
/// must be renewed.
#[utoipa::path(
    post,
    path = "/api/v1/login",
    request_body = LoginDetails,
    responses(
        (status = 200, description = "Session information", body = AuthenticatedResponse),
    )
)]
#[post("/login")]
pub async fn login(
    state: web::Data<AppState>,
    sessions: web::Data<AuthSessions>,
    query: web::Json<LoginDetails>,
) -> Result<HttpResponse, KromerError> {
    let inner = query.into_inner();

    let (token, expires, address) = sessions
        .register_pk(&state.pool, &inner.private_key)
        .await?;

    let response = ApiResponse {
        data: Some(AuthenticatedResponse {
            token,
            expires,
            address,
        }),
        ..Default::default()
    };

    Ok(HttpResponse::Ok().json(response))
}

/// Invalidate the current session, ending it immediately.
#[utoipa::path(
    post,
    path = "/api/v1/logout",
    responses(
        (status = 200, description = "Successfully logged out"),
    ),
    security(("bearerAuth" = [])),
)]
#[post("/logout")]
pub async fn logout(
    sessions: web::Data<AuthSessions>,
    auth: Option<BearerAuth>,
) -> Result<HttpResponse, KromerError> {
    let session_id = check_bearer(&sessions, auth)?;

    sessions.revoke(session_id)?;

    Ok(HttpResponse::Ok().finish())
}
