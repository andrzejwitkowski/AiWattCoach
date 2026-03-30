use aiwattcoach::domain::intervals::{IntervalsCredentials, IntervalsError, IntervalsSettingsPort};

use crate::common::BoxFuture;

#[derive(Clone)]
pub(crate) struct FakeSettingsPort {
    credentials: Option<IntervalsCredentials>,
}

impl FakeSettingsPort {
    pub(crate) fn with_credentials(credentials: IntervalsCredentials) -> Self {
        Self {
            credentials: Some(credentials),
        }
    }

    pub(crate) fn without_credentials() -> Self {
        Self { credentials: None }
    }
}

impl IntervalsSettingsPort for FakeSettingsPort {
    fn get_credentials(
        &self,
        _user_id: &str,
    ) -> BoxFuture<Result<IntervalsCredentials, IntervalsError>> {
        let credentials = self.credentials.clone();
        Box::pin(async move { credentials.ok_or(IntervalsError::CredentialsNotConfigured) })
    }
}
