use async_trait::async_trait;
use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use regex::Regex;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{Acquire, Encode, Executor, Pool, Postgres, Type};

use crate::database::{DatabaseError, Result};
use crate::{database::ModelExt, routes::PaginationParams};

use crate::database::wallet::Model as Wallet;
use crate::errors::wallet::WalletError;

static KRO_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(?:([a-z0-9-_]{1,32})@)?([a-z0-9]{1,64})\.kro").unwrap());

#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct Model {
    pub id: i32,
    pub amount: Decimal,
    pub from: Option<String>,
    pub to: String,
    pub metadata: Option<String>,
    pub name: Option<String>,
    pub sent_metaname: Option<String>,
    pub sent_name: Option<String>,
    pub transaction_type: TransactionType,
    pub date: DateTime<Utc>,
}

#[derive(Debug, Default, Clone, PartialEq, PartialOrd, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "transaction_type", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum TransactionType {
    #[default]
    Mined,
    Unknown,
    NamePurchase,
    NameARecord,
    NameTransfer,
    Transfer,
}

#[derive(Clone, Debug, Default, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct TransactionCreateData {
    pub from: String,
    pub to: String,
    pub amount: Decimal,
    pub metadata: Option<String>,
    pub name: Option<String>,
    pub sent_metaname: Option<String>,
    pub sent_name: Option<String>,
    pub transaction_type: TransactionType,
}

#[derive(Debug, Default, Clone, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct TransactionNameData {
    pub name: Option<String>,
    pub metaname: Option<String>,
}

impl From<String> for TransactionType {
    fn from(value: String) -> Self {
        match value.as_str() {
            "mined" => TransactionType::Mined,
            "name_purchase" => TransactionType::NamePurchase,
            "name_a_record" => TransactionType::NameARecord,
            "name_transfer" => TransactionType::NameTransfer,
            "transfer" => TransactionType::Transfer,
            _ => TransactionType::Unknown,
        }
    }
}

impl From<TransactionType> for &str {
    fn from(value: TransactionType) -> Self {
        match value {
            TransactionType::Unknown => "unknown",
            TransactionType::Mined => "mined",
            TransactionType::NamePurchase => "name_purchase",
            TransactionType::NameARecord => "name_a_record",
            TransactionType::NameTransfer => "name_transfer",
            TransactionType::Transfer => "transfer",
        }
    }
}

#[async_trait]
impl<'q> ModelExt<'q> for Model {
    async fn fetch_by_id<T, E>(pool: E, id: T) -> Result<Option<Self>>
    where
        Self: Sized,
        T: 'q + Encode<'q, Postgres> + Type<Postgres> + Send,
        E: 'q + Executor<'q, Database = Postgres>,
    {
        let q = "SELECT * FROM transactions WHERE id = $1";

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
        let q = "SELECT * from transactions LIMIT $1 OFFSET $2";

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
        let q = "SELECT COUNT(*) FROM transactions";
        let result: i64 = sqlx::query_scalar(q).fetch_one(pool).await?;

        Ok(result as usize)
    }
}

