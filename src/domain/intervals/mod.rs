mod model;
mod ports;
mod service;

pub use model::{
    normalize_external_id, round_distance_bucket, round_duration_bucket, Activity,
    ActivityDeduplicationIdentity, ActivityDetails, ActivityFallbackIdentity, ActivityInterval,
    ActivityIntervalGroup, ActivityMetrics, ActivityStream, ActivityZoneTime, CreateEvent,
    DateRange, Event, EventCategory, EventFileUpload, IntervalsCredentials, IntervalsError,
    UpdateActivity, UpdateEvent, UploadActivity, UploadedActivities,
};
pub use ports::{
    ActivityFileIdentityExtractorPort, ActivityRepositoryPort, BoxFuture, IntervalsApiPort,
    IntervalsSettingsPort, NoopActivityFileIdentityExtractor, NoopActivityRepository,
};
pub use service::{
    IntervalsConnectionError, IntervalsConnectionTester, IntervalsService, IntervalsUseCases,
};
