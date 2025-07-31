mod addresses;
mod names;
mod transactions;

use actix_web::web;

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/addresses").configure(addresses::config));
    cfg.service(web::scope("/transactions").configure(transactions::config));
    cfg.service(web::scope("/names").configure(names::config));
}
