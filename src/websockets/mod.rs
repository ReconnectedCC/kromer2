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
use uuid::Uuid;

use types::common::{WebSocketSessionData, WebSocketSubscriptionType, WebSocketTokenData};

use crate::models::krist::websockets::{WebSocketEvent, WebSocketMessage, WebSocketMessageInner};

pub const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
pub const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);
pub const TOKEN_EXPIRATION: Duration = Duration::from_secs(30);

type BroadcastFuture = std::pin::Pin<
    Box<dyn std::future::Future<Output = (Uuid, Result<(), actix_ws::Closed>)> + Send>,
>;

#[derive(Clone)]
pub struct WebSocketServer {
    pub sessions: Arc<DashMap<Uuid, WebSocketSessionData>>,
    pub pending_tokens: Arc<DashMap<Uuid, WebSocketTokenData>>,
}

impl Default for WebSocketServer {
    fn default() -> Self {
        Self::new()
    }
}

impl WebSocketServer {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(DashMap::with_capacity(100)),
            pending_tokens: Arc::new(DashMap::with_capacity(50)),
        }
    }

    #[tracing::instrument(skip_all, fields(address = data.address))]
    pub fn insert_session(&self, uuid: Uuid, session: Session, data: WebSocketTokenData) {
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

        self.sessions.insert(uuid, session_data);
    }

    #[tracing::instrument(skip(self))]
    pub async fn cleanup_session(&self, uuid: &Uuid) {
        self.sessions.remove(uuid);

        tracing::info!("Cleaned session");
    }

    #[tracing::instrument(skip_all, fields(address = token_data.address))]
    pub async fn obtain_token(&self, token_data: WebSocketTokenData) -> Uuid {
        let uuid = Uuid::new_v4();

        tracing::debug!("Inserting token {uuid} into cache");
        let _ = self.pending_tokens.insert(uuid, token_data);

        let pending_tokens = self.pending_tokens.clone();
        actix_web::rt::spawn(async move {
            time::sleep(TOKEN_EXPIRATION).await;

            // I don't think that this if statement would ever fail? considering we literally just put the fucking token in the map, lol.
            if pending_tokens.remove(&uuid).is_some() {
                tracing::info!("Removed expired token {uuid}");
            }
        });

        uuid
    }

    pub async fn use_token(
        &self,
        uuid: &Uuid,
    ) -> Result<WebSocketTokenData, errors::WebSocketServerError> {
        tracing::debug!("Removing token from cache");

        let (_uuid, token) = self
            .pending_tokens
            .remove(uuid)
            .ok_or(WebSocketServerError::TokenNotFound)?;

        Ok(token)
    }

    #[tracing::instrument(skip_all, fields(event = ?event))]
    pub async fn subscribe_to_event(&self, uuid: &Uuid, event: WebSocketSubscriptionType) {
        if let Some(data) = self.sessions.get_mut(uuid) {
            tracing::info!("Session subscribed to event");
            data.subscriptions.insert(event);
        } else {
            tracing::info!("Tried to subscribe to event {event} but found a non-existent session");
        }
    }

    #[tracing::instrument(skip_all, fields(event = ?event))]
    pub async fn unsubscribe_from_event(&self, uuid: &Uuid, event: &WebSocketSubscriptionType) {
        if let Some(data) = self.sessions.get_mut(uuid) {
            tracing::info!("Session unsubscribed from event");
            data.subscriptions.remove(event);
        }
    }

    pub async fn get_subscription_list(&self, uuid: &Uuid) -> Vec<WebSocketSubscriptionType> {
        if let Some(data) = self.sessions.get(uuid) {
            let subscriptions: Vec<WebSocketSubscriptionType> =
                data.subscriptions.iter().map(|x| x.clone()).collect(); // not my fav piece of code but it works
            return subscriptions;
        }

        Vec::new()
    }

    /// Broadcast an event to all connected clients
    #[tracing::instrument(skip_all)]
    pub async fn broadcast_event(&self, event: WebSocketMessage) {
        let msg =
            serde_json::to_string(&event).expect("Failed to turn event message into a string");
        tracing::debug!("Broadcasting event: {msg}");

        let mut futures: FuturesUnordered<BroadcastFuture> = FuturesUnordered::new();
        let sessions = self.sessions.iter();

        for entry in sessions {
            let uuid = *entry.key();
            let client_data = entry.value();

            // TODO: Somehow make this prettier...
            if let WebSocketMessageInner::Event { ref event } = event.r#type {
                match event {
                    WebSocketEvent::Block { .. } => todo!(),
                    WebSocketEvent::Transaction { transaction } => {
                        let transaction_from = transaction.from.as_deref().unwrap_or_default();
                        if (!client_data.is_guest()
                            && (client_data.address == transaction.to
                                || client_data.address == transaction_from)
                            && client_data
                                .subscriptions
                                .contains(&WebSocketSubscriptionType::OwnTransactions))
                            || client_data
                                .subscriptions
                                .contains(&WebSocketSubscriptionType::Transactions)
                        {
                            let mut session = client_data.session.clone();
                            let msg = msg.clone();
                            futures.push(Box::pin(async move { (uuid, session.text(msg).await) }));
                        }
                    }
                    WebSocketEvent::Name { name } => {
                        if (!client_data.is_guest()
                            && (client_data.address == name.owner)
                            && client_data
                                .subscriptions
                                .contains(&WebSocketSubscriptionType::OwnNames))
                            || client_data
                                .subscriptions
                                .contains(&WebSocketSubscriptionType::Names)
                        {
                            let mut session = client_data.session.clone();
                            let msg = msg.clone();
                            futures.push(Box::pin(async move { (uuid, session.text(msg).await) }));
                        }
                    }
                }
            }
        }

        while let Some((uuid, result)) = futures.next().await {
            if result.is_err() {
                tracing::warn!("Got an unexpected closed session");
                self.cleanup_session(&uuid).await;
            }
        }
    }

    /// Broadcast a message to all connected clients
    #[tracing::instrument(skip_all)]
    pub async fn broadcast(&self, msg: impl Into<ByteString>) {
        let msg = msg.into();
        tracing::debug!("Sending msg: {msg}");

        let mut futures: FuturesUnordered<BroadcastFuture> = FuturesUnordered::new();

        for entry in self.sessions.iter() {
            let uuid = *entry.key();
            let mut session = entry.value().session.clone();

            let msg = msg.clone();

            futures.push(Box::pin(async move { (uuid, session.text(msg).await) }));
        }

        while let Some((uuid, result)) = futures.next().await {
            if result.is_err() {
                tracing::warn!("Got an unexpected closed session");
                self.cleanup_session(&uuid).await;
            }
        }
    }

    pub async fn fetch_session_data(&self, uuid: &Uuid) -> Option<WebSocketSessionData> {
        self.sessions
            .get(uuid)
            .map(|session| session.value().clone())
    }
}
