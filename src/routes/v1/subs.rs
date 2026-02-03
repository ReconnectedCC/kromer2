use std::str::FromStr;

use actix_web::{HttpResponse, post, web};
use actix_web_httpauth::extractors::bearer::BearerAuth;
use chrono::Utc;
use croner::Cron;
use rust_decimal::Decimal;

use crate::{
    AppState,
    auth::check_bearer,
    errors::{KromerError, auth::AuthError, subs::SubsError},
    models::kromer::{
        responses::ApiResponse,
        subs::{ContractCreateRequest, ContractInfo},
    },
    utils::validation::is_valid_kromer_address,
};

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/subs").service(create_contract));
}

/// Create a contract
#[utoipa::path(
    post,
    path = "/api/v1/subs/create",
    security(("bearerAuth" = [])),
)]
#[post("/create")]
pub async fn create_contract(
    state: web::Data<AppState>,
    auth: Option<BearerAuth>,
    body: web::Json<ContractCreateRequest>,
) -> Result<HttpResponse, KromerError> {
    let body = body.into_inner();
    let session_id = check_bearer(&state, auth).await?;

    validate_body(&body)?;

    let addr = state
        .auth
        .get_address(session_id)
        .ok_or(AuthError::InvalidSession)?;

    let contract: ContractInfo = sqlx::query_as(
        "WITH c AS (
            INSERT INTO contract_offers (
                owner_id, 
                title, 
                description, 
                price, 
                max_subscribers, 
                allow_list,
                cron_expr
            ) VALUES (
                (SELECT id FROM wallets WHERE address = $1), 
                $2, 
                $3, 
                $4, 
                $5,
                $6,
                $7
            ) RETURNING *
        ) SELECT 
            w.address, 
            c.contract_id,
            c.title, 
            c.description, 
            c.status,
            c.price, 
            c.max_subscribers, 
            c.allow_list, 
            c.created_at,
            c.updated_at,
            c.cron_expr
        FROM c LEFT JOIN wallets AS w ON c.owner_id = w.id",
    )
    .bind(addr)
    .bind(body.title)
    .bind(body.description)
    .bind(body.price)
    .bind(body.max_subscribers)
    .bind(body.allow_list)
    .bind(body.cron_expr)
    .fetch_one(&state.pool)
    .await?;

    let res = ApiResponse {
        data: Some(contract),
        ..Default::default()
    };

    Ok(HttpResponse::Ok().json(res))
}

fn validate_body(body: &ContractCreateRequest) -> Result<(), SubsError> {
    // Perform a bunch of validations
    let cron_expr = Cron::from_str(&body.cron_expr).map_err(|_| SubsError::InvalidCronExpr)?;
    cron_expr
        .find_next_occurrence(&Utc::now(), false)
        .map_err(|_| SubsError::InvalidCronExpr)?;

    if let Some(allow_list) = &body.allow_list {
        for (i, addr) in allow_list.iter().enumerate() {
            if !is_valid_kromer_address(addr) {
                return Err(SubsError::InvalidAllowList(i));
            }
        }
    }

    if body.price <= Decimal::ZERO {
        return Err(SubsError::InvalidPrice);
    }

    if body.max_subscribers.is_some_and(|n| n < 0) {
        return Err(SubsError::InvalidMaxSubscribers);
    }

    Ok(())
}
