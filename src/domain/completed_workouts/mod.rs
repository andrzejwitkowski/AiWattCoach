mod model;
mod ports;
#[cfg(test)]
mod tests;

pub use model::CompletedWorkout;
pub use model::{
    CompletedWorkoutDetails, CompletedWorkoutInterval, CompletedWorkoutIntervalGroup,
    CompletedWorkoutMetrics, CompletedWorkoutStream, CompletedWorkoutZoneTime,
};
pub use ports::{BoxFuture, CompletedWorkoutRepository};
