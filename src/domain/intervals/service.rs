mod activities;
mod enriched;
mod events;
mod upload;

use super::{
    build_activity_upload_operation_key, find_best_activity_match, normalize_external_id,
    parse_workout_doc,
    ports::{ActivityFileIdentityExtractorPort, BoxFuture},
    Activity, ActivityRepositoryPort, ActivityUploadOperation, ActivityUploadOperationClaimResult,
    ActivityUploadOperationRepositoryPort, ActivityUploadOperationStatus, CreateEvent, DateRange,
    EnrichedEvent, Event, IntervalsApiPort, IntervalsError, IntervalsSettingsPort, UpdateActivity,
    UpdateEvent, UploadActivity, UploadedActivities,
};
use tracing::warn;

#[derive(Clone, Debug, PartialEq)]
pub enum IntervalsConnectionError {
    Unauthenticated,
    InvalidConfiguration,
    Unavailable,
}

impl std::fmt::Display for IntervalsConnectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unauthenticated => write!(f, "Invalid API key or athlete ID"),
            Self::InvalidConfiguration => write!(f, "Invalid configuration"),
            Self::Unavailable => write!(f, "Intervals.icu is currently unavailable"),
        }
    }
}

impl std::error::Error for IntervalsConnectionError {}

pub trait IntervalsConnectionTester: Send + Sync + 'static {
    fn test_connection(
        &self,
        api_key: &str,
        athlete_id: &str,
    ) -> BoxFuture<Result<(), IntervalsConnectionError>>;
}

pub trait IntervalsUseCases: Send + Sync {
    fn list_events(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> BoxFuture<Result<Vec<Event>, IntervalsError>>;

    fn get_event(&self, user_id: &str, event_id: i64) -> BoxFuture<Result<Event, IntervalsError>>;

    fn get_enriched_event(
        &self,
        user_id: &str,
        event_id: i64,
    ) -> BoxFuture<Result<EnrichedEvent, IntervalsError>> {
        let _ = (user_id, event_id);
        Box::pin(async {
            Err(IntervalsError::Internal(
                "enriched event lookup not implemented".to_string(),
            ))
        })
    }

    fn create_event(
        &self,
        user_id: &str,
        event: CreateEvent,
    ) -> BoxFuture<Result<Event, IntervalsError>>;

    fn update_event(
        &self,
        user_id: &str,
        event_id: i64,
        event: UpdateEvent,
    ) -> BoxFuture<Result<Event, IntervalsError>>;

    fn delete_event(&self, user_id: &str, event_id: i64) -> BoxFuture<Result<(), IntervalsError>>;

    fn download_fit(
        &self,
        user_id: &str,
        event_id: i64,
    ) -> BoxFuture<Result<Vec<u8>, IntervalsError>>;

    fn list_activities(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> BoxFuture<Result<Vec<Activity>, IntervalsError>> {
        let _ = (user_id, range);
        Box::pin(async {
            Err(IntervalsError::Internal(
                "activity listing not implemented".to_string(),
            ))
        })
    }

    fn get_activity(
        &self,
        user_id: &str,
        activity_id: &str,
    ) -> BoxFuture<Result<Activity, IntervalsError>> {
        let _ = (user_id, activity_id);
        Box::pin(async {
            Err(IntervalsError::Internal(
                "activity lookup not implemented".to_string(),
            ))
        })
    }

    fn upload_activity(
        &self,
        user_id: &str,
        upload: UploadActivity,
    ) -> BoxFuture<Result<UploadedActivities, IntervalsError>> {
        let _ = (user_id, upload);
        Box::pin(async {
            Err(IntervalsError::Internal(
                "activity upload not implemented".to_string(),
            ))
        })
    }

    fn update_activity(
        &self,
        user_id: &str,
        activity_id: &str,
        activity: UpdateActivity,
    ) -> BoxFuture<Result<Activity, IntervalsError>> {
        let _ = (user_id, activity_id, activity);
        Box::pin(async {
            Err(IntervalsError::Internal(
                "activity update not implemented".to_string(),
            ))
        })
    }

    fn delete_activity(
        &self,
        user_id: &str,
        activity_id: &str,
    ) -> BoxFuture<Result<(), IntervalsError>> {
        let _ = (user_id, activity_id);
        Box::pin(async {
            Err(IntervalsError::Internal(
                "activity delete not implemented".to_string(),
            ))
        })
    }
}

#[derive(Clone)]
pub struct IntervalsService<Api, Settings, Activities, UploadOperations, Extractor>
where
    Api: IntervalsApiPort,
    Settings: IntervalsSettingsPort,
    Activities: ActivityRepositoryPort,
    UploadOperations: ActivityUploadOperationRepositoryPort,
    Extractor: ActivityFileIdentityExtractorPort,
{
    api: Api,
    settings: Settings,
    activities: Activities,
    upload_operations: UploadOperations,
    identity_extractor: Extractor,
}

impl<Api, Settings, Activities, UploadOperations, Extractor>
    IntervalsService<Api, Settings, Activities, UploadOperations, Extractor>
where
    Api: IntervalsApiPort,
    Settings: IntervalsSettingsPort,
    Activities: ActivityRepositoryPort,
    UploadOperations: ActivityUploadOperationRepositoryPort,
    Extractor: ActivityFileIdentityExtractorPort,
{
    pub fn new(
        api: Api,
        settings: Settings,
        activities: Activities,
        upload_operations: UploadOperations,
        identity_extractor: Extractor,
    ) -> Self {
        Self {
            api,
            settings,
            activities,
            upload_operations,
            identity_extractor,
        }
    }
}

impl<Api, Settings, Activities, UploadOperations, Extractor> IntervalsUseCases
    for IntervalsService<Api, Settings, Activities, UploadOperations, Extractor>
where
    Api: IntervalsApiPort,
    Settings: IntervalsSettingsPort,
    Activities: ActivityRepositoryPort,
    UploadOperations: ActivityUploadOperationRepositoryPort,
    Extractor: ActivityFileIdentityExtractorPort,
{
    fn list_events(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> BoxFuture<Result<Vec<Event>, IntervalsError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        let range = range.clone();
        Box::pin(async move { service.list_events_impl(&user_id, &range).await })
    }

    fn get_event(&self, user_id: &str, event_id: i64) -> BoxFuture<Result<Event, IntervalsError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move { service.get_event_impl(&user_id, event_id).await })
    }