impl<'q> Model {
    pub async fn sorted_by_date(
        pool: &Pool<Postgres>,
        pagination: &PaginationParams,
    ) -> Result<Vec<Model>> {
        let limit = pagination.limit.unwrap_or(50);
        let offset = pagination.offset.unwrap_or(0);
        let limit = limit.clamp(1, 1000);

        let q = match pagination.exclude_mined {
            Some(true) => {
                r#"SELECT * FROM transactions WHERE transaction_type != 'mined' ORDER BY date DESC LIMIT $1 OFFSET $2;"#
            }
            _ => r#"SELECT * FROM transactions ORDER BY date DESC LIMIT $1 OFFSET $2;"#,
        };

        sqlx::query_as(q)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await
            .map_err(DatabaseError::Sqlx)
    }

    pub async fn create_no_update<E>(
        executor: E,
        creation_data: TransactionCreateData,
    ) -> Result<Model>
    where
        E: 'q + Executor<'q, Database = Postgres>,
    {
        let metadata = creation_data.metadata.unwrap_or_default();
        let q = r#"INSERT INTO transactions(amount, "from", "to", metadata, transaction_type, date, name) VALUES ($1, $2, $3, $4, $5, NOW(), $6) RETURNING *"#;

        sqlx::query_as(q)
            .bind(creation_data.amount)
            .bind(&creation_data.from)
            .bind(&creation_data.to)
            .bind(metadata)
            .bind(creation_data.transaction_type)
            .bind(&creation_data.name)
            .fetch_one(executor)
            .await
            .map_err(DatabaseError::Sqlx)
    }

    pub async fn create<A>(conn: A, creation_data: TransactionCreateData) -> Result<Model>
    where
        A: Acquire<'q, Database = Postgres>,
    {
        let metadata = creation_data.metadata.unwrap_or_default();

        let mut tx = conn.begin().await?;

        let sender = Wallet::fetch_by_address(&mut *tx, &creation_data.from)
            .await?
            .ok_or_else(|| {
                DatabaseError::Wallet(WalletError::NotFound(creation_data.from.clone()))
            })?;

        let recipient = Wallet::fetch_by_address(&mut *tx, &creation_data.to)
            .await?
            .ok_or_else(|| {
                DatabaseError::Wallet(WalletError::NotFound(creation_data.to.clone()))
            })?;

        let _ = sender
            .update_balance(&mut *tx, -creation_data.amount)
            .await?;
        let _ = recipient
            .update_balance(&mut *tx, creation_data.amount)
            .await?;

        let q = r#"INSERT INTO transactions(amount, "from", "to", metadata, transaction_type, date, name, sent_metaname, sent_name) VALUES ($1, $2, $3, $4, $5, NOW(), $6, $7, $8) RETURNING *"#;

        let model = sqlx::query_as(q)
            .bind(creation_data.amount)
            .bind(&creation_data.from)
            .bind(&creation_data.to)
            .bind(metadata)
            .bind(creation_data.transaction_type)
            .bind(creation_data.name)
            .bind(creation_data.sent_metaname)
            .bind(creation_data.sent_name)
            .fetch_one(&mut *tx)
            .await?;
        tx.commit().await?; // I'm not sure this is how it should be done? `Wallet::update_balance` also creates a transaction..

        Ok(model)
    }

    // Implemented both of the "no_mined" functions here rather than simply modifying the existing total count function because I
    // don't want to change an entire trait def
    pub async fn total_count_no_mined<E>(pool: E, params: &PaginationParams) -> Result<usize>
    where
        E: 'q + Executor<'q, Database = Postgres>,
    {
        let q = match params.exclude_mined {
            Some(true) => r#"SELECT COUNT(*) FROM transactions WHERE transaction_type != 'mined'"#,
            _ => r#"SELECT COUNT(*) FROM transactions"#,
        };
        let result: i64 = sqlx::query_scalar(q).fetch_one(pool).await?;

        Ok(result as usize)
    }

    pub async fn fetch_all_no_mined<E>(pool: E, params: &PaginationParams) -> Result<Vec<Self>>
    where
        Self: Sized,
        E: 'q + Executor<'q, Database = Postgres>,
    {
        let limit = params.limit.unwrap_or(50).clamp(1, 1000);
        let offset = params.offset.unwrap_or(0);

        let q = match params.exclude_mined {
            Some(true) => {
                r#"SELECT * from transactions WHERE transaction_type != 'mined' LIMIT $1 OFFSET $2"#
            }
            _ => r#"SELECT * from transactions LIMIT $1 OFFSET $2"#,
        };

        sqlx::query_as(q)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await
            .map_err(DatabaseError::Sqlx)
    }

    pub async fn fetch_name_history(
        pool: &Pool<Postgres>,
        name: &str,
        limit: i64,
        offset: i64,
        order_by: &str,
        order: &str,
    ) -> Result<(Vec<Model>, usize)> {
        let limit = limit.clamp(1, 1000);

        // Validate order_by parameter against allowed fields
        let order_by = match order_by {
            "id" | "from" | "to" | "value" | "time" | "sent_name" | "sent_metaname" => {
                // Map "time" to "date" since that's our actual column name
                if order_by == "time" { "date" } else { order_by }
            }
            _ => "id",
        };

        let order = match order.to_uppercase().as_str() {
            "ASC" | "DESC" => order.to_uppercase(),
            _ => "ASC".to_string(),
        };

        // Count query - only name-related transactions
        let count_query = r#"
            SELECT COUNT(*) as total
            FROM transactions
            WHERE (name = $1 OR sent_name = $1)
            AND transaction_type IN ('name_purchase', 'name_a_record', 'name_transfer')
        "#;

        let total: i64 = sqlx::query_scalar(count_query)
            .bind(name)
            .fetch_one(pool)
            .await?;

        // Main query - only name-related transactions
        let query = format!(
            r#"
            SELECT *
            FROM transactions
            WHERE (name = $1 OR sent_name = $1)
            AND transaction_type IN ('name_purchase', 'name_a_record', 'name_transfer')
            ORDER BY {} {}
            LIMIT $2 OFFSET $3
        "#,
            order_by, order
        );

        let rows = sqlx::query_as(&query)
            .bind(name)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?;

        Ok((rows, total as usize))
    }

    pub async fn fetch_by_sent_name(
        pool: &Pool<Postgres>,
        sent_name: &str,
        limit: i64,
        offset: i64,
        order_by: &str,
        order: &str,
    ) -> Result<(Vec<Model>, usize)> {
        let limit = limit.clamp(1, 1000);

        // Validate order_by and order parameters
        let order_by = match order_by {
            "id" | "amount" | "date" | "from" | "to" => order_by,
            _ => "id",
        };
        let order = match order.to_uppercase().as_str() {
            "ASC" | "DESC" => order.to_uppercase(),
            _ => "ASC".to_string(),
        };

        // Count query - match both name and sent_name columns
        let count_query = r#"
            SELECT COUNT(*) as total
            FROM transactions
            WHERE name = $1 OR sent_name = $1
        "#;

        let total: i64 = sqlx::query_scalar(count_query)
            .bind(sent_name)
            .fetch_one(pool)
            .await?;

        // Main query with dynamic ordering - match both name and sent_name columns
        let query = format!(
            r#"
            SELECT *
            FROM transactions
            WHERE name = $1 OR sent_name = $1
            ORDER BY {} {}
            LIMIT $2 OFFSET $3
            "#,
            order_by, order
        );

        let rows = sqlx::query_as(&query)
            .bind(sent_name)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?;

        Ok((rows, total as usize))
    }

    pub async fn lookup_transactions(
        pool: &Pool<Postgres>,
        address_list: Option<Vec<String>>,
        limit: i64,
        offset: i64,
        order_by: &str,
        order: &str,
        include_mined: bool,
    ) -> Result<(
        Vec<crate::models::krist::transactions::TransactionJson>,
        usize,
    )> {
        use crate::models::krist::transactions::TransactionJson;

        let limit = limit.clamp(1, 1000);

        // Validate order_by parameter against allowed fields
        let order_by = match order_by {
            "id" | "from" | "to" | "amount" | "date" | "sent_name" | "sent_metaname" => {
                // Map "time" to "date" since that's our actual column name
                if order_by == "time" { "date" } else { order_by }
            }
            _ => "id",
        };

        let order = match order.to_uppercase().as_str() {
            "ASC" | "DESC" => order.to_uppercase(),
            _ => "ASC".to_string(),
        };

        if let Some(addresses) = address_list {
            // Build query for all addresses at once
            let mut query_conditions = Vec::new();
            let mut bind_values = Vec::new();

            // Create OR conditions for each address
            for (i, address) in addresses.iter().enumerate() {
                let param_num = i + 1;
                query_conditions.push(format!(
                    "(\"from\" = ${} OR \"to\" = ${})",
                    param_num, param_num
                ));
                bind_values.push(address.as_str());
            }

            let address_condition = format!("({})", query_conditions.join(" OR "));
            let mut where_conditions = vec![address_condition];

            // Add mined filter if includeMined is false
            if !include_mined {
                where_conditions.push("transaction_type != 'mined'".to_string());
            }

            let where_clause = where_conditions.join(" AND ");
            let limit_param = bind_values.len() + 1;
            let offset_param = bind_values.len() + 2;

            let query = format!(
                r#"
                SELECT *
                FROM transactions
                WHERE {}
                ORDER BY {} {}
                LIMIT {} OFFSET {}
                "#,
                where_clause, order_by, order, limit_param, offset_param
            );

            let mut query_builder = sqlx::query_as(&query);
            for address in bind_values {
                query_builder = query_builder.bind(address);
            }
            query_builder = query_builder.bind(limit).bind(offset);

            let transactions: Vec<Model> = query_builder.fetch_all(pool).await?;

            // Count total matching transactions
            let count_query = format!(
                r#"
                SELECT COUNT(*) as total
                FROM transactions
                WHERE {}
                "#,
                where_clause
            );

            let mut count_query_builder = sqlx::query_scalar(&count_query);
            for address in addresses.iter() {
                count_query_builder = count_query_builder.bind(address.as_str());
            }
            let total: i64 = count_query_builder.fetch_one(pool).await?;

            let json_transactions: Vec<TransactionJson> =
                transactions.into_iter().map(|model| model.into()).collect();

            Ok((json_transactions, total as usize))
        } else {
            // No addresses specified, return empty result
            Ok((Vec::new(), 0))
        }
    }
}

