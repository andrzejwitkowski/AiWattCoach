use super::{client::WahooOAuthClient, dev_client::DevWahooOAuthClient};
use crate::domain::wahoo::{BoxFuture, WahooError, WahooOAuthPort, WahooToken};

#[derive(Clone)]
pub enum WahooOAuthAdapter {
    Live(WahooOAuthClient),
    Dev(DevWahooOAuthClient),
}

impl WahooOAuthPort for WahooOAuthAdapter {
    fn build_authorize_url(&self, state: &str) -> Result<String, WahooError> {
        match self {
            Self::Live(client) => client.build_authorize_url(state),
            Self::Dev(client) => client.build_authorize_url(state),
        }
    }

    fn exchange_code(&self, code: &str) -> BoxFuture<Result<WahooToken, WahooError>> {
        match self {
            Self::Live(client) => client.exchange_code(code),
            Self::Dev(client) => client.exchange_code(code),
        }
    }

    fn refresh_token(&self, refresh_token: &str) -> BoxFuture<Result<WahooToken, WahooError>> {
        match self {
            Self::Live(client) => client.refresh_token(refresh_token),
            Self::Dev(client) => client.refresh_token(refresh_token),
        }
    }
}
