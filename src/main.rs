use actix_cors::Cors;
use actix_web::{App, HttpServer, middleware, web};
use sqlx::postgres::PgPool;
use std::env;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use kromer::{AppState, auth::AuthAddon, routes, websockets::WebSocketServer};

#[actix_web::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    dotenvy::dotenv().ok();

    let server_url = env::var("SERVER_URL").expect("SERVER_URL is not set in .env file");
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL is not set in .env file");

    let pool = PgPool::connect(&database_url).await?;

    tracing::info!("Running database migrations...");
    sqlx::migrate!("./migrations").run(&pool).await?;
    tracing::info!("Database migrations completed successfully");

    let krist_ws_server = WebSocketServer::new();

    let _sub_tx = kromer::subs::new_sub_manager(pool.clone(), krist_ws_server.clone());

    let state = web::Data::new(AppState {
        pool,
        auth: Default::default(),
    });

    #[derive(OpenApi)]
    #[openapi(
        paths(
            routes::v1::wallet::wallet_get_by_uuid,
            routes::v1::wallet::wallet_get_by_name,
            routes::v1::ws::ws_session_get_count,
            routes::v1::auth::login,
            routes::v1::auth::logout,
            routes::v1::subs::create_contract,
            routes::krist::transactions::transaction_list,
            routes::krist::transactions::transaction_create,
            routes::krist::transactions::transaction_latest,
            routes::krist::transactions::transaction_get,
            routes::krist::misc::login_address,
            routes::krist::misc::get_motd,
            routes::krist::misc::get_walletversion,
            routes::krist::misc::get_v2_address,
            routes::krist::misc::get_kromer_supply,
            routes::krist::names::name_list,
            routes::krist::names::name_cost,
            routes::krist::names::name_check,
            routes::krist::names::name_bonus,
            routes::krist::names::name_new,
            routes::krist::names::name_get,
            routes::krist::names::name_register,
            routes::krist::names::name_update_data,
            routes::krist::names::name_transfer,
            routes::krist::wallet::wallet_list,
            routes::krist::wallet::wallet_get,
            routes::krist::wallet::wallet_richest,
            routes::krist::wallet::wallet_get_transactions,
            routes::krist::wallet::wallet_get_names,
            routes::krist::lookup::addresses::addresses_lookup
        ),
        components(schemas(
            kromer::models::kromer::wallets::Wallet,
            kromer::models::kromer::websockets::SessionCountResponse,
            kromer::models::kromer::responses::None,
            kromer::models::kromer::responses::ResponseMeta,
            kromer::models::kromer::responses::ApiError,
            kromer::models::kromer::responses::ErrorDetail,
            kromer::models::krist::transactions::TransactionListResponse,
            kromer::models::krist::transactions::TransactionDetails,
            kromer::models::krist::transactions::TransactionResponse,
            kromer::models::krist::transactions::AddressTransactionQuery,
            kromer::models::krist::transactions::TransactionJson,
            kromer::database::transaction::TransactionType,
            kromer::routes::PaginationParams,
            kromer::models::krist::auth::LoginDetails,
            kromer::models::krist::auth::AddressAuthenticationResponse,
            kromer::models::krist::misc::WalletVersionResponse,
            kromer::models::krist::misc::MoneySupplyResponse,
            kromer::models::krist::misc::PrivateKeyAddressResponse,
            kromer::models::krist::names::NameListResponse,
            kromer::models::krist::names::NameResponse,
            kromer::models::krist::names::NameCostResponse,
            kromer::models::krist::names::DetailedUnpaidResponseRow,
            kromer::models::krist::names::NameAvailablityResponse,
            kromer::models::krist::names::NameBonusResponse,
            kromer::models::krist::names::RegisterNameRequest,
            kromer::models::krist::names::TransferNameRequest,
            kromer::models::krist::names::NameDataUpdateBody,
            kromer::models::krist::names::NameJson,
            kromer::models::krist::motd::DetailedMotdResponse,
            kromer::models::krist::motd::Motd,
            kromer::models::krist::motd::DetailedMotd,
            kromer::models::krist::motd::PackageInfo,
            kromer::models::krist::motd::Constants,
            kromer::models::krist::motd::CurrencyInfo,
            kromer::models::krist::blocks::BlockJson,
            kromer::models::krist::blocks::SubmitBlockResponse,
            kromer::models::krist::addresses::AddressListResponse,
            kromer::models::krist::addresses::AddressResponse,
            kromer::models::krist::addresses::AddressCreationResponse,
            kromer::models::krist::addresses::AddressJson,
            kromer::models::krist::addresses::VerifyResponse,
            kromer::models::krist::addresses::AddressGetQuery,
            kromer::models::krist::webserver::lookup::addresses::LookupResponse,
            kromer::models::krist::webserver::lookup::addresses::QueryParameters,
            kromer::models::kromer::auth::AuthenticatedResponse,
        )),
        modifiers(&AuthAddon),
    )]
    struct ApiDocs;

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
            .wrap(cors)
            .service(web::redirect("/swagger-ui", "/swagger-ui/")) // kinda cursed but it does work!
            .service(
                SwaggerUi::new("/swagger-ui/{_:.*}")
                    .url("/api-docs/openapi.json", ApiDocs::openapi()),
            )
            .configure(routes::config)
            .default_service(web::route().to(routes::not_found::not_found))
    })
    .bind(&server_url)?
    .run();

    http_server.await?;

    Ok(())
}
