use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use aiwattcoach::domain::{
    athlete_summary::{
        AthleteSummary, AthleteSummaryError, AthleteSummaryState, AthleteSummaryUseCases,
        EnsuredAthleteSummary,
    },
    completed_workouts::{
        CompletedWorkout, CompletedWorkoutDetails, CompletedWorkoutMetrics,
        CompletedWorkoutRepository, CompletedWorkoutSeries, CompletedWorkoutStream,
        CompletedWorkoutZoneTime,
    },
    intervals::{
        Activity, ActivityDetails, ActivityMetrics, ActivityStream, DateRange, Event,
        IntervalsError, IntervalsUseCases,
    },
    llm::{BoxFuture as LlmBoxFuture, LlmContextCache, LlmContextCacheRepository, LlmError},
    planned_workouts::{PlannedWorkout, PlannedWorkoutRepository},
    settings::{
        AiAgentsConfig, AnalysisOptions, AvailabilityDay, AvailabilitySettings,
        BoxFuture as SettingsBoxFuture, CyclingSettings, IntervalsConfig, SettingsError,
        UserSettings, UserSettingsRepository,
    },
    special_days::{SpecialDay, SpecialDayRepository},
    workout_summary::{
        BoxFuture as WorkoutBoxFuture, CoachReplyClaimResult, CoachReplyOperation,
        CoachReplyOperationRepository, CoachReplyOperationStatus, ConversationMessage,
        WorkoutRecap, WorkoutSummary, WorkoutSummaryError, WorkoutSummaryRepository,
    },
};
use chrono::Utc;

type SummaryKey = (String, String);
type ReplyOperationKey = (String, String, String);

mod athlete_summary;
mod cache;
mod intervals;
mod settings;
mod workout_summary;

pub(crate) use athlete_summary::InMemoryAthleteSummaryService;
pub(crate) use cache::InMemoryLlmContextCacheRepository;
pub(crate) use intervals::{sample_activity, InMemoryIntervalsService};
pub(crate) use settings::{ai_config, sample_user_settings, InMemoryUserSettingsRepository};
pub(crate) use workout_summary::{
    sample_summary, InMemoryCoachReplyOperationRepository, InMemoryWorkoutSummaryRepository,
};

#[derive(Clone, Default)]
pub(crate) struct InMemoryCompletedWorkoutRepository {
    workouts: Arc<Mutex<Vec<CompletedWorkout>>>,
}

impl InMemoryCompletedWorkoutRepository {
    pub(crate) fn seed(&self, workouts: Vec<CompletedWorkout>) {
        let mut stored = self.workouts.lock().unwrap();
        stored.clear();
        stored.extend(workouts);
    }
}

impl CompletedWorkoutRepository for InMemoryCompletedWorkoutRepository {
    fn list_by_user_id(
        &self,
        user_id: &str,
    ) -> aiwattcoach::domain::completed_workouts::BoxFuture<
        Result<
            Vec<CompletedWorkout>,
            aiwattcoach::domain::completed_workouts::CompletedWorkoutError,
        >,
    > {
        let workouts = self.workouts.lock().unwrap().clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            Ok(workouts
                .into_iter()
                .filter(|workout| workout.user_id == user_id)
                .collect())
        })
    }

    fn list_by_user_id_and_date_range(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> aiwattcoach::domain::completed_workouts::BoxFuture<
        Result<
            Vec<CompletedWorkout>,
            aiwattcoach::domain::completed_workouts::CompletedWorkoutError,
        >,
    > {
        let workouts = self.workouts.lock().unwrap().clone();
        let user_id = user_id.to_string();
        let oldest = oldest.to_string();
        let newest = newest.to_string();
        Box::pin(async move {
            Ok(workouts
                .into_iter()
                .filter(|workout| workout.user_id == user_id)
                .filter(|workout| {
                    let date = workout.start_date_local.get(..10).unwrap_or_default();
                    date >= oldest.as_str() && date <= newest.as_str()
                })
                .collect())
        })
    }

    fn upsert(
        &self,
        workout: CompletedWorkout,
    ) -> aiwattcoach::domain::completed_workouts::BoxFuture<
        Result<CompletedWorkout, aiwattcoach::domain::completed_workouts::CompletedWorkoutError>,
    > {
        let repository = self.clone();
        Box::pin(async move {
            let mut stored = repository.workouts.lock().unwrap();
            stored.retain(|existing| {
                !(existing.user_id == workout.user_id
                    && existing.completed_workout_id == workout.completed_workout_id)
            });
            stored.push(workout.clone());
            Ok(workout)
        })
    }
}

