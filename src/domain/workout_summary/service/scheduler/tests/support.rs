pub(super) use super::task_support::{
    InMemoryTaskRepository, InMemoryTaskWorkerRepository, TestClock, TestIdGenerator,
};
pub(super) use super::workout_summary_support::{
    direct_service, direct_service_with_athlete_summary, direct_service_with_operation_repository,
    existing_summary, BlockingCoach, InMemoryCoachReplyOperationRepository,
    InMemoryWorkoutSummaryRepository, TestCoach,
};
