#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WahooError {
    Unauthenticated,
    InvalidConnectState,
    NotConnected,
    Repository(String),
    External(String),
}

impl std::fmt::Display for WahooError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unauthenticated => write!(f, "Authentication is required"),
            Self::InvalidConnectState => write!(f, "Wahoo connect state is invalid or expired"),
            Self::NotConnected => write!(f, "Wahoo account is not connected"),
            Self::Repository(message) | Self::External(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for WahooError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WahooToken {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at_epoch_seconds: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WahooConnectState {
    pub id: String,
    pub user_id: String,
    pub return_to: Option<String>,
    pub expires_at_epoch_seconds: i64,
    pub created_at_epoch_seconds: i64,
}

impl WahooConnectState {
    pub fn new(
        id: String,
        user_id: String,
        return_to: Option<String>,
        expires_at_epoch_seconds: i64,
        created_at_epoch_seconds: i64,
    ) -> Self {
        Self {
            id,
            user_id,
            return_to,
            expires_at_epoch_seconds,
            created_at_epoch_seconds,
        }
    }

    pub fn is_expired(&self, now_epoch_seconds: i64) -> bool {
        self.expires_at_epoch_seconds <= now_epoch_seconds
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WahooAuthStart {
    pub state: String,
    pub redirect_url: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WahooAuthExchange {
    pub redirect_to: String,
    pub token: WahooToken,
}
