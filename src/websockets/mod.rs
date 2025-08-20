pub mod errors;
pub mod handler;
pub mod routes;
pub mod types;
pub mod utils;

use actix_web::rt::time;
use actix_ws::Session;
use bytestring::ByteString;
use dashmap::{DashMap, DashSet};
use errors::WebSocketServerError;
use futures_util::{StreamExt, stream::FuturesUnordered};
use std::{sync::Arc, time::Duration};
use tokio::sync::Mutex;
use uuid::Uuid;

use types::common::{WebSocketSessionData, WebSocketSubscriptionType, WebSocketTokenData};

use crate::models::krist::websockets::{WebSocketEvent, WebSocketMessage, WebSocketMessageInner};

pub const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
pub const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);
pub const TOKEN_EXPIRATION: Duration = Duration::from_secs(30);

#[derive(Clone)]
pub struct WebSocketServer {
    pub inner: Arc<Mutex<WebSocketServerInner>>,
}

#[derive(Clone)]
pub struct WebSocketServerInner {
    pub sessions: DashMap<Uuid, WebSocketSessionData>,
    pub pending_tokens: DashMap<Uuid, WebSocketTokenData>,
}

impl Default for WebSocketServer {
    fn default() -> Self {
        Self::new()
    }
}

impl WebSocketServer {
    pub fn new() -> Self {
        let inner = WebSocketServerInner {
            sessions: DashMap::with_capacity(100),
            pending_tokens: DashMap::with_capacity(50),
        };

        Self {
            inner: Arc::new(Mutex::new(inner)),
        }
    }

    #[tracing::instrument(skip_all, fields(address = data.address))]
    pub async fn insert_session(&self, uuid: Uuid, session: Session, data: WebSocketTokenData) {
        let subscriptions = DashSet::from_iter([
            WebSocketSubscriptionType::OwnTransactions,
            WebSocketSubscriptionType::Blocks,
        ]);

        tracing::debug!("Inserting new session into session map");
        let session_data = WebSocketSessionData {
            address: data.address,
            private_key: data.private_key,
            session,
            subscriptions,
        };

        self.inner.lock().await.sessions.insert(uuid, session_data);
    }

    pub async fn cleanup_session(&self, uuid: &Uuid) {
        tracing::info!("Cleaning up session {uuid}");
        self.inner.lock().await.sessions.remove(uuid);
    }

    #[tracing::instrument(skip_all, fields(address = token_data.address))]
    pub async fn obtain_token(&self, token_data: WebSocketTokenData) -> Uuid {
        let inner = self.inner.lock().await;
        let inner_clone = self.inner.clone();

        let uuid = Uuid::new_v4();

        tracing::debug!("Inserting token {uuid} into cache");
        let token_data = inner.pending_tokens.insert(uuid, token_data);
        drop(token_data); // Drop manually to ensure the hashmap bucket is no longer locked.

        actix_web::rt::spawn(async move {
            time::sleep(TOKEN_EXPIRATION).await;

            // I don't think that this if statement would ever fail? considering we literally just put the fucking token in the map, lol.
            let inner_mutex = inner_clone.lock().await;
            if let Some(_) = inner_mutex.pending_tokens.remove(&uuid) {
                tracing::info!("Removed expired token {uuid}");
            }
        });

        uuid
    }

    pub async fn use_token(
        &self,
        uuid: &Uuid,
    ) -> Result<WebSocketTokenData, errors::WebSocketServerError> {
        let inner = self.inner.lock().await;

        tracing::debug!("Removing token from cache");

        let (_uuid, token) = inner
            .pending_tokens
            .remove(uuid)
            .ok_or(WebSocketServerError::TokenNotFound)?;

        Ok(token)
    }

    #[tracing::instrument(skip_all, fields(event = ?event))]
    pub async fn subscribe_to_event(&self, uuid: &Uuid, event: WebSocketSubscriptionType) {
        let inner = self.inner.lock().await;

        let entry = inner.sessions.get_mut(uuid);
        if let Some(data) = entry {
            tracing::info!("Session subscribed to event");
            data.subscriptions.insert(event);
        } else {
            tracing::info!("Tried to subscribe to event {event} but found a non-existent session");
        }
    }

