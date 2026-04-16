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
    EnrichedEvent, Event, IntervalsApiPort, IntervalsError, IntervalsSettingsPort,
    NoopPestParserPocRepository, PestParserPocDirection, PestParserPocOperation,
    PestParserPocRepositoryPort, PestParserPocSource, PestParserPocWorkoutRecord, UpdateActivity,
    UpdateEvent, UploadActivity, UploadedActivities,
};
use crate::domain::calendar_view::{CalendarEntryViewRefreshPort, NoopCalendarEntryViewRefresh};
use crate::domain::identity::Clock;
use crate::domain::intervals::workout::parse_workout_ast;
use std::time::{SystemTime, UNIX_EPOCH};
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
pub struct IntervalsService<
    Api,
    Settings,
    Activities,
    UploadOperations,
    Extractor,
    PocRepo = NoopPestParserPocRepository,
    Time = LiveClock,
    Refresh = NoopCalendarEntryViewRefresh,
> where
    Api: IntervalsApiPort,
    Settings: IntervalsSettingsPort,
    Activities: ActivityRepositoryPort,
    UploadOperations: ActivityUploadOperationRepositoryPort,
    Extractor: ActivityFileIdentityExtractorPort,
    PocRepo: PestParserPocRepositoryPort,
    Time: Clock,
    Refresh: CalendarEntryViewRefreshPort,
{
    api: Api,
    settings: Settings,
    activities: Activities,
    upload_operations: UploadOperations,
    identity_extractor: Extractor,
    pest_parser_poc_repository: Option<PocRepo>,
    clock: Time,
    refresh: Refresh,
}

impl<Api, Settings, Activities, UploadOperations, Extractor>
    IntervalsService<
        Api,
        Settings,
        Activities,
        UploadOperations,
        Extractor,
        NoopPestParserPocRepository,
        LiveClock,
        NoopCalendarEntryViewRefresh,
    >
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
            pest_parser_poc_repository: None,
            clock: LiveClock,
            refresh: NoopCalendarEntryViewRefresh,
        }
    }
}

impl<Api, Settings, Activities, UploadOperations, Extractor, PocRepo, Time, Refresh>
    IntervalsService<Api, Settings, Activities, UploadOperations, Extractor, PocRepo, Time, Refresh>
where
    Api: IntervalsApiPort,
    Settings: IntervalsSettingsPort,
    Activities: ActivityRepositoryPort,
    UploadOperations: ActivityUploadOperationRepositoryPort,
    Extractor: ActivityFileIdentityExtractorPort,
    PocRepo: PestParserPocRepositoryPort,
    Time: Clock,
    Refresh: CalendarEntryViewRefreshPort,
{
    pub fn with_pest_parser_poc_repository<Repo>(
        self,
        repository: Repo,
    ) -> IntervalsService<Api, Settings, Activities, UploadOperations, Extractor, Repo, Time, Refresh>
    where
        Repo: PestParserPocRepositoryPort,
    {
        IntervalsService {
            api: self.api,
            settings: self.settings,
            activities: self.activities,
            upload_operations: self.upload_operations,
            identity_extractor: self.identity_extractor,
            pest_parser_poc_repository: Some(repository),
            clock: self.clock,
            refresh: self.refresh,
        }
    }

    pub fn with_clock<NewTime>(
        self,
        clock: NewTime,
    ) -> IntervalsService<
        Api,
        Settings,
        Activities,
        UploadOperations,
        Extractor,
        PocRepo,
        NewTime,
        Refresh,
    >
    where
        NewTime: Clock,
    {
        IntervalsService {
            api: self.api,
            settings: self.settings,
            activities: self.activities,
            upload_operations: self.upload_operations,
            identity_extractor: self.identity_extractor,
            pest_parser_poc_repository: self.pest_parser_poc_repository,
            clock,
            refresh: self.refresh,
        }
    }

    pub fn with_calendar_view_refresh<NewRefresh>(
        self,
        refresh: NewRefresh,
    ) -> IntervalsService<
        Api,
        Settings,
        Activities,
        UploadOperations,
        Extractor,
        PocRepo,
        Time,
        NewRefresh,
    >
    where
        NewRefresh: CalendarEntryViewRefreshPort,
    {
        IntervalsService {
            api: self.api,
            settings: self.settings,
            activities: self.activities,
            upload_operations: self.upload_operations,
            identity_extractor: self.identity_extractor,
            pest_parser_poc_repository: self.pest_parser_poc_repository,
            clock: self.clock,
            refresh,
        }
    }

    pub(super) async fn observe_workout_text(
        &self,
        user_id: &str,
        source: PestParserPocSource,
        source_ref: Option<String>,
        workout_text: Option<&str>,
    ) {
        let Some(repository) = &self.pest_parser_poc_repository else {
            return;
        };
        let Some(workout_text) = workout_text.filter(|value| !value.trim().is_empty()) else {
            return;
        };

        let parsed_at_epoch_seconds = self.clock.now_epoch_seconds();
        let parser_version = "pest-parser-poc-v1".to_string();
        let source_text = workout_text.trim().to_string();
        let legacy_projection = parse_workout_doc(Some(&source_text), None);
        let context = super::PestParserPocRecordContext {
            user_id: user_id.to_string(),
            source,
            source_ref,
            source_text: source_text.clone(),
            parser_version,
            parsed_at_epoch_seconds,
        };
        let record = match parse_workout_ast(&source_text) {
            Ok(_) => PestParserPocWorkoutRecord::parsed(
                context,
                normalize_workout_text(&source_text),
                legacy_projection,
            ),
            Err(error) => PestParserPocWorkoutRecord::failed(
                context,
                error.to_string(),
                "syntax".to_string(),
                legacy_projection,
            ),
        };

        if let Err(error) = repository.insert(record).await {
            tracing::error!(%user_id, %error, "failed to persist pest parser poc workout record");
        }
    }
}

#[derive(Clone)]
pub struct LiveClock;

impl Clock for LiveClock {
    fn now_epoch_seconds(&self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64
    }
}

fn normalize_workout_text(input: &str) -> String {
    input
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| line.trim_start_matches('-').trim())
        .collect::<Vec<_>>()
        .join("\n")
}

impl<Api, Settings, Activities, UploadOperations, Extractor, PocRepo, Time, Refresh>
    IntervalsUseCases
    for IntervalsService<
        Api,
        Settings,
        Activities,
        UploadOperations,
        Extractor,
        PocRepo,
        Time,
        Refresh,
    >
where
    Api: IntervalsApiPort,
    Settings: IntervalsSettingsPort,
    Activities: ActivityRepositoryPort,
    UploadOperations: ActivityUploadOperationRepositoryPort,
    Extractor: ActivityFileIdentityExtractorPort,
    PocRepo: PestParserPocRepositoryPort,
    Time: Clock,
    Refresh: CalendarEntryViewRefreshPort,
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
