use actix_web::HttpResponse;

use crate::errors::KromerError;

pub async fn not_found() -> Result<HttpResponse, KromerError> {
    Err(KromerError::NotFound)
}
