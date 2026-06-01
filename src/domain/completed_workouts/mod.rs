mod authoritative;
mod model;
mod ports;
mod power_curve;
mod power_curve_repo;
mod selection;
mod service;

pub use selection::select_visible_workouts_by_day;
#[cfg(test)]
mod tests;

pub use authoritative::AuthoritativeCompletedWorkoutRepository;
pub use model::{
    CompletedWorkout, CompletedWorkoutDetails, CompletedWorkoutError, CompletedWorkoutInterval,
    CompletedWorkoutIntervalGroup, CompletedWorkoutMetrics, CompletedWorkoutPowerCurve,
    CompletedWorkoutSeries, CompletedWorkoutStream, CompletedWorkoutZoneTime,
};
pub use ports::{BoxFuture, CompletedWorkoutRepository};
pub use power_curve::{compute_power_curve, PowerCurveError};
pub use power_curve_repo::PowerCurveCompletedWorkoutRepository;
pub use service::{
    canonical_completed_workout_id, completed_workout_activity_id,
    BackfillCompletedWorkoutDetailsResult, BackfillCompletedWorkoutMetricsResult,
    CompletedWorkoutAdminUseCases, CompletedWorkoutReadService, CompletedWorkoutReadUseCases,
};
