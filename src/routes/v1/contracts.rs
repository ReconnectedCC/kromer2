use std::str::FromStr;

use actix_web::{HttpResponse, get, post, web};
use actix_web_httpauth::extractors::bearer::BearerAuth;
use chrono::Utc;
use croner::Cron;
use rust_decimal::Decimal;
use sqlx::{Pool, Postgres, QueryBuilder};
use tokio::sync::mpsc::Sender;
use utoipa::ToSchema;

use crate::{
    AppState,
    auth::{AuthSessions, check_bearer},
    errors::{KromerError, auth::AuthError, subs::SubsError},
    models::kromer::{
        Patch,
        responses::{ApiResponse, PaginatedResponse},
        subs::{
            ContractCreateRequest, ContractInfo, ContractQueryParams, ContractStatus,
            ListSubscribersParams, SubscriptionInfo, UpdateContractRequest,
        },
    },
    subs::SubUpdateNofif,
    utils::validation::is_valid_kromer_address,
};

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/contracts")
            .service(create_contract)
            .service(list_contracts)
            .service(contract_by_id)
            .service(contract_subscribers)
            .service(patch_contract),
    );
}

/// Create a contract
///
/// Creates a new contract under the currently authorized wallet.
#[utoipa::path(
    post,
    path = "/api/v1/contracts",
    request_body = ContractCreateRequest,
    responses(
        (status = 200, description = "Created contract", body = ApiResponse<ContractInfo>),
        (status = 400, description = "A part of your request was not valid, check message"),
    ),
    security(("bearerAuth" = [])),
)]
#[post("")]
pub async fn create_contract(
    state: web::Data<AppState>,
    sessions: web::Data<AuthSessions>,
    auth: Option<BearerAuth>,
    body: web::Json<ContractCreateRequest>,
) -> Result<HttpResponse, KromerError> {
    let body = body.into_inner();
    let session_id = check_bearer(&sessions, auth)?;

    validate_body(&body)?;

    let addr = sessions
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
    if !validate_length(&body.title, 1, 64) {
        return Err(SubsError::TitleLength);
    }

    if let Some(desc) = body.description.as_ref()
        && !validate_length(desc, 0, 500)
    {
        return Err(SubsError::InvalidDescription);
    }

    validate_cron_expr(&body.cron_expr)?;
    validate_allow_list(body.allow_list.as_deref())?;

    if body.price <= Decimal::ZERO {
        return Err(SubsError::InvalidPrice);
    }

    if body.max_subscribers.is_some_and(|n| n < 0) {
        return Err(SubsError::InvalidMaxSubscribers);
    }

    Ok(())
}

fn validate_cron_expr(s: &str) -> Result<(), SubsError> {
    let cron_expr = Cron::from_str(s).map_err(|_| SubsError::InvalidCronExpr)?;

    cron_expr
        .find_next_occurrence(&Utc::now(), false)
        .map_err(|_| SubsError::InvalidCronExpr)?;

    Ok(())
}

fn validate_allow_list(list: Option<&[String]>) -> Result<(), SubsError> {
    if let Some(l) = list {
        for (i, addr) in l.iter().enumerate() {
            if !is_valid_kromer_address(addr) {
                return Err(SubsError::InvalidAllowList(i));
            }
        }
    }

    Ok(())
}

