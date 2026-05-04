mod model;
mod packing;
mod service;

pub use model::{
    AthleteProfileContext, FuturePlannedEventContext, IntervalsStatusContext,
    PlannedWorkoutContext, ProjectedDayContext, ProjectedWorkoutContext, RaceContext,
    RecentDayContext, RecentWorkoutContext, RenderedTrainingContext, SpecialDayContext,
    TrainingContext, TrainingContextBuildResult, UpcomingDayContext, WeeklyAvailabilityContext,
};
pub use packing::{approximate_token_count, render_training_context};
pub use service::{
    DefaultTrainingContextBuilder, TrainingContextBuilder, ATHLETE_SUMMARY_FOCUS_ID,
    CALENDAR_OVERVIEW_FOCUS_ID,
};
