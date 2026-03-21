use reqwest::Url;

use crate::{
    adapters::google_oauth::dto::{GoogleTokenResponse, GoogleUserInfoResponse},
    domain::identity::{BoxFuture, GoogleIdentity, GoogleOAuthPort, IdentityError},
};

#[derive(Clone)]
pub struct GoogleOAuthClient {
    pub client: reqwest::Client,
    pub client_id: String,
    pub client_secret: String,
    pub redirect_url: String,
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
        let mut url = Url::parse("https://accounts.google.com/o/oauth2/v2/auth")
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
            let token_response = client
                .post("https://oauth2.googleapis.com/token")
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

            let token_response = token_response
                .error_for_status()
                .map_err(|error| IdentityError::External(error.to_string()))?;
            let token_payload: GoogleTokenResponse = token_response
                .json()
                .await
                .map_err(|error| IdentityError::External(error.to_string()))?;

            let user_info_response = client
                .get("https://openidconnect.googleapis.com/v1/userinfo")
                .bearer_auth(&token_payload.access_token)
                .send()
                .await
                .map_err(|error| IdentityError::External(error.to_string()))?;

            let user_info_response = user_info_response
                .error_for_status()
                .map_err(|error| IdentityError::External(error.to_string()))?;
            let user_info: GoogleUserInfoResponse = user_info_response
                .json()
                .await
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
