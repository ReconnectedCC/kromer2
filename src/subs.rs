//! This module handles all mutation logic for subscriptions.
//!
//! This, like many other parts of Kromer operates on the assumption that only Kromer is messing
//! with the DB in any significant way. I've built in some checks for our stored subscription state,
//! but this may result in untimely resolution of subscriptions
//!
//! This could defenitely be more efficient and save on DB calls but unless I see something that
//! makes that change worth while I'm not touching it (trust me, I tried to do write-through
//! caching bullshit but it made me want to pull my hair out. This is naive but works).
//!
//! ## Stored State
//!
//! We check if any subscriptions would occur in the next 1 minute. If any will, the first is
//! pulled down and saved. If not, we check again in another minute. If a new subscription is
//! created, we resync with the DB.
//!
//! This granularity means that its reccomended users do not create subscriptions with very short
//! subscription times.

use std::{str::FromStr, time::Duration};

use chrono::{DateTime, Utc};
use croner::Cron;
use rust_decimal::Decimal;
use sqlx::{Error as SqlxError, PgPool};
use tokio::{
    sync::mpsc::{Receiver, Sender},
    time::sleep,
};
use tokio_retry2::{Retry, RetryError, strategy::ExponentialBackoff};
use tracing::{info, warn};

use crate::{
    models::kromer::subs::{ContractStatus, SubStatus},
    websockets::WebSocketServer,
};

#[derive(Debug, Clone, sqlx::FromRow)]
struct LapsedInfo {
    subscription_id: i32,
    contract_id: i32,

    lapsed_at: DateTime<Utc>,

    #[sqlx(rename = "owner_id")]
    contractor_id: i32,
    #[sqlx(rename = "wallet_id")]
    contractee_id: i32,

    #[sqlx(rename = "address")]
    contractee_addr: String,

    contract_status: ContractStatus,
    sub_status: SubStatus,

    price: Decimal,
    cron_expr: String,
    allow_list: Option<Vec<String>>,
}

/// ZST used to notify the subscription manager that its backing state has changed.
pub struct SubUpdateNofif;

pub fn new_sub_manager(db: PgPool, ws: WebSocketServer) -> Sender<SubUpdateNofif> {
    let (tx, rx) = tokio::sync::mpsc::channel(25);

    tokio::spawn(sub_manager(db, rx, ws));

    tx
}

/// Long running task that processes our subscriptions. If this task errors, it is able to be
/// restarted with little effort.
async fn sub_manager(db: PgPool, mut rx: Receiver<SubUpdateNofif>, ws: WebSocketServer) {
    loop {
        let next_sub = fetch_soonest_retry(&db).await;

        match next_sub {
            Some(lapsed_at) => {
                tracing::info!("Next subscription lapses @ {}", lapsed_at);
                let sub_timer = sleep((lapsed_at - Utc::now()).to_std().unwrap_or_default());

                let process_lapsed =
                    async |db: &PgPool, _ws: &WebSocketServer| match try_process_lapsed(db).await {
                        Ok(res) => {
                            info!("Resolved lapsed subscription: {:?}", res);
                            // TODO: Dispatch messages to websocket, for both subscription and
                            // transaction
                        }
                        Err(err) => {
                            warn!("Failed to process a lapsed transaction: {}", err)
                        }
                    };

                tokio::select! {
                    _ = sub_timer => {
                       process_lapsed(&db, &ws).await
                    },
                    _ = recv_drain_all(&mut rx) => ()
                }
            }
            None => {
                info!("No pending subscriptions found");
                let sync_timer = sleep(Duration::from_secs(60));

                tokio::select! {
                    _ = sync_timer => (),
                    _ = recv_drain_all(&mut rx) => (),
                }
            }
        }
    }
}

async fn fetch_soonest_retry(db: &PgPool) -> Option<DateTime<Utc>> {
    let retry_strategy = ExponentialBackoff::from_millis(10).take(5);

    let action = async || fetch_soonest(db).await.map_err(RetryError::transient);

    Retry::spawn(retry_strategy, action)
        .await
        .unwrap_or_default()
}

async fn fetch_soonest(db: &PgPool) -> Result<Option<DateTime<Utc>>, SqlxError> {
    sqlx::query_scalar("SELECT lapsed_at FROM subscriptions WHERE lapsed_at < (NOW() + INTERVAL '1 MINUTE') ORDER BY lapsed_at LIMIT 1").fetch_optional(db).await
}

/// Waits until a message is received, then clears all pending messages. We want this behavior
/// because we resync the DB when getting any messages, so more than 1 are not needed at a time.
async fn recv_drain_all(rx: &mut Receiver<SubUpdateNofif>) {
    if rx.recv().await.is_none() {
        return;
    }
    tracing::debug!("Resetting sub manager state");
    while rx.try_recv().is_ok() {}
}

#[derive(Debug, thiserror::Error)]
pub enum ProcessLapsedError {
    #[error("Attempted to call when there were no lapsed subscriptions")]
    NoneLapsed,
    #[error(transparent)]
    DbError(#[from] SqlxError),
    #[error("Expected DB state did not match reality")]
    Desync,

    #[error(
        "Allowed users field changed, user is no longer able to subscribe to this subscription"
    )]
    Unauthorized,

    #[error("Contractee did not have enough funds to renew subscription")]
    InsufficientFunds,
}

