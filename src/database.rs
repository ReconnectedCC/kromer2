pub mod name;
pub mod paginated;
pub mod player;
pub mod transaction;
pub mod wallet;

use sqlx::{Encode, Executor, Postgres, prelude::Type};

use crate::errors::KromerError;
use crate::errors::krist::KristError;
use crate::errors::krist::generic::GenericError;
use crate::errors::name::NameError;
use crate::errors::player::PlayerError;
use crate::errors::transaction::TransactionError;
use crate::errors::wallet::WalletError;

pub type Result<T, E = DatabaseError> = std::result::Result<T, E>;

#[async_trait::async_trait]
pub trait ModelExt<'q> {
    /// Fetches a record from a table and returns it
    async fn fetch_by_id<T, E>(pool: E, id: T) -> Result<Option<Self>>
    where
        Self: Sized,
        T: 'q + Encode<'q, Postgres> + Type<Postgres> + Send,
        E: 'q + Executor<'q, Database = Postgres>;

    /// Fetches all records from a table and returns them
    async fn fetch_all<E>(pool: E, limit: i64, offset: i64) -> Result<Vec<Self>>
    where
        Self: Sized,
        E: 'q + Executor<'q, Database = Postgres>;

    /// Fetches the total number of records in the table
    async fn total_count<E>(pool: E) -> Result<usize>
    where
        E: 'q + Executor<'q, Database = Postgres>;
}

#[derive(Debug, thiserror::Error)]
pub enum DatabaseError {
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),

    #[error(transparent)]
    Name(#[from] NameError),

    #[error(transparent)]
    Player(#[from] PlayerError),

    #[error(transparent)]
    Transaction(#[from] TransactionError),

    #[error(transparent)]
    Wallet(#[from] WalletError),

    #[error(transparent)]
    Generic(#[from] GenericError),
}

impl From<DatabaseError> for KromerError {
    fn from(value: DatabaseError) -> Self {
        match value {
            DatabaseError::Sqlx(error) => KromerError::Database(error),
            DatabaseError::Name(error) => KromerError::Name(error),
            DatabaseError::Player(error) => KromerError::Player(error),
            DatabaseError::Transaction(error) => KromerError::Transaction(error),
            DatabaseError::Wallet(error) => KromerError::Wallet(error),
            DatabaseError::Generic(error) => KromerError::Validation(error.to_string()), // nyehehehe
        }
    }
}

impl From<DatabaseError> for KristError {
    fn from(value: DatabaseError) -> Self {
        match value {
            DatabaseError::Sqlx(error) => KristError::Database(error),
            DatabaseError::Name(error) => KristError::Name(error.into()),
            DatabaseError::Player(_) => unreachable!(
                "This should not be reachable in our use cases, if it does sov fucked up"
            ),
            DatabaseError::Transaction(error) => KristError::Transaction(error.into()),
            DatabaseError::Wallet(error) => KristError::Address(error.into()),
            DatabaseError::Generic(error) => KristError::Generic(error),
        }
    }
}
