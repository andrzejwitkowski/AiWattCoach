use super::{
    ports::BoxFuture, Activity, ActivityRepositoryPort, CreateEvent, DateRange, Event,
    IntervalsApiPort, IntervalsError, IntervalsSettingsPort, UpdateActivity, UpdateEvent,
    UploadActivity, UploadedActivities,
};

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
pub struct IntervalsService<Api, Settings, Activities>
where
    Api: IntervalsApiPort,
    Settings: IntervalsSettingsPort,
    Activities: ActivityRepositoryPort,
{
    api: Api,
    settings: Settings,
    activities: Activities,
}

impl<Api, Settings, Activities> IntervalsService<Api, Settings, Activities>
where
    Api: IntervalsApiPort,
    Settings: IntervalsSettingsPort,
    Activities: ActivityRepositoryPort,
{
    pub fn new(api: Api, settings: Settings, activities: Activities) -> Self {
        Self {
            api,
            settings,
            activities,
        }
    }
}

impl<Api, Settings, Activities> IntervalsUseCases for IntervalsService<Api, Settings, Activities>
where
    Api: IntervalsApiPort,
    Settings: IntervalsSettingsPort,
    Activities: ActivityRepositoryPort,
{
    fn list_events(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> BoxFuture<Result<Vec<Event>, IntervalsError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        let range = range.clone();
        Box::pin(async move {
            let credentials = service.settings.get_credentials(&user_id).await?;
            service.api.list_events(&credentials, &range).await
        })
    }

    fn get_event(&self, user_id: &str, event_id: i64) -> BoxFuture<Result<Event, IntervalsError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let credentials = service.settings.get_credentials(&user_id).await?;
            service.api.get_event(&credentials, event_id).await
        })
    }

    fn create_event(
        &self,
        user_id: &str,
        event: CreateEvent,
    ) -> BoxFuture<Result<Event, IntervalsError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let credentials = service.settings.get_credentials(&user_id).await?;
            service.api.create_event(&credentials, event).await
        })
    }

    fn update_event(
        &self,
        user_id: &str,
        event_id: i64,
        event: UpdateEvent,
    ) -> BoxFuture<Result<Event, IntervalsError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let credentials = service.settings.get_credentials(&user_id).await?;
            service
                .api
                .update_event(&credentials, event_id, event)
                .await
        })
    }

    fn delete_event(&self, user_id: &str, event_id: i64) -> BoxFuture<Result<(), IntervalsError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let credentials = service.settings.get_credentials(&user_id).await?;
            service.api.delete_event(&credentials, event_id).await
        })
    }

    fn download_fit(
        &self,
        user_id: &str,
        event_id: i64,
    ) -> BoxFuture<Result<Vec<u8>, IntervalsError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let credentials = service.settings.get_credentials(&user_id).await?;
            service.api.download_fit(&credentials, event_id).await
        })
    }

    fn list_activities(
        &self,
        user_id: &str,
        range: &DateRange,
    ) -> BoxFuture<Result<Vec<Activity>, IntervalsError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        let range = range.clone();
        Box::pin(async move {
            let credentials = service.settings.get_credentials(&user_id).await?;
            let activities = service.api.list_activities(&credentials, &range).await?;
            service
                .activities
                .upsert_many(&user_id, activities.clone())
                .await?;
            service
                .activities
                .find_by_user_id_and_range(&user_id, &range)
                .await
        })
    }

    fn get_activity(
        &self,
        user_id: &str,
        activity_id: &str,
    ) -> BoxFuture<Result<Activity, IntervalsError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        let activity_id = activity_id.to_string();
        Box::pin(async move {
            let credentials = service.settings.get_credentials(&user_id).await?;
            let activity = service.api.get_activity(&credentials, &activity_id).await?;
            service
                .activities
                .upsert(&user_id, activity.clone())
                .await?;
            Ok(activity)
        })
    }

    fn upload_activity(
        &self,
        user_id: &str,
        upload: UploadActivity,
    ) -> BoxFuture<Result<UploadedActivities, IntervalsError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let credentials = service.settings.get_credentials(&user_id).await?;
            let uploaded = service.api.upload_activity(&credentials, upload).await?;
            service
                .activities
                .upsert_many(&user_id, uploaded.activities.clone())
                .await?;
            Ok(uploaded)
        })
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
            let credentials = service.settings.get_credentials(&user_id).await?;
            let updated = service
                .api
                .update_activity(&credentials, &activity_id, activity)
                .await?;
            service.activities.upsert(&user_id, updated.clone()).await?;
            Ok(updated)
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
        Box::pin(async move {
            let credentials = service.settings.get_credentials(&user_id).await?;
            service
                .api
                .delete_activity(&credentials, &activity_id)
                .await?;
            service.activities.delete(&user_id, &activity_id).await
        })
    }
}
