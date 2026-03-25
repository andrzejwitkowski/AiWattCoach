use std::{future::Future, pin::Pin};

use super::{
    Activity, CreateEvent, DateRange, Event, IntervalsCredentials, IntervalsError, UpdateActivity,
    UpdateEvent, UploadActivity, UploadedActivities,
};

pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

#[derive(Clone, Default)]
pub struct NoopActivityRepository;

pub trait IntervalsApiPort: Clone + Send + Sync + 'static {
    fn list_events(
        &self,
        credentials: &IntervalsCredentials,
        range: &DateRange,
    ) -> BoxFuture<Result<Vec<Event>, IntervalsError>>;

    fn get_event(
        &self,
        credentials: &IntervalsCredentials,
        event_id: i64,
    ) -> BoxFuture<Result<Event, IntervalsError>>;

    fn create_event(
        &self,
        credentials: &IntervalsCredentials,
        event: CreateEvent,
    ) -> BoxFuture<Result<Event, IntervalsError>>;

    fn update_event(
        &self,
        credentials: &IntervalsCredentials,
        event_id: i64,
        event: UpdateEvent,
    ) -> BoxFuture<Result<Event, IntervalsError>>;

    fn delete_event(
        &self,
        credentials: &IntervalsCredentials,
        event_id: i64,
    ) -> BoxFuture<Result<(), IntervalsError>>;

    fn download_fit(
        &self,
        credentials: &IntervalsCredentials,
        event_id: i64,
    ) -> BoxFuture<Result<Vec<u8>, IntervalsError>>;

    fn list_activities(
        &self,
        credentials: &IntervalsCredentials,
        range: &DateRange,
    ) -> BoxFuture<Result<Vec<Activity>, IntervalsError>> {
        let _ = (credentials, range);
        Box::pin(async {
            Err(IntervalsError::Internal(
                "activity listing not implemented".to_string(),
            ))
        })
    }

    fn get_activity(
        &self,
        credentials: &IntervalsCredentials,
        activity_id: &str,
    ) -> BoxFuture<Result<Activity, IntervalsError>> {
        let _ = (credentials, activity_id);
        Box::pin(async {
            Err(IntervalsError::Internal(
                "activity lookup not implemented".to_string(),
            ))
        })
    }

    fn upload_activity(
        &self,
        credentials: &IntervalsCredentials,
        upload: UploadActivity,
    ) -> BoxFuture<Result<UploadedActivities, IntervalsError>> {
        let _ = (credentials, upload);
        Box::pin(async {
            Err(IntervalsError::Internal(
                "activity upload not implemented".to_string(),
            ))
        })
    }

    fn update_activity(
        &self,
        credentials: &IntervalsCredentials,
        activity_id: &str,
        activity: UpdateActivity,
    ) -> BoxFuture<Result<Activity, IntervalsError>> {
        let _ = (credentials, activity_id, activity);
        Box::pin(async {
            Err(IntervalsError::Internal(
                "activity update not implemented".to_string(),
            ))
        })
    }

    fn delete_activity(
        &self,
        credentials: &IntervalsCredentials,
        activity_id: &str,
    ) -> BoxFuture<Result<(), IntervalsError>> {
        let _ = (credentials, activity_id);
        Box::pin(async {
            Err(IntervalsError::Internal(
                "activity delete not implemented".to_string(),
            ))
        })
    }
}

pub trait ActivityRepositoryPort: Clone + Send + Sync + 'static {
    fn upsert(
        &self,
        user_id: &str,
        activity: Activity,
    ) -> BoxFuture<Result<Activity, IntervalsError>>;

    fn upsert_many(
        &self,
        user_id: &str,
        activities: Vec<Activity>,
    ) -> BoxFuture<Result<Vec<Activity>, IntervalsError>>;

    fn find_by_user_id_and_range(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> BoxFuture<Result<Vec<Activity>, IntervalsError>>;

    fn find_by_user_id_and_activity_id(
        &self,
        user_id: &str,
        activity_id: &str,
    ) -> BoxFuture<Result<Option<Activity>, IntervalsError>>;

    fn delete(&self, user_id: &str, activity_id: &str) -> BoxFuture<Result<(), IntervalsError>>;
}

impl ActivityRepositoryPort for NoopActivityRepository {
    fn upsert(
        &self,
        _user_id: &str,
        activity: Activity,
    ) -> BoxFuture<Result<Activity, IntervalsError>> {
        Box::pin(async move { Ok(activity) })
    }

    fn upsert_many(
        &self,
        _user_id: &str,
        activities: Vec<Activity>,
    ) -> BoxFuture<Result<Vec<Activity>, IntervalsError>> {
        Box::pin(async move { Ok(activities) })
    }

    fn find_by_user_id_and_range(
        &self,
        _user_id: &str,
        _range: &DateRange,
    ) -> BoxFuture<Result<Vec<Activity>, IntervalsError>> {
        Box::pin(async move { Ok(Vec::new()) })
    }

    fn find_by_user_id_and_activity_id(
        &self,
        _user_id: &str,
        _activity_id: &str,
    ) -> BoxFuture<Result<Option<Activity>, IntervalsError>> {
        Box::pin(async move { Ok(None) })
    }

    fn delete(&self, _user_id: &str, _activity_id: &str) -> BoxFuture<Result<(), IntervalsError>> {
        Box::pin(async move { Ok(()) })
    }
}

pub trait IntervalsSettingsPort: Clone + Send + Sync + 'static {
    fn get_credentials(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<IntervalsCredentials, IntervalsError>>;
}
