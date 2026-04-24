use crate::domain::wahoo::{BoxFuture, WahooError, WahooOAuthPort, WahooToken};

const DEV_AUTH_CODE: &str = "dev-wahoo-auth";

#[derive(Clone)]
pub struct DevWahooOAuthClient;

impl WahooOAuthPort for DevWahooOAuthClient {
    fn build_authorize_url(&self, state: &str) -> Result<String, WahooError> {
        Ok(format!(
            "/api/auth/wahoo/callback?state={state}&code={DEV_AUTH_CODE}"
        ))
    }

    fn exchange_code(&self, code: &str) -> BoxFuture<Result<WahooToken, WahooError>> {
        let code = code.to_string();
        Box::pin(async move {
            if code != DEV_AUTH_CODE {
                return Err(WahooError::External("invalid dev Wahoo code".to_string()));
            }

            Ok(WahooToken {
                access_token: "dev-wahoo-access-token".to_string(),
                refresh_token: "dev-wahoo-refresh-token".to_string(),
                expires_at_epoch_seconds: chrono::Utc::now().timestamp() + 7200,
            })
        })
    }

    fn refresh_token(&self, refresh_token: &str) -> BoxFuture<Result<WahooToken, WahooError>> {
        let refresh_token = refresh_token.to_string();
        Box::pin(async move {
            if refresh_token.trim().is_empty() {
                return Err(WahooError::NotConfigured);
            }

            Ok(WahooToken {
                access_token: "dev-wahoo-access-token-refreshed".to_string(),
                refresh_token: "dev-wahoo-refresh-token-refreshed".to_string(),
                expires_at_epoch_seconds: chrono::Utc::now().timestamp() + 7200,
            })
        })
    }
}
