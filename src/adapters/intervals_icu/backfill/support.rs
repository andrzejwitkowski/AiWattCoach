use crate::domain::{
    completed_workouts::{CompletedWorkout, CompletedWorkoutDetails},
    intervals::Activity,
};

pub(super) fn needs_detail_backfill(workout: &CompletedWorkout) -> bool {
    workout.source_activity_id.is_some()
        && workout.details_unavailable_reason.is_none()
        && !has_any_backfillable_details(&workout.details)
}

pub(super) fn needs_metric_backfill(workout: &CompletedWorkout) -> bool {
    workout.source_activity_id.is_some()
        && (workout.metrics.training_stress_score.is_none()
            || workout.metrics.ftp_watts.is_none()
            || workout.metrics.normalized_power_watts.is_none())
}

pub(super) fn has_backfillable_metrics(activity: &Activity) -> bool {
    activity.metrics.training_stress_score.is_some()
        || activity.metrics.ftp_watts.is_some()
        || activity.metrics.normalized_power_watts.is_some()
}

pub(super) fn earliest_recompute_date<'a>(
    workout: &'a CompletedWorkout,
    activity: &'a Activity,
) -> Option<&'a str> {
    match (
        workout.start_date_local.get(..10),
        activity.start_date_local.get(..10),
    ) {
        (Some(existing), Some(imported)) => Some(std::cmp::min(existing, imported)),
        (Some(existing), None) => Some(existing),
        (None, Some(imported)) => Some(imported),
        (None, None) => None,
    }
}

pub(super) fn oldest_workout_date(workouts: &[CompletedWorkout]) -> Option<String> {
    workouts
        .iter()
        .filter_map(|workout| workout.start_date_local.get(..10))
        .min()
        .map(ToString::to_string)
}

pub(super) fn has_backfillable_activity_details(activity: &Activity) -> bool {
    !activity.details.streams.is_empty()
        || !activity.details.intervals.is_empty()
        || !activity.details.interval_groups.is_empty()
        || !activity.details.interval_summary.is_empty()
        || !activity.details.skyline_chart.is_empty()
        || !activity.details.power_zone_times.is_empty()
        || !activity.details.heart_rate_zone_times.is_empty()
        || !activity.details.pace_zone_times.is_empty()
        || !activity.details.gap_zone_times.is_empty()
}

pub(super) fn has_any_backfillable_details(details: &CompletedWorkoutDetails) -> bool {
    !details.streams.is_empty()
        || !details.intervals.is_empty()
        || !details.interval_groups.is_empty()
        || !details.interval_summary.is_empty()
        || !details.skyline_chart.is_empty()
        || !details.power_zone_times.is_empty()
        || !details.heart_rate_zone_times.is_empty()
        || !details.pace_zone_times.is_empty()
        || !details.gap_zone_times.is_empty()
}
