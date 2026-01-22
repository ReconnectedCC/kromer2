use clap::Parser;
use sqlx::{Pool, Postgres};
use tokio::sync::OnceCell;

pub mod database;
pub mod errors;
pub mod guards;
pub mod models;
pub mod routes;
pub mod utils;
pub mod websockets;
static ARGS: OnceCell<Args> = OnceCell::const_new();

/// A mostly Krist-Compatible currency server for ComputerCraft, made by ReconnectedCC. Args override environment variables.
#[derive(Parser, Debug)]
#[command(about, long_about = None)]

pub struct Args {
    /// Enable debug mode, prints debug messages to the console

    #[arg(short, long)]
    pub debug: bool,
    #[arg(long)]
    /// Sets the Server URL
    pub url: Option<String>,
    /// Sets the Database URL
    #[arg(long)]
    pub database_url: Option<String>,
    /// Sets the Internal Key
    #[arg(long)]
    pub key: Option<String>,
    /// Force Websocket to use the insecure "ws://" protocol
    #[arg(short, long)]
    pub insecure: bool,
}

pub fn init_args(args: Args) {
    ARGS.set(args).unwrap();
}

pub fn get_args() -> &'static Args {
    ARGS.get().unwrap()
}

pub mod build_info {
    // The file has been placed there by the build script.
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

#[derive(Debug)]
pub struct AppState {
    pub pool: Pool<Postgres>,
}
