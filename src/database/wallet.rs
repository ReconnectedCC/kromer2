use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::{Decimal, dec};
use sqlx::{Acquire, Encode, Executor, Postgres, Type};

use crate::database::{DatabaseError, ModelExt, Result, name, transaction};
use crate::errors::KromerError;
use crate::routes::PaginationParams;
use crate::utils::crypto;

#[derive(Debug, Clone, PartialEq, sqlx::FromRow, serde::Serialize)]
pub struct Model {
    pub id: i32,
    pub address: String,
    pub balance: Decimal,
    pub created_at: DateTime<Utc>,
    pub locked: bool,
    pub total_in: Decimal,
    pub total_out: Decimal,
    pub private_key: Option<String>,

    #[serde(skip)]
    #[sqlx(default)]
    pub names: Option<i64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VerifyResponse {
    pub authed: bool,
    pub model: Model,
}

#[async_trait]
impl<'q> ModelExt<'q> for Model {
    async fn fetch_by_id<T, E>(pool: E, id: T) -> Result<Option<Self>>
    where
        Self: Sized,
        T: 'q + Encode<'q, Postgres> + Type<Postgres> + Send,
        E: 'q + Executor<'q, Database = Postgres>,
    {
        let q = "SELECT * FROM wallets WHERE id = $1";

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
        let q = "SELECT * from wallets LIMIT $1 OFFSET $2";

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
        let q = "SELECT COUNT(*) FROM wallets";
        let result: i64 = sqlx::query_scalar(q).fetch_one(pool).await?;

        Ok(result as usize)
    }
}

impl<'q> Model {
    pub async fn fetch_by_address<S, E>(pool: E, address: S) -> Result<Option<Self>>
    where
        S: AsRef<str>,
        E: 'q + Executor<'q, Database = Postgres>,
    {
        let address = address.as_ref();

        let q = "SELECT * FROM wallets WHERE address = $1;";
        sqlx::query_as(q)
            .bind(address)
            .fetch_optional(pool)
            .await
            .map_err(DatabaseError::Sqlx)
    }

    pub async fn fetch_richest<E>(pool: E, limit: i64, offset: i64) -> Result<Vec<Self>>
    where
        E: 'q + Executor<'q, Database = Postgres>,
    {
        let limit = limit.clamp(1, 1000);

        let q = "SELECT * FROM wallets ORDER BY balance DESC LIMIT $1 OFFSET $2;";
        sqlx::query_as(q)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await
            .map_err(DatabaseError::Sqlx)
    }

    #[tracing::instrument(skip(pool))]
    pub async fn verify_address<A, S>(pool: A, private_key: S) -> Result<VerifyResponse>
    where
        S: AsRef<str> + std::fmt::Debug,
        A: 'q + Acquire<'q, Database = Postgres>,
    {
        let private_key = private_key.as_ref();
        let mut tx = pool.acquire().await?;

        let address = crypto::make_v2_address(private_key, "k");
        let guh = format!("{address}{private_key}");

        tracing::info!("Authentication attempt on address {address}");

        let result = Model::fetch_by_address(&mut *tx, &address).await?;
        let hash = crypto::sha256(&guh);

        let wallet = match result {
            Some(w) => w,
            None => Self::create_wallet(&mut *tx, &address, &hash, None).await?,
        };
        let pkey = &wallet.private_key;

        let authed = *pkey == Some(hash);
        if !authed {
            tracing::info!("Someone tried to login to an address they do not own");
        }

        return Ok(VerifyResponse {
            authed,
            model: wallet,
        });
    }

    pub async fn create_wallet<E>(
        pool: E,
        address: &str,
        hash: &str,
        initial_balance: Option<Decimal>,
    ) -> sqlx::Result<Model>
    where
        E: 'q + Executor<'q, Database = Postgres>,
    {
        let initial_balance = initial_balance.unwrap_or(dec!(0.0));

        // Pretty big query, lol
        let q = "INSERT INTO wallets(address, balance, created_at, private_key) VALUES ($1, $2, NOW(), $3) RETURNING *";

        sqlx::query_as(q)
            .bind(address)
            .bind(initial_balance)
            .bind(hash)
            .fetch_one(pool)
            .await
    }

    pub async fn total_transactions<E>(&self, executor: E, exclude_mined: bool) -> sqlx::Result<i64>
    where
        E: 'q + Executor<'q, Database = Postgres>,
    {
        let q = match exclude_mined {
            true => {
                r#"SELECT COUNT(*) FROM transactions WHERE ("from" = $1 OR "to" = $1) AND transaction_type != 'mined';"#
            }
            false => r#"SELECT COUNT(*) FROM transactions  WHERE "from" = $1 OR "to" = $1;"#,
        };

        sqlx::query_scalar(q)
            .bind(&self.address)
            .fetch_one(executor)
            .await
    }