impl TransactionNameData {
    /// Parse a transaction name from a string-like type according to CommonMeta format.
    /// Takes any type that can be converted to a string reference.
    ///
    /// If the input is empty, returns a default `TransactionNameData`.
    /// Otherwise parses according to the pattern: `meta@name.kro`
    ///
    /// # Examples
    /// ```
    /// use kromer::database::transaction::TransactionNameData;
    /// let data = TransactionNameData::parse("meta@name.kro");
    /// assert_eq!(data.metaname, Some("meta".to_string()));
    /// assert_eq!(data.name, Some("name".to_string()));
    ///
    /// let empty = TransactionNameData::parse("");
    /// assert_eq!(empty, TransactionNameData::default());
    /// ```
    pub fn parse<S: AsRef<str>>(input: S) -> Self {
        let input = input.as_ref();
        if input.is_empty() {
            return Self::default(); // Don't do useless parsing if the input is empty, thats silly.
        }

        match KRO_REGEX.captures(input) {
            Some(captures) => {
                let metaname = captures.get(1).map(|m| m.as_str().to_string()); // TODO: Less allocating, should maybe use `&str` on the transaction models
                let name = captures.get(2).map(|m| m.as_str().to_string());

                Self { metaname, name }
            }
            None => Self::default(),
        }
    }

