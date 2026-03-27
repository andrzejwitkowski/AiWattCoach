use crate::domain::intervals::{
    Activity, BoxFuture, CreateEvent, DateRange, Event, IntervalsApiPort, IntervalsConnectionError,
    IntervalsConnectionTester, IntervalsCredentials, IntervalsError, UpdateActivity, UpdateEvent,
    UploadActivity, UploadedActivities,
};

#[derive(Clone, Default)]
pub struct DevIntervalsClient;

impl IntervalsConnectionTester for DevIntervalsClient {
    fn test_connection(
        &self,
        _api_key: &str,
        _athlete_id: &str,
    ) -> BoxFuture<Result<(), IntervalsConnectionError>> {
        Box::pin(async { Ok(()) })
    }
}

impl IntervalsApiPort for DevIntervalsClient {
    fn list_events(
        &self,
        _credentials: &IntervalsCredentials,
        _range: &DateRange,
    ) -> BoxFuture<Result<Vec<Event>, IntervalsError>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn get_event(
        &self,
        _credentials: &IntervalsCredentials,
        _event_id: i64,
    ) -> BoxFuture<Result<Event, IntervalsError>> {
        Box::pin(async { Err(IntervalsError::NotFound) })
    }

    fn create_event(
        &self,
        _credentials: &IntervalsCredentials,
        _event: CreateEvent,
    ) -> BoxFuture<Result<Event, IntervalsError>> {
        Box::pin(async {
            Err(IntervalsError::Internal(
                "dev intervals mock does not support event creation".to_string(),
            ))
        })
    }

    fn update_event(
        &self,
        _credentials: &IntervalsCredentials,
        _event_id: i64,
        _event: UpdateEvent,
    ) -> BoxFuture<Result<Event, IntervalsError>> {
        Box::pin(async { Err(IntervalsError::NotFound) })
    }

    fn delete_event(
        &self,
        _credentials: &IntervalsCredentials,
        _event_id: i64,
    ) -> BoxFuture<Result<(), IntervalsError>> {
        Box::pin(async { Err(IntervalsError::NotFound) })
    }

    fn download_fit(
        &self,
        _credentials: &IntervalsCredentials,
        _event_id: i64,
    ) -> BoxFuture<Result<Vec<u8>, IntervalsError>> {
        Box::pin(async { Err(IntervalsError::NotFound) })
    }

    fn list_activities(
        &self,
        _credentials: &IntervalsCredentials,
        _range: &DateRange,
    ) -> BoxFuture<Result<Vec<Activity>, IntervalsError>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn get_activity(
        &self,
        _credentials: &IntervalsCredentials,
        _activity_id: &str,
    ) -> BoxFuture<Result<Activity, IntervalsError>> {
        Box::pin(async { Err(IntervalsError::NotFound) })
    }

    fn upload_activity(
        &self,
        _credentials: &IntervalsCredentials,
        _upload: UploadActivity,
    ) -> BoxFuture<Result<UploadedActivities, IntervalsError>> {
        Box::pin(async {
            Err(IntervalsError::Internal(
                "dev intervals mock does not support activity upload".to_string(),
            ))
        })
    }

    fn update_activity(
        &self,
        _credentials: &IntervalsCredentials,
        _activity_id: &str,
        _activity: UpdateActivity,
    ) -> BoxFuture<Result<Activity, IntervalsError>> {
        Box::pin(async { Err(IntervalsError::NotFound) })
    }

    fn delete_activity(
        &self,
        _credentials: &IntervalsCredentials,
        _activity_id: &str,
    ) -> BoxFuture<Result<(), IntervalsError>> {
        Box::pin(async { Err(IntervalsError::NotFound) })
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::intervals::{
        DateRange, IntervalsApiPort, IntervalsConnectionTester, IntervalsCredentials,
    };

    use super::DevIntervalsClient;

    #[tokio::test]
    async fn returns_empty_lists_for_calendar_queries() {
        let client = DevIntervalsClient;
        let credentials = IntervalsCredentials {
            api_key: "dev-key".to_string(),
            athlete_id: "dev-athlete".to_string(),
        };
        let range = DateRange {
            oldest: "2026-03-23".to_string(),
            newest: "2026-03-29".to_string(),
        };

        assert!(client
            .list_events(&credentials, &range)
            .await
            .unwrap()
            .is_empty());
        assert!(client
            .list_activities(&credentials, &range)
            .await
            .unwrap()
            .is_empty());
        assert!(client
            .test_connection("dev-key", "dev-athlete")
            .await
            .is_ok());
    }
}
