use std::str::FromStr;

use actix_web::{HttpResponse, get, post, web};
use actix_web_httpauth::extractors::bearer::BearerAuth;
use chrono::Utc;
use croner::Cron;
use rust_decimal::Decimal;
use sqlx::QueryBuilder;

use crate::{
    AppState,
    auth::check_bearer,
    errors::{KromerError, auth::AuthError, subs::SubsError},
    models::kromer::{
        responses::{ApiResponse, PaginatedResponse},
        subs::{
            ContractCreateRequest, ContractInfo, ContractQueryParams, ListSubscribersParams,
            SubscriptionInfo,
        },
    },
    utils::validation::is_valid_kromer_address,
};

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/subs")
            .service(create_contract)
            .service(list_contracts)
            .service(contract_by_id)
            .service(contract_subscribers),
    );
}

/// Create a contract
///
/// Creates a new contract under the currently authorized wallet.
#[utoipa::path(
    post,
    path = "/api/v1/subs/c",
    request_body = ContractCreateRequest,
    responses(
        (status = 200, description = "Created contract", body = ApiResponse<ContractInfo>),
        (status = 400, description = "A part of your request was not valid, check message"),
    ),
    security(("bearerAuth" = [])),
)]
#[post("/c")]
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

/// List contracts
#[utoipa::path(
    get,
    path = "/api/v1/c",
    params(ContractQueryParams),
    responses(
        (status = 200, description = "List contracts", body = ApiResponse<PaginatedResponse<ContractInfo>>)
    )
)]
#[get("/c")]
pub async fn list_contracts(
    state: web::Data<AppState>,
    query: web::Query<ContractQueryParams>,
) -> Result<HttpResponse, KromerError> {
    let offset = query.offset.unwrap_or(0).abs() as i64;
    let limit = query.limit.unwrap_or(50).abs().min(500) as i64;

    let is_open = query.is_open.unwrap_or_default();

    // Woo! Query builder fun

    let mut list_qb = QueryBuilder::new(
        "SELECT 
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
        FROM contract_offers AS c LEFT JOIN wallets AS w ON c.owner_id = w.id",
    );

    let mut count_qb = QueryBuilder::new(
        "SELECT COUNT(*) FROM contract_offers AS c LEFT JOIN wallets AS w on c.owner_id = w.id",
    );

    match (query.address.as_deref(), is_open) {
        (Some(addr), true) => {
            let frag = " WHERE c.status = 'open' AND w.address = ";

            list_qb.push(frag);
            list_qb.push_bind(addr);

            count_qb.push(frag);
            count_qb.push_bind(addr);
        }
        (Some(addr), false) => {
            let frag = " WHERE w.address = ";

            list_qb.push(frag);
            list_qb.push_bind(addr);

            count_qb.push(frag);
            count_qb.push_bind(addr);
        }
        (None, true) => {
            let frag = " WHERE c.status = 'open'";

            list_qb.push(frag);
            count_qb.push(frag);
        }
        (None, false) => (),
    };

    list_qb.push(" ORDER BY c.created_at LIMIT ");
    list_qb.push_bind(limit);
    list_qb.push(" OFFSET ");
    list_qb.push_bind(offset);

    let items: Vec<ContractInfo> = list_qb.build_query_as().fetch_all(&state.pool).await?;
    let table_len: i64 = count_qb.build_query_scalar().fetch_one(&state.pool).await?;

    let remaining = (table_len - (offset + items.len() as i64))
        .max(0)
        .try_into()
        .expect("Value cannot be negative");

    let res = ApiResponse {
        data: Some(PaginatedResponse {
            count: items.len(),

            items,
            remaining,
        }),
        ..Default::default()
    };

    Ok(HttpResponse::Ok().json(res))
}

/// Fetch contract info by ID
#[utoipa::path(
    get,
    path = "/api/v1/subs/c/{id}",
    params(
        ("id", description = "Contract ID")
    ),
    responses(
        (status = 200, description = "Contract info", body = ApiResponse<ContractInfo>),
        (status = 404, description = "Contract not found")
    )
)]
#[get("/c/{id}")]
pub async fn contract_by_id(
    state: web::Data<AppState>,
    id: web::Path<i32>,
) -> Result<HttpResponse, KromerError> {
    let id = id.into_inner();

    if id < 0 {
        return Err(SubsError::InvalidId(id).into());
    }

    let info: ContractInfo = sqlx::query_as(
        "SELECT 
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
        FROM contract_offers AS c LEFT JOIN wallets AS w ON c.owner_id = w.id WHERE contract_id = $1"
    ).bind(id).fetch_optional(&state.pool).await?.ok_or(SubsError::ContractNotFound(id))?;

    Ok(HttpResponse::Ok().json(ApiResponse {
        data: Some(info),
        ..Default::default()
    }))
}

#[utoipa::path(
    get,
    path = "/api/v1/c",
    params(ListSubscribersParams),
    responses(
        (status = 200, description = "List subscribers", body = ApiResponse<PaginatedResponse<SubscriptionInfo>>)
    )
)]
#[get("c/{id}/subscribers")]
pub async fn contract_subscribers(
    state: web::Data<AppState>,
    id: web::Path<i32>,
    query: web::Query<ListSubscribersParams>,
) -> Result<HttpResponse, KromerError> {
    let id = id.into_inner();

    let offset = query.offset.unwrap_or(0).abs() as i64;
    let limit = query.limit.unwrap_or(50).abs().min(500) as i64;

    let is_active = query.is_active.unwrap_or_default();

    if id < 0 {
        return Err(SubsError::InvalidId(id).into());
    }

    let mut list_qb = QueryBuilder::new(
        "SELECT
            w.address,
            s.subscription_id,
            s.status,
            s.lapsed_at,
            s.started_at
        FROM subscriptions AS s LEFT JOIN wallets AS w ON s.wallet_id = w.id",
    );

    let mut count_qb = QueryBuilder::new(
        "SELECT COUNT(*) FROM subscriptions AS s LEFT JOIN wallets AS w on s.wallet_id = w.id",
    );

    if is_active {
        let frag = " WHERE s.status = 'active'";

        list_qb.push(frag);
        count_qb.push(frag);
    }

    list_qb.push(" ORDER BY s.subscription_id LIMIT ");
    list_qb.push_bind(limit);
    list_qb.push(" OFFSET ");
    list_qb.push_bind(offset);

    let items: Vec<SubscriptionInfo> = list_qb.build_query_as().fetch_all(&state.pool).await?;
    let table_len: i64 = count_qb.build_query_scalar().fetch_one(&state.pool).await?;

    let remaining = (table_len - (offset + items.len() as i64))
        .max(0)
        .try_into()
        .expect("Value cannot be negative");

    let res = ApiResponse {
        data: Some(PaginatedResponse {
            count: items.len(),

            items,
            remaining,
        }),
        ..Default::default()
    };

    Ok(HttpResponse::Ok().json(res))
}
