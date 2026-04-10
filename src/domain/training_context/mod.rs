mod model;
mod packing;
mod service;

pub use model::{
    AthleteProfileContext, FuturePlannedEventContext, IntervalsStatusContext, ProjectedDayContext,
    ProjectedWorkoutContext, RaceContext, RenderedTrainingContext, TrainingContext,
    TrainingContextBuildResult, WeeklyAvailabilityContext,
};
pub use packing::{approximate_token_count, render_training_context};
pub use service::{DefaultTrainingContextBuilder, TrainingContextBuilder};
