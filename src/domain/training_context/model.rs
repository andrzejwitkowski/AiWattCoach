use crate::domain::settings::Weekday;

pub struct RedactedTrainingContextDebug<'a>(&'a TrainingContext);

impl std::fmt::Debug for RedactedTrainingContextDebug<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let context = self.0;
        f.debug_struct("TrainingContext")
            .field(
                "generated_at_epoch_seconds",
                &context.generated_at_epoch_seconds,
            )
            .field("focus_workout_present", &context.focus_workout_id.is_some())
            .field("focus_kind", &context.focus_kind)
            .field("races", &context.races.len())
            .field("planned_rest_days", &context.planned_rest_days.len())
            .field("future_events", &context.future_events.len())
            .field("recent_days", &context.recent_days.len())
            .field(
                "recent_workout_recaps",
                &context.recent_workout_recaps.len(),
            )
            .field("upcoming_days", &context.upcoming_days.len())
            .field("projected_days", &context.projected_days.len())
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderedTrainingContext {
    pub stable_context: String,
    pub volatile_context: String,
    pub approximate_tokens: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TrainingContextBuildResult {
    pub focus_date: String,
    pub context: TrainingContext,
    pub rendered: RenderedTrainingContext,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct TrainingContext {
    pub generated_at_epoch_seconds: i64,
    pub focus_workout_id: Option<String>,
    pub focus_kind: String,
    pub intervals_status: IntervalsStatusContext,
    pub profile: AthleteProfileContext,
    pub races: Vec<RaceContext>,
    pub planned_rest_days: Vec<PlannedRestDayContext>,
    pub future_events: Vec<FuturePlannedEventContext>,
    pub history: HistoricalTrainingContext,
    pub recent_days: Vec<RecentDayContext>,
    pub recent_workout_recaps: Vec<RecentWorkoutRecapContext>,
    pub upcoming_days: Vec<UpcomingDayContext>,
    pub projected_days: Vec<ProjectedDayContext>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecentWorkoutRecapContext {
    pub date: String,
    pub workout_id: String,
    pub rpe: Option<u8>,
    pub recap: String,
}

impl TrainingContext {
    pub fn redacted_debug(&self) -> RedactedTrainingContextDebug<'_> {
        RedactedTrainingContextDebug(self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct PlannedRestDayContext {
    pub planned_rest_day_id: String,
    pub start_date: String,
    pub end_date: String,
    pub title: Option<String>,
    pub note: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct RaceContext {
    pub race_id: String,
    pub date: String,
    pub name: String,
    pub distance_meters: i32,
    pub discipline: String,
    pub priority: String,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct FuturePlannedEventContext {
    pub event_id: i64,
    pub start_date_local: String,
    pub category: String,
    pub event_type: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub estimated_duration_seconds: Option<i32>,
    pub estimated_training_stress_score: Option<f64>,
    pub estimated_intensity_factor: Option<f64>,
    pub estimated_normalized_power_watts: Option<i32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct IntervalsStatusContext {
    pub activities: String,
    pub events: String,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct AthleteProfileContext {
    pub full_name: Option<String>,
    pub age: Option<u32>,
    pub height_cm: Option<u32>,
    pub weight_kg: Option<f64>,
    pub ftp_watts: Option<u32>,
    pub hr_max_bpm: Option<u32>,
    pub vo2_max: Option<f64>,
    pub athlete_prompt: Option<String>,
    pub medications: Option<String>,
    pub athlete_notes: Option<String>,
    pub availability_configured: bool,
    pub weekly_availability: Vec<WeeklyAvailabilityContext>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct WeeklyAvailabilityContext {
    pub weekday: Weekday,
    pub available: bool,
    pub max_duration_minutes: Option<u16>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct HistoricalTrainingContext {
    pub window_start: String,
    pub window_end: String,
    pub activity_count: usize,
    pub total_tss: i32,
    pub ctl: Option<f64>,
    pub atl: Option<f64>,
    pub tsb: Option<f64>,
    pub ftp_current: Option<i32>,
    pub ftp_change: Option<i32>,
    pub average_tss_7d: Option<f64>,
    pub average_tss_28d: Option<f64>,
    pub average_if_28d: Option<f64>,
    pub average_ef_28d: Option<f64>,
    pub load_trend: Vec<HistoricalLoadTrendPoint>,
    pub workouts: Vec<HistoricalWorkoutContext>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct HistoricalLoadTrendPoint {
    pub date: String,
    pub sample_days: u8,
    pub period_tss: i32,
    pub rolling_tss_7d: Option<f64>,
    pub rolling_tss_28d: Option<f64>,
    pub ctl: Option<f64>,
    pub atl: Option<f64>,
    pub tsb: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct HistoricalWorkoutContext {
    pub date: String,
    pub activity_id: String,
    pub name: Option<String>,
    pub activity_type: Option<String>,
    pub duration_seconds: Option<i32>,
    pub training_stress_score: Option<i32>,
    pub intensity_factor: Option<f64>,
    pub efficiency_factor: Option<f64>,
    pub normalized_power_watts: Option<i32>,
    pub ftp_watts: Option<i32>,
    pub workout_recap: Option<String>,
    pub variability_index: Option<f64>,
    pub power_segments: Vec<[i32; 3]>,
    pub interval_blocks: Vec<PlannedWorkoutBlockContext>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct RecentDayContext {
    pub date: String,
    pub free_day: bool,
    pub sick_day: bool,
    pub sick_note: Option<String>,
    pub workouts: Vec<RecentWorkoutContext>,
    pub planned_workouts: Vec<PlannedWorkoutContext>,
    pub special_days: Vec<SpecialDayContext>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct RecentWorkoutContext {
    pub activity_id: String,
    pub start_date_local: String,
    pub name: Option<String>,
    pub activity_type: Option<String>,
    pub training_stress_score: Option<i32>,
    pub efficiency_factor: Option<f64>,
    pub intensity_factor: Option<f64>,
    pub normalized_power_watts: Option<i32>,
    pub ftp_watts: Option<i32>,
    pub rpe: Option<u8>,
    pub workout_recap: Option<String>,
    pub variability_index: Option<f64>,
    pub power_segments: Vec<[i32; 3]>,
    pub cadence_segments: Vec<[i32; 3]>,
    pub planned_workout: Option<PlannedWorkoutReference>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct PlannedWorkoutReference {
    pub event_id: i64,
    pub start_date_local: String,
    pub name: Option<String>,
    pub category: String,
    pub interval_blocks: Vec<PlannedWorkoutBlockContext>,
    pub raw_workout_doc: Option<String>,
    pub estimated_training_stress_score: Option<f64>,
    pub estimated_intensity_factor: Option<f64>,
    pub estimated_normalized_power_watts: Option<i32>,
    pub completed: bool,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct PlannedWorkoutContext {
    pub event_id: i64,
    pub start_date_local: String,
    pub name: Option<String>,
    pub category: String,
    pub interval_blocks: Vec<PlannedWorkoutBlockContext>,
    pub raw_workout_doc: Option<String>,
    pub estimated_training_stress_score: Option<f64>,
    pub estimated_intensity_factor: Option<f64>,
    pub estimated_normalized_power_watts: Option<i32>,
    pub completed: bool,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct PlannedWorkoutBlockContext {
    pub duration_seconds: i32,
    pub min_percent_ftp: Option<f64>,
    pub max_percent_ftp: Option<f64>,
    pub min_target_watts: Option<i32>,
    pub max_target_watts: Option<i32>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct SpecialDayContext {
    pub event_id: i64,
    pub start_date_local: String,
    pub name: Option<String>,
    pub category: String,
    pub description: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct UpcomingDayContext {
    pub date: String,
    pub free_day: bool,
    pub planned_workouts: Vec<PlannedWorkoutContext>,
    pub special_days: Vec<SpecialDayContext>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct ProjectedDayContext {
    pub date: String,
    pub workouts: Vec<ProjectedWorkoutContext>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct ProjectedWorkoutContext {
    pub source_workout_id: String,
    pub start_date_local: String,
    pub name: Option<String>,
    pub interval_blocks: Vec<PlannedWorkoutBlockContext>,
    pub raw_workout_doc: Option<String>,
    pub rest_day: bool,
    pub rest_day_reason: Option<String>,
}
