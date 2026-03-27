use super::client::IntervalsIcuClient;
use super::dev_client::DevIntervalsClient;
use crate::domain::intervals::{
    Activity, BoxFuture, CreateEvent, DateRange, Event, IntervalsApiPort, IntervalsConnectionError,
    IntervalsConnectionTester, IntervalsCredentials, IntervalsError, UpdateActivity, UpdateEvent,
    UploadActivity, UploadedActivities,
};

#[derive(Clone)]
pub enum IntervalsApiAdapter {
    Live(IntervalsIcuClient),
    Dev(DevIntervalsClient),
}

impl IntervalsConnectionTester for IntervalsApiAdapter {
    fn test_connection(
        &self,
        api_key: &str,
        athlete_id: &str,
    ) -> BoxFuture<Result<(), IntervalsConnectionError>> {
        match self {
            Self::Live(client) => client.test_connection(api_key, athlete_id),
            Self::Dev(client) => client.test_connection(api_key, athlete_id),
        }
    }
}

impl IntervalsApiPort for IntervalsApiAdapter {
    fn list_events(
        &self,
        credentials: &IntervalsCredentials,
        range: &DateRange,
    ) -> BoxFuture<Result<Vec<Event>, IntervalsError>> {
        match self {
            Self::Live(client) => client.list_events(credentials, range),
            Self::Dev(client) => client.list_events(credentials, range),
        }
    }

    fn get_event(
        &self,
        credentials: &IntervalsCredentials,
        event_id: i64,
    ) -> BoxFuture<Result<Event, IntervalsError>> {
        match self {
            Self::Live(client) => client.get_event(credentials, event_id),
            Self::Dev(client) => client.get_event(credentials, event_id),
        }
    }

    fn create_event(
        &self,
        credentials: &IntervalsCredentials,
        event: CreateEvent,
    ) -> BoxFuture<Result<Event, IntervalsError>> {
        match self {
            Self::Live(client) => client.create_event(credentials, event),
            Self::Dev(client) => client.create_event(credentials, event),
        }
    }

    fn update_event(
        &self,
        credentials: &IntervalsCredentials,
        event_id: i64,
        event: UpdateEvent,
    ) -> BoxFuture<Result<Event, IntervalsError>> {
        match self {
            Self::Live(client) => client.update_event(credentials, event_id, event),
            Self::Dev(client) => client.update_event(credentials, event_id, event),
        }
    }

    fn delete_event(
        &self,
        credentials: &IntervalsCredentials,
        event_id: i64,
    ) -> BoxFuture<Result<(), IntervalsError>> {
        match self {
            Self::Live(client) => client.delete_event(credentials, event_id),
            Self::Dev(client) => client.delete_event(credentials, event_id),
        }
    }

    fn download_fit(
        &self,
        credentials: &IntervalsCredentials,
        event_id: i64,
    ) -> BoxFuture<Result<Vec<u8>, IntervalsError>> {
        match self {
            Self::Live(client) => client.download_fit(credentials, event_id),
            Self::Dev(client) => client.download_fit(credentials, event_id),
        }
    }

    fn list_activities(
        &self,
        credentials: &IntervalsCredentials,
        range: &DateRange,
    ) -> BoxFuture<Result<Vec<Activity>, IntervalsError>> {
        match self {
            Self::Live(client) => client.list_activities(credentials, range),
            Self::Dev(client) => client.list_activities(credentials, range),
        }
    }

    fn get_activity(
        &self,
        credentials: &IntervalsCredentials,
        activity_id: &str,
    ) -> BoxFuture<Result<Activity, IntervalsError>> {
        match self {
            Self::Live(client) => client.get_activity(credentials, activity_id),
            Self::Dev(client) => client.get_activity(credentials, activity_id),
        }
    }

    fn upload_activity(
        &self,
        credentials: &IntervalsCredentials,
        upload: UploadActivity,
    ) -> BoxFuture<Result<UploadedActivities, IntervalsError>> {
        match self {
            Self::Live(client) => client.upload_activity(credentials, upload),
            Self::Dev(client) => client.upload_activity(credentials, upload),
        }
    }

    fn update_activity(
        &self,
        credentials: &IntervalsCredentials,
        activity_id: &str,
        activity: UpdateActivity,
    ) -> BoxFuture<Result<Activity, IntervalsError>> {
        match self {
            Self::Live(client) => client.update_activity(credentials, activity_id, activity),
            Self::Dev(client) => client.update_activity(credentials, activity_id, activity),
        }
    }

    fn delete_activity(
        &self,
        credentials: &IntervalsCredentials,
        activity_id: &str,
    ) -> BoxFuture<Result<(), IntervalsError>> {
        match self {
            Self::Live(client) => client.delete_activity(credentials, activity_id),
            Self::Dev(client) => client.delete_activity(credentials, activity_id),
        }
    }
}
