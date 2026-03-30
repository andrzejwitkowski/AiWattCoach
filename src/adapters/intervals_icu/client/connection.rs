use reqwest::StatusCode;

use crate::domain::intervals::{BoxFuture, IntervalsConnectionError, IntervalsConnectionTester};

use super::IntervalsIcuClient;

impl IntervalsConnectionTester for IntervalsIcuClient {
    fn test_connection(
        &self,
        api_key: &str,
        athlete_id: &str,
    ) -> BoxFuture<Result<(), IntervalsConnectionError>> {
        let client = self.client.clone();
        let base_url = self.base_url.clone();
        let api_key = api_key.to_string();
        let athlete_id = athlete_id.to_string();

        Box::pin(async move {
            let url = IntervalsIcuClient::athlete_url_impl(&base_url, &athlete_id, "");

            let response = client.get(&url).basic_auth("API_KEY", Some(&api_key));
            let response = IntervalsIcuClient::with_trace_context(response)
                .send()
                .await
                .map_err(|_| IntervalsConnectionError::Unavailable)?;

            let status = response.status();

            if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
                return Err(IntervalsConnectionError::Unauthenticated);
            }

            if status == StatusCode::NOT_FOUND {
                return Err(IntervalsConnectionError::InvalidConfiguration);
            }

            if !status.is_success() {
                return Err(IntervalsConnectionError::Unavailable);
            }

            Ok(())
        })
    }
}
