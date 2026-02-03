//! Stupid simple auth for Kromer V1 endpoints.

use chrono::{DateTime, TimeDelta, Utc};
use dashmap::DashMap;
use sqlx::PgPool;
use uuid::Uuid;

use crate::database::wallet::Model as Wallet;

#[derive(Debug, Clone, Default)]
pub struct AuthSessions {
    sessions: DashMap<Uuid, (String, DateTime<Utc>)>,
}

impl AuthSessions {
    /// Removes expired sessions.
    pub async fn vacuum(&self) {
        let current_time = Utc::now();

        self.sessions.retain(|_, (_, exp)| *exp <= current_time);
    }

    /// Registers a new session. `addr` is expected to be a valid address.
    pub fn register(&self, addr: String) -> (Uuid, DateTime<Utc>) {
        let id = Uuid::new_v4();
        let exp = Utc::now() + TimeDelta::minutes(5);

        self.sessions.insert(id, (addr, exp));

        (id, exp)
    }

    /// Registers a new
    pub async fn register_pk(&self, db: &PgPool, pk: &str) -> Option<(Uuid, DateTime<Utc>)> {
        let addr = Wallet::verify_address(db, pk)
            .await
            .ok()
            .filter(|x| x.authed)?
            .model
            .address;

        Some(self.register(addr))
    }

    /// Revokes an active session, returning the address if there was one.
    pub async fn revoke(&self, id: Uuid) -> Option<String> {
        self.sessions.remove(&id).map(|(_, (addr, _))| addr)
    }

    /// Checks if a given ID is authorized to operate on an address. Returns [None] if `id` is not
    /// an active session.
    pub fn is_authed_addr(&self, id: Uuid, addr: &str) -> Option<bool> {
        let val = self.sessions.get(&id)?;

        let (actual_addr, exp) = val.value();

        // Cleanup expired sessions
        if *exp <= Utc::now() {
            self.sessions.remove(&id);

            None
        } else {
            Some(addr == actual_addr)
        }
    }
}
