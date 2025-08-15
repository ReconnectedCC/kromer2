use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use super::{addresses::AddressJson, motd::DetailedMotd, transactions::TransactionJson};

#[derive(Debug, Deserialize, Serialize)]
pub struct WebSocketMessage {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ok: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<usize>,
    #[serde(flatten)]
    pub r#type: WebSocketMessageInner,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum WebSocketMessageInner {
    Hello {
        #[serde(flatten)]
        motd: Box<DetailedMotd>,
    },
    Keepalive {
        server_time: String,
    },
    Response {
        #[serde(flatten)]
        data: WebSocketMessageResponse,
    },
    Error {
        error: String,
        message: String,
    },
    Event {
        #[serde(flatten)]
        event: WebSocketEvent,
    },
    Work,
    MakeTransaction {
        /// The privatekey of your address.
        #[serde(rename = "privatekey")]
        private_key: Option<String>, // Ugh, fucking krist..

        /// The recipient of the transaction.
        to: String,

        /// The amount to send to the recipient.
        amount: Decimal,

        /// Optional metadata to include in the transaction.
        metadata: Option<String>,
    },

    GetValidSubscriptionLevels,

    Address {
        address: String,

        /// When supplied, fetch the count of names owned by the address.
        #[serde(rename = "fetchNames")]
        fetch_names: Option<bool>,
    },

    Me,
    GetSubscriptionLevel,
    Logout,
    Login {
        #[serde(rename = "privatekey")]
        private_key: String,
    },

    Subscribe {
        event: String,
    },

    Unsubscribe {
        event: String,
    },
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "responding_to", rename_all = "snake_case")]
pub enum WebSocketMessageResponse {
    Work {
        /// The current Krist work (difficulty)
        work: usize,
    },

    MakeTransaction {
        transaction: TransactionJson,
    },

    GetValidSubscriptionLevels {
        /// All valid subscription levels
        valid_subscription_levels: Vec<String>,
    },

    Address {
        address: AddressJson,
    },

    Me {
        /// Whether the current user is a guest or not
        is_guest: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        address: Option<AddressJson>,
    },

    GetSubscriptionLevel {
        subscription_level: Vec<String>,
    },

    Logout {
        /// Whether the current user is a guest or not
        is_guest: bool,
    },

    Login {
        /// Whether the current user is a guest or not
        is_guest: bool,
        address: Option<AddressJson>,
    },

    Subscribe {
        subscription_level: Vec<String>,
    },

    Unsubscribe {
        subscription_level: Vec<String>,
    },
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "event", rename_all = "camelCase")]
pub enum WebSocketEvent {
    Block {
        block: super::blocks::BlockJson,
        new_work: i64,
    },
    Transaction {
        transaction: TransactionJson,
    },
    Name {
        name: super::names::NameJson,
    },
}

impl WebSocketMessage {
    pub fn new_event(event: WebSocketEvent) -> WebSocketMessage {
        WebSocketMessage {
            ok: None,
            id: None,
            r#type: WebSocketMessageInner::Event { event },
        }
    }
}

// #[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
// pub struct WebSocketEventMessage {
//     #[serde(rename = "type")]
//     pub message_type: String,
//     #[serde(flatten)]
//     pub event: WebSocketEventType,
// }

// impl WebSocketEventMessage {
//     pub fn new_transaction(transaction: TransactionJson) -> WebSocketEventMessage {
//         WebSocketEventMessage {
//             message_type: "event".to_owned(),
//             event: WebSocketEventType::Transaction { transaction },
//         }
//     }
// }

// #[cfg(test)]
// mod tests {
//     use super::{OutgoingWebSocketMessage, WebSocketMessageType};

//     #[test]
//     fn test_hello_type() {
//         let raw = r#"{"ok":true,"type":"hello","server_time":"2025-01-08T19:18:26.589Z","motd":"The API URL has changed to https://krist.dev\n\nBlock submission is disabled ([more info](https://discord.sc3.io))","set":"2023-03-22T21:14:06.000Z","motd_set":"2023-03-22T21:14:06.000Z","public_url":"krist.dev","public_ws_url":"ws.krist.dev","mining_enabled":false,"transactions_enabled":true,"debug_mode":false,"work":575,"last_block":{"height":2121616,"address":"kristdeath","hash":"00000000009f16ac5ded918793310016ea2d61a29d5a328e244cd8478da6924c","short_hash":"00000000009f","value":1,"time":"2022-07-19T20:43:09.000Z","difficulty":551},"package":{"name":"krist","version":"3.5.2","author":"Lemmmy","licence":"GPL-3.0","repository":"https://github.com/tmpim/Krist"},"constants":{"wallet_version":16,"nonce_max_size":24,"name_cost":500,"min_work":1,"max_work":100000,"work_factor":0.025,"seconds_per_block":300},"currency":{"address_prefix":"k","name_suffix":"kst","currency_name":"Krist","currency_symbol":"KST"},"notice":"Krist was originally created by 3d6 and Lemmmy. It is now owned and operated by tmpim, and licensed under GPL-3.0."}"#;
//         let msg: OutgoingWebSocketMessage =
//             serde_json::from_str(raw).expect("failed to deserialize outgoing websocket message");
//         assert_eq!(msg.ok, Some(true));
//         assert_eq!(msg.message.member_str(), "hello");

//         match msg.message {
//             WebSocketMessageType::Hello { motd: _ } => {
//                 // TODO: Checks for motd
//             }
//             _ => panic!("Invalid message type"),
//         }
//     }

//     #[test]
//     fn test_keepalive_type() {
//         let raw = r#"{"type":"keepalive","server_time":"2025-01-08T19:18:26.596Z"}"#;
//         let msg: OutgoingWebSocketMessage =
//             serde_json::from_str(raw).expect("failed to deserialize outgoing websocket message");
//         assert_eq!(msg.ok, None);
//         assert_eq!(msg.message.member_str(), "keepalive");
//     }
// }

impl WebSocketMessageInner {
    /// Return the enum member name as a str
    pub fn member_str(&self) -> &'static str {
        match self {
            WebSocketMessageInner::Address { .. } => "address",
            WebSocketMessageInner::Login { .. } => "login",
            WebSocketMessageInner::Logout => "logout",
            WebSocketMessageInner::Me => "me",
            // WebSocketMessageInner::SubmitBlock => "submit_block",
            WebSocketMessageInner::Subscribe { .. } => "subscribe",
            WebSocketMessageInner::GetSubscriptionLevel => "get_subscription_level",
            WebSocketMessageInner::GetValidSubscriptionLevels => "get_valid_subscription_levels",
            WebSocketMessageInner::Unsubscribe { .. } => "unsubscribe",
            WebSocketMessageInner::MakeTransaction { .. } => "make_transaction",
            WebSocketMessageInner::Work => "work",
            WebSocketMessageInner::Hello { .. } => "hello",
            WebSocketMessageInner::Error { .. } => "error",
            WebSocketMessageInner::Response { .. } => "response",
            WebSocketMessageInner::Keepalive { .. } => "keepalive",
            WebSocketMessageInner::Event { .. } => "event",
            // WebSocketMessageInner::Unknown => "unknown",
        }
    }
}
