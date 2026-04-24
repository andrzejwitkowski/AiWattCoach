use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct WahooTokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,
}
