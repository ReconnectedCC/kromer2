mod internal;
pub mod krist;
pub mod not_found;
pub mod v1;

use actix_web::{HttpResponse, get, middleware, web};
use utoipa::{IntoParams, ToSchema};

use crate::{errors::krist::KristError, guards};

#[get("/")]
pub async fn index_get() -> Result<HttpResponse, KristError> {
    Ok(HttpResponse::Ok().body("Hello, world!"))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    let krist_json_cfg =
        web::JsonConfig::default().error_handler(|err, _req| KristError::JsonPayload(err).into());

    let krist_path_config =
        web::PathConfig::default().error_handler(|err, _req| KristError::Path(err).into());

    cfg.service(
        web::scope("/api/v1")
            .wrap(middleware::NormalizePath::trim())
            .app_data(krist_json_cfg.clone()) // TODO: Custom.
            .app_data(krist_path_config.clone())
            .configure(v1::config),
    );
    cfg.service(
        web::scope("/api/krist")
            .wrap(middleware::NormalizePath::trim())
            .app_data(krist_json_cfg)
            .app_data(krist_path_config)
            .configure(krist::config),
    );
    cfg.service(
        web::scope("/api/_internal")
            .wrap(middleware::NormalizePath::trim())
            .guard(guards::internal_key_guard)
            .configure(internal::config),
    );
    cfg.service(web::scope("").service(index_get));
}

#[derive(Debug, serde::Deserialize, serde::Serialize, ToSchema, IntoParams)]
pub struct PaginationParams {
    #[serde(alias = "excludeMined")]
    // Only used on /transactions routes
    pub exclude_mined: Option<bool>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

impl Default for PaginationParams {
    fn default() -> Self {
        Self {
            exclude_mined: None,
            limit: Some(50),
            offset: Some(0),
        }
    }
}
