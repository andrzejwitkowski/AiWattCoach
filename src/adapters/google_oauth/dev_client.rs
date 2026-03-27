use crate::domain::identity::{BoxFuture, GoogleIdentity, GoogleOAuthPort, IdentityError};

const DEV_AUTH_CODE: &str = "dev-google-auth";

#[derive(Clone)]
pub struct DevGoogleOAuthClient {
    google_subject: String,
    email: String,
    display_name: String,
    avatar_url: Option<String>,
}

impl DevGoogleOAuthClient {
    pub fn new(
        google_subject: impl Into<String>,
        email: impl Into<String>,
        display_name: impl Into<String>,
        avatar_url: Option<String>,
    ) -> Self {
        Self {
            google_subject: google_subject.into(),
            email: email.into(),
            display_name: display_name.into(),
            avatar_url,
        }
    }
}

impl GoogleOAuthPort for DevGoogleOAuthClient {
    fn build_authorize_url(&self, state: &str) -> Result<String, IdentityError> {
        Ok(format!(
            "/api/auth/google/callback?state={state}&code={DEV_AUTH_CODE}"
        ))
    }

    fn exchange_code_for_identity(
        &self,
        code: &str,
    ) -> BoxFuture<Result<GoogleIdentity, IdentityError>> {
        let google_subject = self.google_subject.clone();
        let email = self.email.clone();
        let display_name = self.display_name.clone();
        let avatar_url = self.avatar_url.clone();
        let code = code.to_string();

        Box::pin(async move {
            if code != DEV_AUTH_CODE {
                return Err(IdentityError::Unauthenticated);
            }

            GoogleIdentity::new(
                &google_subject,
                &email,
                true,
                Some(display_name),
                avatar_url,
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::identity::GoogleOAuthPort;

    use super::DevGoogleOAuthClient;

    #[tokio::test]
    async fn builds_same_origin_callback_redirect() {
        let client = DevGoogleOAuthClient::new(
            "dev-google-subject",
            "dev@aiwattcoach.local",
            "Dev Athlete",
            None,
        );

        let redirect = client.build_authorize_url("state-123").unwrap();

        assert_eq!(
            redirect,
            "/api/auth/google/callback?state=state-123&code=dev-google-auth"
        );
    }

    #[tokio::test]
    async fn returns_configured_identity_for_dev_code() {
        let client = DevGoogleOAuthClient::new(
            "dev-google-subject",
            "dev@aiwattcoach.local",
            "Dev Athlete",
            Some("https://example.com/dev.png".to_string()),
        );

        let identity = client
            .exchange_code_for_identity("dev-google-auth")
            .await
            .unwrap();

        assert_eq!(identity.subject, "dev-google-subject");
        assert_eq!(identity.email, "dev@aiwattcoach.local");
        assert_eq!(identity.display_name.as_deref(), Some("Dev Athlete"));
        assert_eq!(
            identity.avatar_url.as_deref(),
            Some("https://example.com/dev.png")
        );
    }

    #[tokio::test]
    async fn rejects_invalid_dev_code_as_unauthenticated() {
        let client = DevGoogleOAuthClient::new(
            "dev-google-subject",
            "dev@aiwattcoach.local",
            "Dev Athlete",
            None,
        );

        let error = client
            .exchange_code_for_identity("wrong-code")
            .await
            .unwrap_err();

        assert_eq!(
            error,
            crate::domain::identity::IdentityError::Unauthenticated
        );
    }
}
