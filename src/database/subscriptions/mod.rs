pub mod contracts;
pub mod proposals;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Encode, Executor, Postgres, prelude::Type};

use crate::database::{DatabaseError, ModelExt, Result};

#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct Model {
    pub id: i32,
    pub payer_wallet_id: i32,
    pub payee_wallet_id: i32,

    pub current_contract_id: i64,
    pub next_run_at: Option<DateTime<Utc>>,
    pub start_at: DateTime<Utc>,
    pub status: SubscriptionStatus,
    pub run_count: i32,
    pub failure_policy: FailurePolicy,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[async_trait::async_trait]
impl<'q> ModelExt<'q> for Model {
    async fn fetch_by_id<T, E>(pool: E, id: T) -> Result<Option<Self>>
    where
        Self: Sized,
        T: 'q + Encode<'q, Postgres> + Type<Postgres> + Send,
        E: 'q + Executor<'q, Database = Postgres>,
    {
        let q = "SELECT * FROM subscriptions WHERE id = $1";

        sqlx::query_as(q)
            .bind(id)
            .fetch_optional(pool)
            .await
            .map_err(DatabaseError::Sqlx)
    }

    async fn fetch_all<E>(pool: E, limit: i64, offset: i64) -> Result<Vec<Self>>
    where
        Self: Sized,
        E: 'q + Executor<'q, Database = Postgres>,
    {
        let limit = limit.clamp(1, 1000);
        let q = "SELECT * from subscriptions LIMIT $1 OFFSET $2";

        sqlx::query_as(q)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await
            .map_err(DatabaseError::Sqlx)
    }

    async fn total_count<E>(pool: E) -> Result<usize>
    where
        E: 'q + Executor<'q, Database = Postgres>,
    {
        let q = "SELECT COUNT(*) FROM subscriptions";
        let result: i64 = sqlx::query_scalar(q).fetch_one(pool).await?;

        Ok(result as usize)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::Type)]
pub enum SubscriptionStatus {
    Active,
    Paused,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::Type)]
pub enum PaymentStatus {
    Pending,
    Success,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::Type)]
pub enum FailurePolicy {
    Pause,
    Retry,
    Skip,
}
