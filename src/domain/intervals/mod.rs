mod model;
mod planned_workout;
mod ports;
mod service;
mod workout;

pub use model::{
    build_activity_upload_operation_key, normalize_external_id, round_distance_bucket,
    round_duration_bucket, Activity, ActivityDeduplicationIdentity, ActivityDetails,
    ActivityFallbackIdentity, ActivityInterval, ActivityIntervalGroup, ActivityMetrics,
    ActivityStream, ActivityUploadOperation, ActivityUploadOperationClaimResult,
    ActivityUploadOperationStatus, ActivityZoneTime, CreateEvent, DateRange, Event, EventCategory,
    EventFileUpload, IntervalsCredentials, IntervalsError, UpdateActivity, UpdateEvent,
    UploadActivity, UploadedActivities,
};
pub use planned_workout::{
    parse_planned_workout, parse_planned_workout_days, serialize_planned_workout, PlannedWorkout,
    PlannedWorkoutDay, PlannedWorkoutDays, PlannedWorkoutLine, PlannedWorkoutParseError,
    PlannedWorkoutRepeat, PlannedWorkoutStep, PlannedWorkoutStepKind, PlannedWorkoutTarget,
    PlannedWorkoutText,
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
