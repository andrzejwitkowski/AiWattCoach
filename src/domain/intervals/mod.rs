mod model;
mod ports;
mod service;
mod workout;

pub use model::{
    build_activity_upload_operation_key, normalize_external_id, round_distance_bucket,
    round_duration_bucket, Activity, ActivityDeduplicationIdentity, ActivityDetails,
    ActivityFallbackIdentity, ActivityInterval, ActivityIntervalGroup, ActivityMetrics,
    ActivityStream, ActivityUploadOperation, ActivityUploadOperationStatus, ActivityZoneTime,
    CreateEvent, DateRange, Event, EventCategory, EventFileUpload, IntervalsCredentials,
    IntervalsError, UpdateActivity, UpdateEvent, UploadActivity, UploadedActivities,
};
pub use ports::{
    ActivityFileIdentityExtractorPort, ActivityRepositoryPort,
    ActivityUploadOperationRepositoryPort, BoxFuture, EnrichedEvent, IntervalsApiPort,
    IntervalsSettingsPort, NoopActivityFileIdentityExtractor, NoopActivityRepository,
    NoopActivityUploadOperationRepository,
};
pub use service::{
    IntervalsConnectionError, IntervalsConnectionTester, IntervalsService, IntervalsUseCases,
};
pub use workout::{
    find_best_activity_match, parse_workout_doc, ActualWorkoutMatch, MatchedWorkoutInterval,
    ParsedWorkoutDoc, WorkoutIntervalDefinition, WorkoutSegment, WorkoutSummary,
};
