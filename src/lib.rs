use sqlx::{Pool, Postgres};

pub mod database;
pub mod errors;
pub mod guards;
pub mod models;
pub mod routes;
pub mod subs;
pub mod utils;
pub mod websockets;

pub mod build_info {
    // The file has been placed there by the build script.
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

#[derive(Debug)]
pub struct AppState {
    pub pool: Pool<Postgres>,
}
