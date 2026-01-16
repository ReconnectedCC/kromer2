use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::{Encode, Executor, Postgres, prelude::Type};

use super::contracts::ContractStatus;
use crate::database::{DatabaseError, ModelExt, Result};

#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct Model {
    pub id: i64,
    pub offer_id: i64,

    pub proposed_amount: Decimal,
    pub proposed_cron_expr: Option<String>,

    pub payer_accepted: bool,
    pub payer_accepted_at: Option<DateTime<Utc>>,

    pub status: ContractStatus,
    pub applied_at: Option<DateTime<Utc>>,

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
        let q = "SELECT * FROM offer_proposals WHERE id = $1";

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
        let q = "SELECT * from offer_proposals LIMIT $1 OFFSET $2";

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
        let q = "SELECT COUNT(*) FROM offer_proposals";
        let result: i64 = sqlx::query_scalar(q).fetch_one(pool).await?;

        Ok(result as usize)
    }
}
