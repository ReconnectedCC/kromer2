use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::{Encode, Executor, Postgres, prelude::Type};

use super::SubscriptionStatus;
use crate::database::{DatabaseError, ModelExt, Result};

#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct Model {
    pub id: i64,
    pub payee_wallet_id: i32,

    pub amount: Decimal,

    pub cron_expr: String,

    pub status: SubscriptionStatus,
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
        let q = "SELECT * FROM contracts WHERE id = $1";

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
        let q = "SELECT * from contracts LIMIT $1 OFFSET $2";

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
        let q = "SELECT COUNT(*) FROM contracts";
        let result: i64 = sqlx::query_scalar(q).fetch_one(pool).await?;

        Ok(result as usize)
    }
}

#[derive(Debug, Clone, PartialEq, sqlx::Type)]
pub enum ContractStatus {
    Proposed,
    Active,
    Rejected,
    Superseded,
}