    fn get_enriched_event(
        &self,
        user_id: &str,
        event_id: i64,
    ) -> BoxFuture<Result<EnrichedEvent, IntervalsError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move { service.get_enriched_event_impl(&user_id, event_id).await })
    }

    fn create_event(
        &self,
        user_id: &str,
        event: CreateEvent,
    ) -> BoxFuture<Result<Event, IntervalsError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move { service.create_event_impl(&user_id, event).await })
    }

    fn update_event(
        &self,
        user_id: &str,
        event_id: i64,
        event: UpdateEvent,
    ) -> BoxFuture<Result<Event, IntervalsError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move { service.update_event_impl(&user_id, event_id, event).await })
    }

    fn delete_event(&self, user_id: &str, event_id: i64) -> BoxFuture<Result<(), IntervalsError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move { service.delete_event_impl(&user_id, event_id).await })
    }

    fn download_fit(
        &self,
        user_id: &str,
        event_id: i64,
    ) -> BoxFuture<Result<Vec<u8>, IntervalsError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move { service.download_fit_impl(&user_id, event_id).await })
    }

    fn list_activities(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> BoxFuture<Result<Vec<Activity>, IntervalsError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        let range = range.clone();
        Box::pin(async move { service.list_activities_impl(&user_id, &range).await })
    }

    fn get_activity(
        &self,
        user_id: &str,
        activity_id: &str,
    ) -> BoxFuture<Result<Activity, IntervalsError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        let activity_id = activity_id.to_string();
        Box::pin(async move { service.get_activity_impl(&user_id, &activity_id).await })
    }

    fn upload_activity(
        &self,
        user_id: &str,
        upload: UploadActivity,
    ) -> BoxFuture<Result<UploadedActivities, IntervalsError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move { service.upload_activity_impl(&user_id, upload).await })
    }

    fn update_activity(
        &self,
        user_id: &str,
        activity_id: &str,
        activity: UpdateActivity,
    ) -> BoxFuture<Result<Activity, IntervalsError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        let activity_id = activity_id.to_string();
        Box::pin(async move {
            service
                .update_activity_impl(&user_id, &activity_id, activity)
                .await
        })
    }

    fn delete_activity(
        &self,
        user_id: &str,
        activity_id: &str,
    ) -> BoxFuture<Result<(), IntervalsError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        let activity_id = activity_id.to_string();
        Box::pin(async move { service.delete_activity_impl(&user_id, &activity_id).await })
    }
}