    #[tracing::instrument(skip_all, fields(event = ?event))]
    pub async fn unsubscribe_from_event(&self, uuid: &Uuid, event: &WebSocketSubscriptionType) {
        let inner = self.inner.lock().await;

        let entry = inner.sessions.get_mut(uuid);
        if let Some(data) = entry {
            tracing::info!("Session unsubscribed from event");
            data.subscriptions.remove(event);
        }
    }

    pub async fn get_subscription_list(&self, uuid: &Uuid) -> Vec<WebSocketSubscriptionType> {
        let inner = self.inner.lock().await;

        let entry = inner.sessions.get(uuid);
        if let Some(data) = entry {
            let subscriptions: Vec<WebSocketSubscriptionType> =
                data.subscriptions.iter().map(|x| x.clone()).collect(); // not my fav piece of code but it works
            return subscriptions;
        }

        Vec::new()
    }

    /// Broadcast an event to all connected clients
    pub async fn broadcast_event(&self, event: WebSocketMessage) {
        let msg =
            serde_json::to_string(&event).expect("Failed to turn event message into a string");
        tracing::debug!("Broadcasting event: {msg}");

        let inner = self.inner.lock().await;
        let sessions = inner.sessions.iter_mut();

        for mut session in sessions {
            let (uuid, client_data) = session.pair_mut();

            if let WebSocketMessageInner::Event { ref event } = event.r#type {
                match event {
                    WebSocketEvent::Block { .. } => todo!(),
                    WebSocketEvent::Transaction { transaction } => {
                        let mut subs = client_data.subscriptions.iter();
                        let transaction_from = transaction.from.clone().unwrap_or("".to_string());
                        if (!client_data.is_guest()
                            && (client_data.address == transaction.to
                                || client_data.address == transaction_from)
                            && subs.any(|t| t.eq(&WebSocketSubscriptionType::OwnTransactions)))
                            || subs.any(|t| t.eq(&WebSocketSubscriptionType::Transactions))
                        {
                            let result = client_data.session.text(msg.clone()).await;
                            if result.is_err() {
                                tracing::warn!("Got an unexpected closed session");

                                self.cleanup_session(uuid).await;
                            }
                        }
                    }
                    WebSocketEvent::Name { name } => {
                        let mut subs = client_data.subscriptions.iter();
                        if !client_data.is_guest()
                            && (client_data.address == name.owner)
                            && subs.any(|t| t.eq(&WebSocketSubscriptionType::OwnNames))
                            || subs.any(|t| t.eq(&WebSocketSubscriptionType::Names))
                        {
                            let result = client_data.session.text(msg.clone()).await;
                            if result.is_err() {
                                tracing::warn!("Got an unexpected closed session");

                                self.cleanup_session(uuid).await;
                            }
                        }
                    }
                }
            }
        }
    }

    /// Broadcast a message to all connected clients
    pub async fn broadcast(&self, msg: impl Into<ByteString>) {
        let msg = msg.into();
        tracing::debug!("Sending msg: {msg}");

        let inner = self.inner.lock().await;
        let mut futures = FuturesUnordered::new();

        for mut entry in inner.sessions.iter_mut() {
            let msg = msg.clone();

            futures.push(async move {
                let (uuid, data) = entry.pair_mut();
                let res = data.session.text(msg).await;
                if res.is_err() {
                    tracing::warn!("Got an unexpected closed session");

                    self.cleanup_session(uuid).await;
                }
            });
        }

        while let Some(_result) = futures.next().await {}
    }

    pub async fn fetch_session_data(&self, uuid: &Uuid) -> Option<WebSocketSessionData> {
        let inner = self.inner.lock().await;

        inner
            .sessions
            .get(uuid)
            .map(|session| session.value().clone())
    }
}
