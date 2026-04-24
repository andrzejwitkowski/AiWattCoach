use std::{future::Future, pin::Pin};

use super::{WahooConnectState, WahooError, WahooToken};

pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

pub trait WahooConnectStateRepository: Clone + Send + Sync + 'static {
    fn create(&self, state: WahooConnectState) -> BoxFuture<Result<WahooConnectState, WahooError>>;

    fn consume(&self, state_id: &str) -> BoxFuture<Result<Option<WahooConnectState>, WahooError>>;
}

pub trait WahooOAuthPort: Clone + Send + Sync + 'static {
    fn build_authorize_url(&self, state: &str) -> Result<String, WahooError>;

    fn exchange_code(&self, code: &str) -> BoxFuture<Result<WahooToken, WahooError>>;

    fn refresh_token(&self, refresh_token: &str) -> BoxFuture<Result<WahooToken, WahooError>>;
}
