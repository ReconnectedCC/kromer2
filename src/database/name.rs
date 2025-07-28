use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::{Decimal, dec};
use sqlx::{Acquire, Encode, Executor, Pool, Postgres, Type};

use crate::database::transaction::Model as Transaction;
use crate::database::transaction::{TransactionCreateData, TransactionType};
use crate::database::wallet::Model as Wallet;
use crate::database::{DatabaseError, Result};

use crate::errors::name::NameError;
use crate::errors::wallet::WalletError;
use crate::models::krist::websockets::{WebSocketEvent, WebSocketMessage};
use crate::websockets::WebSocketServer;
use crate::{
    database::ModelExt, errors::krist::generic::GenericError,
    models::krist::names::NameDataUpdateBody, routes::PaginationParams, utils::validation,
};

use crate::database::paginated::PaginatedResult;

#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct Model {
    pub id: i32,
    pub last_transfered: Option<DateTime<Utc>>,
    pub last_updated: Option<DateTime<Utc>>,
    pub name: String,
    pub owner: String,
    pub original_owner: String,
    pub time_registered: DateTime<Utc>,
    pub unpaid: Decimal,
    pub metadata: Option<String>,
}

#[async_trait]
impl<'q> ModelExt<'q> for Model {
    async fn fetch_by_id<T, E>(pool: E, id: T) -> Result<Option<Self>>
    where
        Self: Sized,
        T: 'q + Encode<'q, Postgres> + Type<Postgres> + Send,
        E: 'q + Executor<'q, Database = Postgres>,
    {
        let q = "SELECT * FROM names WHERE id = $1";

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
        let q = "SELECT * from names LIMIT $1 OFFSET $2";

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
        let q = "SELECT COUNT(*) FROM names";
        let result: i64 = sqlx::query_scalar(q).fetch_one(pool).await?;

        Ok(result as usize)
    }
}

