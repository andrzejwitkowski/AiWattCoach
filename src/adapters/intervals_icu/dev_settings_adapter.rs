use crate::domain::intervals::{
    BoxFuture, IntervalsCredentials, IntervalsError, IntervalsSettingsPort,
};

#[derive(Clone, Default)]
pub struct DevIntervalsSettingsProvider;

impl IntervalsSettingsPort for DevIntervalsSettingsProvider {
    fn get_credentials(
        &self,
        _user_id: &str,
    ) -> BoxFuture<Result<IntervalsCredentials, IntervalsError>> {
        Box::pin(async {
            Ok(IntervalsCredentials {
                api_key: "dev-intervals-api-key".to_string(),
                athlete_id: "dev-athlete".to_string(),
            })
        })
    }
}
