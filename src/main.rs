use actix_cors::Cors;
use actix_web::{App, HttpServer, middleware, web};
use clap::Parser;
use kromer::{AppState, Args, get_args, init_args, routes, websockets::WebSocketServer};
use sqlx::postgres::PgPool;
use std::env;

#[actix_web::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let parsed_args = Args::parse();
    init_args(parsed_args);
    let args = get_args();
    if !args.debug {
        tracing_subscriber::fmt::init();
    } else {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .init();
        tracing::info!("Debug mode enabled");
    }
    dotenvy::dotenv().ok();

    let server_url = args.url.clone().unwrap_or_else(|| {
        env::var("SERVER_URL")
            .expect("SERVER_URL is not set in .env file or as command line argument (--url)")
    });
    let database_url = args.database_url.clone().unwrap_or_else(|| {
        env::var("DATABASE_URL").expect(
            "DATABASE_URL is not set in .env file or as command line argument (--database_url)",
        )
    });

    let pool = PgPool::connect(&database_url).await?;

    tracing::info!("Running database migrations...");
    sqlx::migrate!("./migrations").run(&pool).await?;
    tracing::info!("Database migrations completed successfully");

    let krist_ws_server = WebSocketServer::new();
    let state = web::Data::new(AppState { pool });

    let http_server = HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allowed_methods(["GET", "POST", "PUT"])
            .allow_any_header()
            .max_age(3600);

        App::new()
            .app_data(state.clone())
            .app_data(web::Data::new(krist_ws_server.clone()))
            .wrap(middleware::Logger::new(
                r#"%a "%r" %s %b "%{Referer}i" "%{User-Agent}i" "%{X-CC-ID}i" %T"#,
            ))
            .wrap(middleware::NormalizePath::trim())
            .wrap(cors)
            .configure(routes::config)
            .default_service(web::route().to(routes::not_found::not_found))
    })
    .bind(&server_url)?
    .run();

    http_server.await?;

    Ok(())
}
