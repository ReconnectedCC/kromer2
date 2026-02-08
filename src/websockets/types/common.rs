use scc::HashSet;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct WebSocketTokenData {
    pub address: String,
    pub private_key: Option<String>,
}

#[derive(Clone, Serialize)]
pub struct WebSocketSessionData {
    pub address: String,
    pub private_key: Option<String>,
    #[serde(skip)]
    pub session: actix_ws::Session,
    pub subscriptions: HashSet<WebSocketSubscriptionType>,
}

#[derive(Clone, Copy, Debug, Hash, Eq, Serialize, Deserialize, PartialEq, PartialOrd)]
#[serde(rename_all = "camelCase")]
pub enum WebSocketSubscriptionType {
    Blocks,
    OwnBlocks,
    Transactions,
    OwnTransactions,
    Names,
    OwnNames,
    Motd,
}

impl WebSocketSubscriptionType {
    pub fn is_valid(subscription_type: &str) -> bool {
        subscription_type
            .parse::<WebSocketSubscriptionType>()
            .is_ok()
    }

    pub fn into_string(&self) -> String {
        match self {
            WebSocketSubscriptionType::Blocks => "blocks".to_owned(),
            WebSocketSubscriptionType::OwnBlocks => "ownBlocks".to_owned(),
            WebSocketSubscriptionType::Transactions => "transactions".to_owned(),
            WebSocketSubscriptionType::OwnTransactions => "ownTransactions".to_owned(),
            WebSocketSubscriptionType::Names => "names".to_owned(),
            WebSocketSubscriptionType::OwnNames => "ownNames".to_owned(),
            WebSocketSubscriptionType::Motd => "motd".to_owned(),
        }
    }
}

impl WebSocketSessionData {
    pub fn is_guest(&self) -> bool {
        self.address == *"guest"
    }

    pub fn is_subscribed_to(&self, event: WebSocketSubscriptionType) -> bool {
        self.subscriptions.contains_sync(&event)
    }
}

impl std::str::FromStr for WebSocketSubscriptionType {
    type Err = ();

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input {
            "blocks" => Ok(Self::Blocks),
            "ownBlocks" => Ok(Self::OwnBlocks),
            "transactions" => Ok(Self::Transactions),
            "ownTransactions" => Ok(Self::OwnTransactions),
            "names" => Ok(Self::Names),
            "ownNames" => Ok(Self::OwnNames),
            "motd" => Ok(Self::Motd),
            _ => Err(()),
        }
    }
}

impl std::fmt::Display for WebSocketSubscriptionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Blocks => write!(f, "blocks"),
            Self::OwnBlocks => write!(f, "ownBlocks"),
            Self::Transactions => write!(f, "transactions"),
            Self::OwnTransactions => write!(f, "ownTransactions"),
            Self::Names => write!(f, "names"),
            Self::OwnNames => write!(f, "ownNames"),
            Self::Motd => write!(f, "motd"),
        }
    }
}

impl WebSocketTokenData {
    #[inline]
    pub fn new(address: String, private_key: Option<String>) -> Self {
        Self {
            address,
            private_key,
        }
    }
}
