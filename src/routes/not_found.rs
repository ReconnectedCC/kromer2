use actix_web::HttpResponse;

use crate::errors::KromerError;

#[allow(clippy::unused_async)]
pub async fn not_found() -> Result<HttpResponse, KromerError> {
    Err(KromerError::NotFound)
}
