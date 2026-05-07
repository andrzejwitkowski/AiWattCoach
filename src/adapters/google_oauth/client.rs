use reqwest::Url;

use crate::{
    adapters::google_oauth::dto::{GoogleTokenResponse, GoogleUserInfoResponse},
    domain::identity::{BoxFuture, GoogleIdentity, GoogleOAuthPort, IdentityError},
};

use super::logging;

const GOOGLE_AUTHORIZE_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const GOOGLE_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const GOOGLE_USERINFO_URL: &str = "https://openidconnect.googleapis.com/v1/userinfo";

#[derive(Clone)]
pub struct GoogleOAuthClient {
    client: reqwest::Client,
    client_id: String,
    client_secret: String,
    redirect_url: String,
}

impl GoogleOAuthClient {
    pub fn new(
        client: reqwest::Client,
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
        redirect_url: impl Into<String>,
    ) -> Self {
        Self {
            client,
            client_id: client_id.into(),
            client_secret: client_secret.into(),
            redirect_url: redirect_url.into(),
        }
    }
}

impl GoogleOAuthPort for GoogleOAuthClient {
    fn build_authorize_url(&self, state: &str) -> Result<String, IdentityError> {
        let mut url = Url::parse(GOOGLE_AUTHORIZE_URL)
            .map_err(|error| IdentityError::External(error.to_string()))?;
        url.query_pairs_mut()
            .append_pair("response_type", "code")
            .append_pair("client_id", &self.client_id)
            .append_pair("redirect_uri", &self.redirect_url)
            .append_pair("scope", "openid email profile")
            .append_pair("state", state);

        Ok(url.to_string())
    }

    fn exchange_code_for_identity(
        &self,
        code: &str,
    ) -> BoxFuture<Result<GoogleIdentity, IdentityError>> {
        let client = self.client.clone();
        let client_id = self.client_id.clone();
        let client_secret = self.client_secret.clone();
        let redirect_url = self.redirect_url.clone();
        let code = code.to_string();

        Box::pin(async move {
            let token_form = vec![
                ("code".to_string(), code.clone()),
                ("client_id".to_string(), client_id.clone()),
                ("client_secret".to_string(), client_secret.clone()),
                ("redirect_uri".to_string(), redirect_url.clone()),
                ("grant_type".to_string(), "authorization_code".to_string()),
            ];
            logging::log_request("POST", GOOGLE_TOKEN_URL, &token_form);

            let token_response = client
                .post(GOOGLE_TOKEN_URL)
                .form(&[
                    ("code", code.as_str()),
                    ("client_id", client_id.as_str()),
                    ("client_secret", client_secret.as_str()),
                    ("redirect_uri", redirect_url.as_str()),
                    ("grant_type", "authorization_code"),
                ])
                .send()
                .await
                .map_err(|error| IdentityError::External(error.to_string()))?;

            let token_status = token_response.status();
            let token_body = token_response
                .text()
                .await
                .map_err(|error| IdentityError::External(error.to_string()))?;
            logging::log_response("POST", GOOGLE_TOKEN_URL, token_status, &token_body);
            if !token_status.is_success() {
                return Err(IdentityError::External(format!(
                    "Google token exchange failed with status {token_status}"
                )));
            }
            let token_payload: GoogleTokenResponse = serde_json::from_str(&token_body)
                .map_err(|error| IdentityError::External(error.to_string()))?;

            logging::log_request("GET", GOOGLE_USERINFO_URL, &[]);
            let user_info_response = client
                .get(GOOGLE_USERINFO_URL)
                .bearer_auth(&token_payload.access_token)
                .send()
                .await
                .map_err(|error| IdentityError::External(error.to_string()))?;

            let user_info_status = user_info_response.status();
            let user_info_body = user_info_response
                .text()
                .await
                .map_err(|error| IdentityError::External(error.to_string()))?;
            logging::log_response(
                "GET",
                GOOGLE_USERINFO_URL,
                user_info_status,
                &user_info_body,
            );
            if !user_info_status.is_success() {
                return Err(IdentityError::External(format!(
                    "Google userinfo request failed with status {user_info_status}"
                )));
            }
            let user_info: GoogleUserInfoResponse = serde_json::from_str(&user_info_body)
                .map_err(|error| IdentityError::External(error.to_string()))?;

            GoogleIdentity::new(
                &user_info.sub,
                &user_info.email,
                user_info.email_verified,
                user_info.name,
                user_info.picture,
            )
        })
    }
}
