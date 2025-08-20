mod internal;
mod krist;
pub mod not_found;
mod v1;

use actix_web::{HttpResponse, get, web};

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
            .app_data(krist_json_cfg.clone()) // TODO: Custom.
            .app_data(krist_path_config.clone())
            .configure(v1::config),
    );
    cfg.service(
        web::scope("/api/krist")
            .app_data(krist_json_cfg)
            .app_data(krist_path_config)
            .configure(krist::config),
    );
    cfg.service(
        web::scope("/api/_internal")
            .guard(guards::internal_key_guard)
            .configure(internal::config),
    );
    cfg.service(web::scope("").service(index_get));
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
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