/// List contracts
#[utoipa::path(
    get,
    path = "/api/v1/contracts",
    params(ContractQueryParams),
    responses(
        (status = 200, description = "List contracts", body = ApiResponse<PaginatedResponse<ContractInfo>>)
    )
)]
#[get("")]
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
    path = "/api/v1/contracts/{id}",
    params(
        ("id", description = "Contract ID")
    ),
    responses(
        (status = 200, description = "Contract info", body = ApiResponse<ContractInfo>),
        (status = 404, description = "Contract not found")
    )
)]
#[get("/{id}")]
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
    path = "/api/v1/contracts/{id}/subscribers",
    params(ListSubscribersParams),
    responses(
        (status = 200, description = "List subscribers", body = ApiResponse<PaginatedResponse<SubscriptionInfo>>)
    )
)]
#[get("/{id}/subscribers")]
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

    if !contract_exists(&state.pool, id).await? {
        return Err(SubsError::ContractNotFound(id).into());
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

    let frag = if is_active {
        " WHERE s.status = 'active' AND s.contract_id = "
    } else {
        " WHERE s.contract_id = "
    };

    list_qb.push(frag);
    list_qb.push_bind(id);
    count_qb.push(frag);
    count_qb.push_bind(id);

    list_qb.push(" ORDER BY s.subscription_id LIMIT ");
    list_qb.push_bind(limit);
    list_qb.push(" OFFSET ");
    list_qb.push_bind(offset);

    let items: Vec<SubscriptionInfo> = list_qb.build_query_as().fetch_all(&state.pool).await?;
    let Some(table_len): Option<i64> = count_qb
        .build_query_scalar()
        .fetch_optional(&state.pool)
        .await?
    else {
        return Err(SubsError::ContractNotFound(id).into());
    };

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

async fn contract_exists(db: &Pool<Postgres>, id: i32) -> Result<bool, sqlx::Error> {
    let res: i64 =
        sqlx::query_scalar("SELECT COUNT(1) FROM contract_offers WHERE contract_id = $1")
            .bind(id)
            .fetch_one(db)
            .await?;

    match res {
        1 => Ok(true),
        0 => Ok(false),
        _ => unreachable!(),
    }
}

/// Update a contract
///
/// I can't get OpenAPI to document this well so I'll write it out here. This request allows you to
/// update some or all of the fields on a contract. To update a field, its new parameter. For
/// nullable fields (`description`, `max_subscribers`, and `allow_list`), a `null` value will unset
/// the parameter. This is not the same as not including the field. For all others parameters, a
/// `null` is considered a no-op
#[utoipa::path(
    patch,
    path = "/api/v1/contracts/{id}",
    request_body = PatchContractSchema,

    responses(
        (status = 200, description = "Changed contract", body = inline(ApiResponse<ContractInfo>)),
        (status = 400, description = "A part of your request was not valid, check message"),
    ),
    security(("bearerAuth" = [])),
)]
#[actix_web::patch("/{id}")]
async fn patch_contract(
    state: web::Data<AppState>,
    id: web::Path<i32>,
    auth: Option<BearerAuth>,
    sub_tx: web::ThinData<Sender<SubUpdateNofif>>,
    sessions: web::Data<AuthSessions>,
    body: web::Json<UpdateContractRequest>,
) -> Result<HttpResponse, KromerError> {
    let body = body.into_inner();

    if body.is_empty() {
        return Err(SubsError::EmptyContractUpdate.into());
    }

    let session_id = check_bearer(&sessions, auth)?;
    let contract_id = id.into_inner();

    if contract_id < 0 {
        return Err(SubsError::InvalidId(contract_id).into());
    }

    let mut tx = state.pool.begin().await?;

    let mut contract_info: ContractInfo = sqlx::query_as(
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
        FROM contract_offers AS c LEFT OUTER JOIN wallets AS w ON c.owner_id = w.id WHERE contract_id = $1 FOR UPDATE OF c"
    ).bind(contract_id).fetch_optional(&mut *tx).await?.ok_or(SubsError::ContractNotFound(contract_id))?;

    if !sessions
        .is_authed_addr(session_id, &contract_info.address)
        .ok_or(AuthError::InvalidSession)?
    {
        return Err(AuthError::Unauthorized.into());
    }

    let mut update_subscribers = false;

    match body.title {
        Some(title) if validate_length(&title, 1, 64) => contract_info.title = title,
        Some(_) => return Err(SubsError::TitleLength.into()),
        None => {}
    }

    match body.description {
        Patch::Some(desc) if validate_length(&desc, 0, 500) => {
            contract_info.description = Some(desc)
        }
        Patch::Some(_) => return Err(SubsError::InvalidDescription.into()),
        Patch::Null => contract_info.description = None,
        Patch::None => {}
    }

    if let Some(status) = body.status {
        contract_info.status = status;
        if matches!(status, ContractStatus::Closed | ContractStatus::Canceled) {
            update_subscribers = true;
        }
    }

    match body.price {
        Some(price) if price > Decimal::ZERO => {
            if price != contract_info.price {
                update_subscribers = true;
                contract_info.price = price
            }
        }
        Some(_) => return Err(SubsError::InvalidPrice.into()),
        None => {}
    }

    if let Some(s) = body.cron_expr
        && s != contract_info.cron_expr
    {
        validate_cron_expr(&s)?;

        contract_info.cron_expr = s;
        update_subscribers = true;
    }

    match body.allow_list {
        Patch::Some(list) => {
            validate_allow_list(Some(&list))?;
            contract_info.allow_list = Some(list);
        }
        Patch::Null => contract_info.allow_list = None,
        Patch::None => {}
    }

    match body.max_subscribers {
        Patch::Some(max_subs) if max_subs > 0 => {
            contract_info.max_subscribers = Some(max_subs);
        }
        Patch::Some(_) => return Err(SubsError::InvalidMaxSubscribers.into()),
        Patch::Null => {
            contract_info.max_subscribers = None;
        }
        Patch::None => {}
    }

    let q = r#"
        UPDATE contract_offers SET 
            title = $1, 
            description = $2, 
            status = $3,
            price = $4,
            cron_expr = $5,
            max_subscribers = $6,
            allow_list = $7,
            updated_at = NOW()
        WHERE contract_id = $8
        RETURNING updated_at
    "#;

    contract_info.updated_at = sqlx::query_scalar(q)
        .bind(&contract_info.title)
        .bind(&contract_info.description)
        .bind(contract_info.status)
        .bind(contract_info.price)
        .bind(&contract_info.cron_expr)
        .bind(contract_info.max_subscribers)
        .bind(&contract_info.allow_list)
        .bind(contract_id)
        .fetch_one(&mut *tx)
        .await?;

    tx.commit().await?;

    if update_subscribers {
        let res = sub_tx.send(SubUpdateNofif).await;
        if res.is_err() {
            tracing::error!("Failed to notify subscription service of update");
        }
    }

    Ok(HttpResponse::Ok().json(ApiResponse {
        data: Some(contract_info),
        ..Default::default()
    }))
}

fn validate_length(s: &str, min: usize, max: usize) -> bool {
    let s_len = s.chars().count();

    s_len >= min && s_len <= max
}

/// Parameters to perform partial patches on an endpoint.
// Utoipa didn't like my patch type so here this is :(s
#[derive(ToSchema)]
pub struct PatchContractSchema {
    pub title: Option<String>,
    #[schema(nullable)]
    pub description: Option<String>,
    pub status: Option<ContractStatus>,
    pub price: Option<Decimal>,
    pub cron_expr: Option<String>,
    #[schema(nullable)]
    pub max_subscribers: Option<i32>,
    #[schema(nullable)]
    pub allow_list: Option<Vec<String>>,
}