    pub async fn transactions<E>(
        &self,
        pool: E,
        query: &PaginationParams,
    ) -> sqlx::Result<Vec<transaction::Model>>
    where
        E: 'q + Executor<'q, Database = Postgres>,
    {
        let limit = query.limit.unwrap_or(50).clamp(1, 1000);
        let offset = query.offset.unwrap_or(0);

        let q = match query.exclude_mined {
            Some(true) => {
                r#"SELECT * FROM transactions WHERE ("from" = $1 OR "to" = $1) AND transaction_type != 'mined' ORDER BY date DESC LIMIT $2 OFFSET $3;"#
            }
            _ => {
                r#"SELECT * FROM transactions WHERE "from" = $1 OR "to" = $1 ORDER BY date DESC LIMIT $2 OFFSET $3;"#
            }
        };

        sqlx::query_as(q)
            .bind(&self.address)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await
    }

    pub async fn names<E>(
        &self,
        pool: E,
        query: &PaginationParams,
    ) -> sqlx::Result<Vec<name::Model>>
    where
        E: 'q + Executor<'q, Database = Postgres>,
    {
        let limit = query.limit.unwrap_or(50);
        let offset = query.offset.unwrap_or(0);
        let limit = limit.clamp(1, 1000);

        let q = r#"SELECT * FROM names WHERE owner = $1 ORDER BY name ASC LIMIT $2 OFFSET $3;"#;
        sqlx::query_as(q)
            .bind(&self.address)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await
    }

    pub async fn set_balance<E>(&self, executor: E, balance: Decimal) -> Result<Model, KromerError>
    where
        E: 'q + Executor<'q, Database = Postgres>,
    {
        let q = "UPDATE wallets SET balance = $1 WHERE address = $2 RETURNING *";

        sqlx::query_as(q)
            .bind(balance)
            .bind(&self.address)
            .fetch_one(executor)
            .await
            .map_err(KromerError::Database)
    }

    pub async fn update_balance<E>(&self, executor: E, balance: Decimal) -> sqlx::Result<Model>
    where
        E: 'q + Executor<'q, Database = Postgres>,
    {
        let q = r#"
        UPDATE wallets
        SET
            balance = balance + $1,
            total_in = total_in + CASE WHEN $1 > 0 THEN $1 ELSE 0 END,
            total_out = abs(total_out) + CASE WHEN $1 < 0 THEN abs($1) ELSE 0 END
        WHERE address = $2
        RETURNING *;
        "#;

        sqlx::query_as(q)
            .bind(balance)
            .bind(&self.address)
            .fetch_one(executor)
            .await
    }

    pub async fn lookup_addresses<A>(
        conn: A,
        addresses: Vec<&str>,
        fetch_names: bool,
    ) -> sqlx::Result<Vec<Model>>
    where
        A: 'q + Acquire<'q, Database = Postgres>,
    {
        let mut conn = conn.acquire().await?;

        let query = match fetch_names {
            true => {
                r#"SELECT wallets.*,
                       COUNT(names.id) AS NAMES
                FROM wallets
                LEFT JOIN NAMES ON wallets.address = names.owner
                WHERE wallets.address = ANY($1) GROUP BY wallets.id
                  ORDER  BY NAMES DESC
                "#
            }
            false => {
                r#"SELECT
                    wallets.*,
                    NULL::BIGINT AS names
                FROM wallets
                WHERE wallets.address = ANY($1)
                "#
            }
        };
        let models = sqlx::query_as(query)
            .bind(addresses)
            .fetch_all(&mut *conn)
            .await?;

        Ok(models)
    }

    pub async fn fetch_by_address_names<S, E>(pool: E, address: S) -> Result<Option<Self>>
    where
        S: AsRef<str>,
        E: 'q + Executor<'q, Database = Postgres>,
    {
        let address = address.as_ref();

        let q = r#"SELECT wallets.*,
                COUNT(names.id) AS NAMES
            FROM wallets
            LEFT JOIN NAMES ON wallets.address = names.owner
            WHERE wallets.address = $1 GROUP BY wallets.id
            ORDER BY NAMES DESC"#;
        sqlx::query_as(q)
            .bind(address)
            .fetch_optional(pool)
            .await
            .map_err(DatabaseError::Sqlx)
    }
}