    /// Parse a transaction name from an optional string-like type.
    /// If the input is `None`, returns a default `TransactionNameData`.
    /// Otherwise, parses the string according to CommonMeta format.
    ///
    /// # Examples
    /// ```
    /// use kromer::database::transaction::TransactionNameData;
    /// let data = TransactionNameData::parse_opt(Some("meta@name.kro"));
    /// assert_eq!(data.metaname, Some("meta".to_string()));
    /// assert_eq!(data.name, Some("name".to_string()));
    ///
    /// let empty = TransactionNameData::parse_opt(None::<String>);
    /// assert_eq!(empty, TransactionNameData::default());
    /// ```
    pub fn parse_opt<S: AsRef<str>>(input: Option<S>) -> Self {
        if input.is_none() {
            return Self::default(); // Do we really need to parse stuff is there is no value? No, we dont.
        }

        let input = input.unwrap(); // We can do this, we made sure it exists.
        Self::parse(input)
    }

    /// Parse a transaction name from a reference to an optional string-like type.
    /// If the input is `None`, returns a default `TransactionNameData`.
    /// Otherwise, parses the string according to CommonMeta format.
    ///
    /// # Examples
    /// ```
    /// use kromer::database::transaction::TransactionNameData;
    /// let input = Some("meta@name.kro".to_string());
    /// let data = TransactionNameData::parse_opt_ref(&input);
    /// assert_eq!(data.metaname, Some("meta".to_string()));
    /// assert_eq!(data.name, Some("name".to_string()));
    ///
    /// let empty = TransactionNameData::parse_opt_ref(&None::<String>);
    /// assert_eq!(empty, TransactionNameData::default());
    /// ```
    pub fn parse_opt_ref<S: AsRef<str>>(input: &Option<S>) -> Self {
        if input.is_none() {
            return Self::default(); // Do we really need to parse stuff is there is no value? No, we dont.
        }

        let input = input.as_ref().unwrap(); // We can do this, we made sure it exists.
        Self::parse(input)
    }

    /// Return the name as a string slice.
    #[inline(always)]
    pub fn name(&self) -> Option<&str> {
        let name_ref = self.name.as_ref();

        name_ref.map(|name| name.as_str())
    }

    /// Return the metaname as a string slice
    #[inline(always)]
    pub fn metaname(&self) -> Option<&str> {
        let metaname_ref = self.metaname.as_ref();

        metaname_ref.map(|metaname| metaname.as_str())
    }
}