impl<'q> Model {
    /// Get name from its name field
    pub async fn fetch_by_name<S, E>(pool: E, name: S) -> Result<Option<Model>>
    where
        S: AsRef<str>,
        E: 'q + Executor<'q, Database = Postgres>,
    {
        let name = name.as_ref();
        let q = "SELECT * FROM names WHERE name = $1;";

        sqlx::query_as(q)
            .bind(name)
            .fetch_optional(pool)
            .await
            .map_err(DatabaseError::Sqlx)
    }

    pub async fn all_unpaid<E>(pool: E, pagination: &PaginationParams) -> Result<Vec<Model>>
    where
        E: 'q + Executor<'q, Database = Postgres>,
    {
        let limit = pagination.limit.unwrap_or(50);
        let offset = pagination.offset.unwrap_or(0);
        let limit = limit.clamp(1, 1000);

        let q = "SELECT * FROM names WHERE unpaid > 0 LIMIT $1 OFFSET $2";

        sqlx::query_as(q)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await
            .map_err(DatabaseError::Sqlx)
    }

    pub async fn count_unpaid<E>(pool: E) -> sqlx::Result<i64>
    where
        E: 'q + Executor<'q, Database = Postgres>,
    {
        let q = "SELECT count(*) FROM names WHERE unpaid > 0";

        sqlx::query_scalar(q).fetch_one(pool).await
    }

    pub async fn create<E>(pool: E, name: String, owner: String) -> Result<Model>
    where
        E: 'q + Executor<'q, Database = Postgres>,
    {
        let q = "INSERT INTO names(name, owner, original_owner, time_registered) VALUES ($1, $2, $2, NOW()) RETURNING *";

        sqlx::query_as(q)
            .bind(name)
            .bind(owner)
            .fetch_one(pool)
            .await
            .map_err(DatabaseError::Sqlx)
    }

    pub async fn update_metadata<E, S>(pool: E, name: S, metadata: String) -> Result<Model>
    where
        S: AsRef<str>,
        E: 'q + Executor<'q, Database = Postgres>,
    {
        let name = name.as_ref();

        let q = "UPDATE names SET metadata = $2 WHERE name = $1 RETURNING *";

        sqlx::query_as(q)
            .bind(name)
            .bind(metadata)
            .fetch_one(pool)
            .await
            .map_err(DatabaseError::Sqlx)
    }

    pub async fn ctrl_update_metadata<S: AsRef<str>>(
        pool: &Pool<Postgres>,
        name: S,
        body: NameDataUpdateBody,
    ) -> Result<Model> {
        let name = name.as_ref();

        let metadata_record = match body.a {
            Some(metadata_record) => metadata_record,
            None => {
                return Err(DatabaseError::Generic(GenericError::InvalidParameter(
                    "name".to_owned(),
                )));
            }
        };

        if !validation::is_valid_name(&name, false) {
            return Err(DatabaseError::Generic(GenericError::InvalidParameter(
                "name".to_owned(),
            )));
        }

        if !validation::is_valid_a_record(&metadata_record) {
            return Err(DatabaseError::Generic(GenericError::InvalidParameter(
                "a".to_owned(),
            )));
        }

        let name = name.trim().to_lowercase();
        let wallet = Wallet::verify_address(pool, body.private_key).await?;
        if !wallet.authed {
            tracing::info!("Auth failed on name update");
            return Err(DatabaseError::Wallet(WalletError::AuthFailed));
        }

        let model = Model::fetch_by_name(pool, &name)
            .await?
            .ok_or_else(|| DatabaseError::Name(NameError::NameNotFound(name.clone())))?;
        if model.owner != wallet.model.address {
            return Err(DatabaseError::Name(NameError::NotNameOwner(name)));
        }

        if model.metadata == Some(metadata_record.clone()) {
            return Ok(model);
        }

        let updated_model = Self::update_metadata(pool, &name, metadata_record).await?;

        Ok(updated_model)
    }

    /// Fetches the owner of the wallet and returns its database model.
    pub async fn owner<A>(&self, conn: A) -> Result<Option<Wallet>>
    where
        A: Acquire<'q, Database = Postgres>,
    {
        let mut tx = conn.begin().await?;

        let owner = Wallet::fetch_by_address(&mut *tx, &self.owner).await?;

        tx.commit().await?;

        Ok(owner)
    }

    /// Transfer ownership to a new wallet
    pub async fn transfer_ownership<A>(
        self,
        conn: A,
        server: &WebSocketServer,
        new_owner_address: String,
    ) -> Result<Model>
    where
        A: Acquire<'q, Database = Postgres>,
    {
        let mut tx = conn.begin().await?;
        let q = "UPDATE names SET owner = $2, last_updated = NOW(), last_transfered = NOW() WHERE owner = $1 RETURNING *";

        let updated_name: Model = sqlx::query_as(q)
            .bind(&self.owner)
            .bind(&new_owner_address)
            .fetch_one(&mut *tx)
            .await?;

        let creation_data = TransactionCreateData {
            from: self.owner,
            to: new_owner_address,
            amount: dec!(0),
            name: Some(self.name),
            transaction_type: TransactionType::NameTransfer,
            ..Default::default()
        };

        let transaction = Transaction::create(&mut *tx, creation_data).await?;
        let event = WebSocketMessage::new_event(WebSocketEvent::Transaction {
            transaction: transaction.into(),
        });
        server.broadcast_event(event).await;

        tx.commit().await?;

        Ok(updated_name)
    }

    pub async fn lookup_names(
        pool: &Pool<Postgres>,
        address_list: Option<Vec<String>>,
        limit: i64,
        offset: i64,
        order_by: &str,
        order: &str,
    ) -> sqlx::Result<PaginatedResult<Model>> {
        // Validate order_by to prevent SQL Injections
        let valid_columns = [
            "name",
            "owner",
            "registered",
            "transferred",
            "transferredOrRegistered",
        ];
        if !valid_columns.contains(&order_by) {
            return Err(sqlx::Error::Protocol("Invalid order_by column".into()));
        }

        // Validate order direction
        let order = match order.to_uppercase().as_str() {
            "ASC" | "DESC" => order.to_uppercase(),
            _ => return Err(sqlx::Error::Protocol("Invalid order direction".into())),
        };

        // Build the ORDER BY clause
        let order_clause = match order_by {
            "transferredOrRegistered" => {
                format!("COALESCE(last_transfered, time_registered) {order}")
            }
            "registered" => format!("time_registered {order}"),
            "transferred" => format!("last_transfered {order}"),
            other => format!("{other} {order}"),
        };

        // Count query
        let count_sql = r#"
            SELECT COUNT(*) as total
            FROM names
            WHERE ($1::text[] IS NULL or owner = ANY($1::text[]))
            "#;

        // Main query
        let main_query = format!(
            r#"
            SELECT *
            FROM names
            WHERE ($1::text[] IS NULL OR owner = ANY($1::text[]))
            ORDER BY {}
            LIMIT $2 OFFSET $3;
            "#,
            order_clause
        );

        // Execute count query
        let total: i64 = sqlx::query_scalar(count_sql)
            .bind(&address_list)
            .fetch_one(pool)
            .await?;

        // Execute main query
        let rows: Vec<Model> = sqlx::query_as::<_, Model>(&main_query)
            .bind(&address_list)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?;

        Ok(PaginatedResult::new(rows, total))
    }
}
