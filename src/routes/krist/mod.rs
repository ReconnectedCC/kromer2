pub mod lookup;
pub mod misc;
pub mod names;
pub mod transactions;
pub mod wallet;
pub mod ws;

use actix_web::{HttpResponse, get, web};

#[get("")]
pub async fn index_get() -> HttpResponse {
    HttpResponse::Ok().body("Hello! This is the Krist compatible API provided by Kromer, some functionality might be slightly different but most software made for krist should work")
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/lookup").configure(lookup::config));

    cfg.service(index_get);

    cfg.configure(wallet::config);
    cfg.configure(transactions::config);
    cfg.configure(ws::config);
    cfg.configure(names::config);
    cfg.configure(misc::config);
}