/// Result returned from a
#[derive(Debug, Clone, Copy)]
pub struct LapsedRes {
    pub contract_id: i32,
    pub subscription_id: i32,
    pub status: LapsedStatus,
}

#[derive(Debug, Clone, Copy)]
pub enum LapsedStatus {
    Canceled,
    Renewed {
        next_lapsed: DateTime<Utc>,
        transction_id: i32,
    },
}

/// Attempts to process the first subscription
#[tracing::instrument]
async fn try_process_lapsed(db: &PgPool) -> Result<LapsedRes, ProcessLapsedError> {
    let q = "
        SELECT 
            s.subscription_id,  
            s.wallet_id, 
            s.lapsed_at, 
            s.status AS sub_status, 
            c.contract_id, 
            c.owner_id, 
            c.status AS contract_status, 
            c.price, 
            c.cron_expr,
            c.allow_list,
            w.address
        FROM subscriptions AS s
        LEFT JOIN contract_offers AS c ON s.contract_id = c.contract_id
        LEFT JOIN wallets AS w ON s.wallet_id = w.id
        WHERE s.lapsed_at < (NOW() + INTERVAL '10 SECONDS') ORDER BY s.lapsed_at LIMIT 1
        ";

    let info: LapsedInfo = sqlx::query_as(q)
        .fetch_optional(db)
        .await?
        .ok_or(ProcessLapsedError::NoneLapsed)?;

    match info.contract_status {
        ContractStatus::Canceled => cancel_sub(db, &info).await,
        ContractStatus::Open | ContractStatus::Closed => match info.sub_status {
            SubStatus::Active => continue_sub(db, &info).await,
            SubStatus::Pending | SubStatus::Canceled => match cancel_sub(db, &info).await {
                Err(ProcessLapsedError::Unauthorized | ProcessLapsedError::InsufficientFunds) => {
                    cancel_sub(db, &info).await
                }
                res => res,
            },
        },
    }
}

async fn cancel_sub(db: &PgPool, info: &LapsedInfo) -> Result<LapsedRes, ProcessLapsedError> {
    let row_n = sqlx::query(
        "UPDATE subscriptions SET lapsed_at = NULL, status = 'canceled' WHERE subscription_id = $1",
    )
    .bind(info.subscription_id)
    .execute(db)
    .await?
    .rows_affected();

    if row_n != 1 {
        Err(ProcessLapsedError::Desync)
    } else {
        Ok(LapsedRes {
            subscription_id: info.subscription_id,
            contract_id: info.contract_id,
            status: LapsedStatus::Canceled,
        })
    }
}

async fn continue_sub(db: &PgPool, info: &LapsedInfo) -> Result<LapsedRes, ProcessLapsedError> {
    let authorized_addr = match &info.allow_list {
        Some(v) => v.contains(&info.contractee_addr),
        None => true,
    };

    if !authorized_addr {
        return Err(ProcessLapsedError::Unauthorized);
    }

    let mut tx = db.begin().await?;

    sqlx::query(
        "UPDATE wallets SET balance = balance - $1, total_out = abs(total_out) + $2 WHERE id = $2",
    )
    .bind(info.price)
    .bind(info.contractee_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        if let Some(err) = e.as_database_error()
            && err.is_check_violation()
        {
            ProcessLapsedError::InsufficientFunds
        } else {
            ProcessLapsedError::DbError(e)
        }
    })?;

    sqlx::query(
        "UPDATE wallets SET balance = balance +$1, total_in = abs(total_out) + $2 WHERE id = $2",
    )
    .bind(info.price)
    .bind(info.contractor_id)
    .execute(&mut *tx)
    .await?;

    let transaction_id: i32 = sqlx::query_scalar(
        r#"INSERT INTO transactions 
                ("from", "to", amount, metadata, transaction_type, date) 
                VALUES (
                    (SELECT address FROM wallets WHERE id = $1), 
                    (SELECT address FROM wallets WHERE id = $2), 
                    $3, 
                    $4, 
                    'transfer',
                    NOW()
                ) 
                RETURNING id"#,
    )
    .bind(info.contractee_id)
    .bind(info.contractor_id)
    .bind(info.price)
    .bind(format!("sub_id={}", info.subscription_id))
    .fetch_one(&mut *tx)
    .await?;

    let next_time = Cron::from_str(&info.cron_expr)
        .and_then(|x| x.find_next_occurrence::<Utc>(&info.lapsed_at, false))
        .expect("Should be validated on insertion. If not, we cry :'(");

    sqlx::query("UPDATE subscriptions SET lapsed_at = $1 WHERE subscription_id = $2")
        .bind(next_time)
        .bind(info.subscription_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    Ok(LapsedRes {
        contract_id: info.contract_id,
        subscription_id: info.subscription_id,
        status: LapsedStatus::Renewed {
            next_lapsed: next_time,
            transction_id: transaction_id,
        },
    })
}