#[derive(Clone, Default)]
pub(crate) struct InMemoryPlannedWorkoutRepository;

impl PlannedWorkoutRepository for InMemoryPlannedWorkoutRepository {
    fn list_by_user_id(
        &self,
        _user_id: &str,
    ) -> aiwattcoach::domain::planned_workouts::BoxFuture<
        Result<Vec<PlannedWorkout>, aiwattcoach::domain::planned_workouts::PlannedWorkoutError>,
    > {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn list_by_user_id_and_date_range(
        &self,
        _user_id: &str,
        _oldest: &str,
        _newest: &str,
    ) -> aiwattcoach::domain::planned_workouts::BoxFuture<
        Result<Vec<PlannedWorkout>, aiwattcoach::domain::planned_workouts::PlannedWorkoutError>,
    > {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn upsert(
        &self,
        workout: PlannedWorkout,
    ) -> aiwattcoach::domain::planned_workouts::BoxFuture<
        Result<PlannedWorkout, aiwattcoach::domain::planned_workouts::PlannedWorkoutError>,
    > {
        Box::pin(async move { Ok(workout) })
    }
}

#[derive(Clone, Default)]
pub(crate) struct InMemorySpecialDayRepository;

impl SpecialDayRepository for InMemorySpecialDayRepository {
    fn list_by_user_id(
        &self,
        _user_id: &str,
    ) -> aiwattcoach::domain::special_days::BoxFuture<
        Result<Vec<SpecialDay>, aiwattcoach::domain::special_days::SpecialDayError>,
    > {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn list_by_user_id_and_date_range(
        &self,
        _user_id: &str,
        _oldest: &str,
        _newest: &str,
    ) -> aiwattcoach::domain::special_days::BoxFuture<
        Result<Vec<SpecialDay>, aiwattcoach::domain::special_days::SpecialDayError>,
    > {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn upsert(
        &self,
        special_day: SpecialDay,
    ) -> aiwattcoach::domain::special_days::BoxFuture<
        Result<SpecialDay, aiwattcoach::domain::special_days::SpecialDayError>,
    > {
        Box::pin(async move { Ok(special_day) })
    }
}

pub(crate) fn canonical_completed_workout_from_activity(activity: &Activity) -> CompletedWorkout {
    CompletedWorkout {
        completed_workout_id: format!("intervals-activity:{}", activity.id),
        user_id: activity.athlete_id.clone().unwrap_or_default(),
        start_date_local: activity.start_date_local.clone(),
        planned_workout_id: None,
        name: activity.name.clone(),
        description: activity.description.clone(),
        activity_type: activity.activity_type.clone(),
        duration_seconds: activity
            .elapsed_time_seconds
            .or(activity.moving_time_seconds),
        distance_meters: activity.distance_meters,
        metrics: CompletedWorkoutMetrics {
            training_stress_score: activity.metrics.training_stress_score,
            normalized_power_watts: activity.metrics.normalized_power_watts,
            intensity_factor: activity.metrics.intensity_factor,
            efficiency_factor: activity.metrics.efficiency_factor,
            variability_index: activity.metrics.variability_index,
            average_power_watts: activity.metrics.average_power_watts,
            ftp_watts: activity.metrics.ftp_watts,
            total_work_joules: activity.metrics.total_work_joules,
            calories: activity.metrics.calories,
            trimp: activity.metrics.trimp,
            power_load: activity.metrics.power_load,
            heart_rate_load: activity.metrics.heart_rate_load,
            pace_load: activity.metrics.pace_load,
            strain_score: activity.metrics.strain_score,
        },
        details: CompletedWorkoutDetails {
            intervals: activity
                .details
                .intervals
                .iter()
                .map(
                    |interval| aiwattcoach::domain::completed_workouts::CompletedWorkoutInterval {
                        id: interval.id,
                        label: interval.label.clone(),
                        interval_type: interval.interval_type.clone(),
                        group_id: interval.group_id.clone(),
                        start_index: interval.start_index,
                        end_index: interval.end_index,
                        start_time_seconds: interval.start_time_seconds,
                        end_time_seconds: interval.end_time_seconds,
                        moving_time_seconds: interval.moving_time_seconds,
                        elapsed_time_seconds: interval.elapsed_time_seconds,
                        distance_meters: interval.distance_meters,
                        average_power_watts: interval.average_power_watts,
                        normalized_power_watts: interval.normalized_power_watts,
                        training_stress_score: interval.training_stress_score,
                        average_heart_rate_bpm: interval.average_heart_rate_bpm,
                        average_cadence_rpm: interval.average_cadence_rpm,
                        average_speed_mps: interval.average_speed_mps,
                        average_stride_meters: interval.average_stride_meters,
                        zone: interval.zone,
                    },
                )
                .collect(),
            interval_groups: activity
                .details
                .interval_groups
                .iter()
                .map(|group| {
                    aiwattcoach::domain::completed_workouts::CompletedWorkoutIntervalGroup {
                        id: group.id.clone(),
                        count: group.count,
                        start_index: group.start_index,
                        moving_time_seconds: group.moving_time_seconds,
                        elapsed_time_seconds: group.elapsed_time_seconds,
                        distance_meters: group.distance_meters,
                        average_power_watts: group.average_power_watts,
                        normalized_power_watts: group.normalized_power_watts,
                        training_stress_score: group.training_stress_score,
                        average_heart_rate_bpm: group.average_heart_rate_bpm,
                        average_cadence_rpm: group.average_cadence_rpm,
                        average_speed_mps: group.average_speed_mps,
                        average_stride_meters: group.average_stride_meters,
                    }
                })
                .collect(),
            streams: activity
                .details
                .streams
                .iter()
                .map(|stream| CompletedWorkoutStream {
                    stream_type: stream.stream_type.clone(),
                    name: stream.name.clone(),
                    primary_series: stream.data.as_ref().and_then(map_series),
                    secondary_series: stream.data2.as_ref().and_then(map_series),
                    value_type_is_array: stream.value_type_is_array,
                    custom: stream.custom,
                    all_null: stream.all_null,
                })
                .collect(),
            interval_summary: activity.details.interval_summary.clone(),
            skyline_chart: activity.details.skyline_chart.clone(),
            power_zone_times: activity
                .details
                .power_zone_times
                .iter()
                .map(|zone| CompletedWorkoutZoneTime {
                    zone_id: zone.zone_id.clone(),
                    seconds: zone.seconds,
                })
                .collect(),
            heart_rate_zone_times: activity.details.heart_rate_zone_times.clone(),
            pace_zone_times: activity.details.pace_zone_times.clone(),
            gap_zone_times: activity.details.gap_zone_times.clone(),
        },
    }
}

fn map_series(value: &serde_json::Value) -> Option<CompletedWorkoutSeries> {
    let serde_json::Value::Array(items) = value else {
        return None;
    };

    if items.iter().all(|item| item.as_i64().is_some()) {
        return Some(CompletedWorkoutSeries::Integers(
            items.iter().filter_map(|item| item.as_i64()).collect(),
        ));
    }

    if items.iter().all(|item| item.as_f64().is_some()) {
        return Some(CompletedWorkoutSeries::Floats(
            items.iter().filter_map(|item| item.as_f64()).collect(),
        ));
    }

    None
}
