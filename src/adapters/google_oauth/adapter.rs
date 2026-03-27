use super::{client::GoogleOAuthClient, dev_client::DevGoogleOAuthClient};
use crate::domain::identity::{BoxFuture, GoogleIdentity, GoogleOAuthPort, IdentityError};

#[derive(Clone)]
pub enum GoogleOAuthAdapter {
    Google(GoogleOAuthClient),
    Dev(DevGoogleOAuthClient),
}

impl GoogleOAuthPort for GoogleOAuthAdapter {
    fn build_authorize_url(&self, state: &str) -> Result<String, IdentityError> {
        match self {
            Self::Google(client) => client.build_authorize_url(state),
            Self::Dev(client) => client.build_authorize_url(state),
        }
    }

    fn exchange_code_for_identity(
        &self,
        code: &str,
    ) -> BoxFuture<Result<GoogleIdentity, IdentityError>> {
        match self {
            Self::Google(client) => client.exchange_code_for_identity(code),
            Self::Dev(client) => client.exchange_code_for_identity(code),
        }
    }
}
