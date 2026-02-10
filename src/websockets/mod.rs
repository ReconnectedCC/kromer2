pub mod errors;
pub mod handler;
pub mod routes;
pub mod types;
pub mod utils;

use actix_web::rt::time;
use actix_ws::Session;
use bytestring::ByteString;
use errors::WebSocketServerError;
use futures_util::{StreamExt, stream::FuturesUnordered};
use scc::{HashMap, HashSet};
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
    pub sessions: Arc<HashMap<Uuid, WebSocketSessionData>>,
    pub pending_tokens: Arc<HashMap<Uuid, WebSocketTokenData>>,
}

impl Default for WebSocketServer {
    fn default() -> Self {
        Self::new()
    }
}

impl WebSocketServer {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(HashMap::with_capacity(100)),
            pending_tokens: Arc::new(HashMap::with_capacity(50)),
        }
    }

    #[tracing::instrument(skip_all, fields(address = data.address))]
    pub fn insert_session(&self, uuid: Uuid, session: Session, data: WebSocketTokenData) {
        let subscriptions = HashSet::from_iter([
            WebSocketSubscriptionType::OwnTransactions,
            WebSocketSubscriptionType::Blocks,
        ]);

        tracing::debug!("Inserting new session into session map");
        let session_data = WebSocketSessionData {
            address: data.address,
            private_key: data.private_key,
            session,
            subscriptions,
            computer_id: data.computer_id,
        };

        if self.sessions.insert_sync(uuid, session_data).is_err() {
            tracing::error!("Attempted to insert session that already exists");
        }
    }

    #[tracing::instrument(skip(self))]
    pub fn cleanup_session(&self, uuid: &Uuid) {
        self.sessions.remove_sync(uuid);

        tracing::info!("Cleaned session");
    }

    #[tracing::instrument(skip_all, fields(address = token_data.address))]
    pub fn obtain_token(&self, mut token_data: WebSocketTokenData) -> Uuid {
        let uuid = loop {
            let uuid = Uuid::new_v4();

            match self.pending_tokens.insert_sync(uuid, token_data) {
                Ok(()) => {
                    tracing::debug!("Inserted token {uuid} into cache");
                    break uuid;
                }
                Err((_k, v)) => {
                    tracing::debug!("WS session ID collission on {uuid}");
                    token_data = v;
                }
            }
        };

        let pending_tokens = self.pending_tokens.clone();
        actix_web::rt::spawn(async move {
            time::sleep(TOKEN_EXPIRATION).await;

            // I don't think that this if statement would ever fail? considering we literally just put the fucking token in the map, lol.
            if pending_tokens.remove_async(&uuid).await.is_some() {
                tracing::info!("Removed expired token {uuid}");
            }
        });

        uuid
    }

    pub fn use_token(
        &self,
        uuid: &Uuid,
    ) -> Result<WebSocketTokenData, errors::WebSocketServerError> {
        tracing::debug!("Removing token from cache");

        let (_uuid, token) = self
            .pending_tokens
            .remove_sync(uuid)
            .ok_or(WebSocketServerError::TokenNotFound)?;

        Ok(token)
    }

    #[tracing::instrument(skip_all, fields(event = ?event))]
    pub fn subscribe_to_event(&self, uuid: &Uuid, event: WebSocketSubscriptionType) {
        if self
            .sessions
            .update_sync(uuid, |_, v| {
                if v.subscriptions.insert_sync(event).is_err() {
                    tracing::debug!("Session already subscribed to event")
                } else {
                    tracing::info!("Session subscribed to event");
                }
            })
            .is_none()
        {
            tracing::info!("Tried to subscribe to event {event} but found a non-existent session");
        };
    }

    #[tracing::instrument(skip_all, fields(event = ?event))]
    pub fn unsubscribe_from_event(&self, uuid: &Uuid, event: &WebSocketSubscriptionType) {
        self.sessions
            .update_sync(uuid, |_, v| match v.subscriptions.remove_sync(event) {
                Some(k) => {
                    tracing::info!("Session unsubscribed from {k}");
                }
                None => {
                    tracing::warn!(
                        "Attempted to unsubscribe from event that user was not subscribed to"
                    );
                }
            });
    }

    pub fn get_subscription_list(&self, uuid: &Uuid) -> Vec<WebSocketSubscriptionType> {
        if let Some(data) = self.sessions.get_sync(uuid) {
            let mut subscriptions: Vec<WebSocketSubscriptionType> =
                Vec::with_capacity(data.subscriptions.len());

            data.subscriptions.iter_sync(|k| {
                subscriptions.push(*k);
                true
            });

            subscriptions
        } else {
            Vec::new()
        }
    }

    /// Broadcast an event to all connected clients
    #[tracing::instrument(skip_all)]
    pub async fn broadcast_event(&self, event: WebSocketMessage) {
        let msg =
            serde_json::to_string(&event).expect("Failed to turn event message into a string");
        tracing::debug!("Broadcasting event: {msg}");

        let mut futures: FuturesUnordered<BroadcastFuture> = FuturesUnordered::new();

        self.sessions.iter_sync(|k, client_data| {
            let id = *k;

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
                                .is_subscribed_to(WebSocketSubscriptionType::OwnTransactions))
                            || client_data.is_subscribed_to(WebSocketSubscriptionType::Transactions)
                        {
                            let mut session = client_data.session.clone();
                            let msg = msg.clone();
                            futures.push(Box::pin(async move { (id, session.text(msg).await) }));
                        }
                    }
                    WebSocketEvent::Name { name } => {
                        if (!client_data.is_guest()
                            && (client_data.address == name.owner)
                            && client_data.is_subscribed_to(WebSocketSubscriptionType::OwnNames))
                            || client_data.is_subscribed_to(WebSocketSubscriptionType::Names)
                        {
                            let mut session = client_data.session.clone();
                            let msg = msg.clone();
                            futures.push(Box::pin(async move { (id, session.text(msg).await) }));
                        }
                    }
                }
            }

            true
        });

        while let Some((uuid, result)) = futures.next().await {
            if result.is_err() {
                tracing::warn!("Got an unexpected closed session");
                self.cleanup_session(&uuid);
            }
        }
    }

    /// Broadcast a message to all connected clients
    #[tracing::instrument(skip_all)]
    pub async fn broadcast(&self, msg: impl Into<ByteString>) {
        let msg = msg.into();
        tracing::debug!("Sending msg: {msg}");

        let mut futures: FuturesUnordered<BroadcastFuture> = FuturesUnordered::new();

        self.sessions
            .iter_async(|uuid, v| {
                let id = *uuid;
                let mut session = v.session.clone();

                let msg = msg.clone();

                futures.push(Box::pin(async move { (id, session.text(msg).await) }));

                true
            })
            .await;

        while let Some((uuid, result)) = futures.next().await {
            if result.is_err() {
                tracing::warn!("Got an unexpected closed session");
                self.cleanup_session(&uuid);
            }
        }
    }

    pub fn fetch_session_data(&self, uuid: &Uuid) -> Option<WebSocketSessionData> {
        self.sessions.get_sync(uuid).map(|r| r.clone())
    }
}
