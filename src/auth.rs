//! Stupid simple auth for Kromer V1 endpoints.

use actix_web_httpauth::extractors::bearer::BearerAuth;
use chrono::{DateTime, TimeDelta, Utc};
use dashmap::DashMap;
use sqlx::PgPool;
use utoipa::openapi::security::{Http, SecurityScheme};
use uuid::Uuid;

use crate::{AppState, database::wallet::Model as Wallet, errors::auth::AuthError};

#[derive(Debug, Clone, Default)]
pub struct AuthSessions {
    sessions: DashMap<Uuid, (String, DateTime<Utc>)>,
}

impl AuthSessions {
    /// Removes expired sessions.
    pub fn vacuum(&self) {
        let current_time = Utc::now();

        self.sessions.retain(|_, (_, exp)| *exp <= current_time);
    }

    /// Registers a new session. `addr` is expected to be a valid address.
    pub fn register(&self, addr: String) -> (Uuid, DateTime<Utc>) {
        let id = Uuid::new_v4();
        let exp = Utc::now() + TimeDelta::hours(1);

        self.sessions.insert(id, (addr, exp));

        (id, exp)
    }

    /// Registers a new session given a private key
    pub async fn register_pk(
        &self,
        db: &PgPool,
        pk: &str,
    ) -> Result<(Uuid, DateTime<Utc>, String), AuthError> {
        let res = Wallet::verify_address(db, pk)
            .await
            .map_err(|_| AuthError::InvalidSession)?;

        if res.authed {
            let (id, exp) = self.register(res.model.address.clone());
            Ok((id, exp, res.model.address))
        } else {
            Err(AuthError::InvalidSession)
        }
    }

    /// Revokes an active session, returning the address if there was one.
    pub fn revoke(&self, id: Uuid) -> Result<String, AuthError> {
        self.sessions
            .remove(&id)
            .map(|(_, (addr, _))| addr)
            .ok_or(AuthError::InvalidSession)
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

    pub fn session_exists(&self, id: Uuid) -> bool {
        let Some(res) = self.sessions.get(&id) else {
            return false;
        };

        // Cleanup expired sessions
        if res.value().1 <= Utc::now() {
            self.sessions.remove(&id);

            false
        } else {
            true
        }
    }
}

pub async fn check_bearer(state: &AppState, cred: Option<BearerAuth>) -> Result<Uuid, AuthError> {
    let Some(cred) = cred else {
        return Err(AuthError::MissingBearer);
    };

    let id = Uuid::try_parse(cred.token()).map_err(|_| AuthError::InvalidSession)?;

    if !state.auth.session_exists(id) {
        Err(AuthError::InvalidSession)
    } else {
        Ok(id)
    }
}

pub struct AuthAddon;

impl utoipa::Modify for AuthAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi.components.as_mut().unwrap();
        components.add_security_scheme(
            "bearerAuth",
            SecurityScheme::Http(Http::new(utoipa::openapi::security::HttpAuthScheme::Bearer)),
        );
    }
}
