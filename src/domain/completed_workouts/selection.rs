use std::collections::{BTreeMap, HashSet};

use super::{CompletedWorkout, CompletedWorkoutSeries, CompletedWorkoutStream};

pub fn has_power_details(workout: &CompletedWorkout) -> bool {
    workout.details.streams.iter().any(stream_has_power_samples)
}

pub fn select_visible_workouts_by_day(
    workouts: Vec<CompletedWorkout>,
    wahoo_entity_ids: &HashSet<String>,
) -> Vec<CompletedWorkout> {
    let mut grouped = BTreeMap::<String, Vec<CompletedWorkout>>::new();
    let mut undated = Vec::new();

    for workout in workouts {
        let Some(day) = workout.start_date_local.get(..10) else {
            undated.push(workout);
            continue;
        };
        grouped.entry(day.to_string()).or_default().push(workout);
    }

    let mut visible = grouped
        .into_values()
        .flat_map(|day_workouts| select_day_bucket(day_workouts, wahoo_entity_ids))
        .collect::<Vec<_>>();
    visible.extend(undated);
    visible
}

fn select_day_bucket(
    day_workouts: Vec<CompletedWorkout>,
    wahoo_entity_ids: &HashSet<String>,
) -> Vec<CompletedWorkout> {
    let (wahoo, other): (Vec<_>, Vec<_>) = day_workouts
        .into_iter()
        .partition(|workout| wahoo_entity_ids.contains(&workout.completed_workout_id));

    if wahoo.iter().any(has_power_details) {
        return wahoo;
    }
    if other.iter().any(has_power_details) {
        return other;
    }
    if !wahoo.is_empty() {
        return wahoo;
    }

    other
}

fn stream_has_power_samples(stream: &CompletedWorkoutStream) -> bool {
    stream.stream_type.eq_ignore_ascii_case("watts")
        && !stream.all_null
        && (series_has_samples(stream.primary_series.as_ref())
            || series_has_samples(stream.secondary_series.as_ref()))
}

fn series_has_samples(series: Option<&CompletedWorkoutSeries>) -> bool {
    match series {
        Some(CompletedWorkoutSeries::Integers(values)) => !values.is_empty(),
        Some(CompletedWorkoutSeries::Floats(values)) => !values.is_empty(),
        Some(CompletedWorkoutSeries::Bools(values)) => !values.is_empty(),
        Some(CompletedWorkoutSeries::Strings(values)) => !values.is_empty(),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{has_power_details, select_visible_workouts_by_day};
    use crate::domain::completed_workouts::{
        CompletedWorkout, CompletedWorkoutDetails, CompletedWorkoutMetrics, CompletedWorkoutSeries,
        CompletedWorkoutStream,
    };

    fn sample_workout(id: &str, day: &str) -> CompletedWorkout {
        CompletedWorkout::new(
            id.to_string(),
            "user-1".to_string(),
            format!("{day}T08:00:00"),
            Some(id.to_string()),
            None,
            Some(id.to_string()),
            None,
            Some("Ride".to_string()),
            None,
            false,
            Some(3600),
            Some(1000.0),
            CompletedWorkoutMetrics {
                training_stress_score: Some(10),
                normalized_power_watts: None,
                intensity_factor: None,
                efficiency_factor: None,
                variability_index: None,
                average_power_watts: None,
                ftp_watts: None,
                total_work_joules: None,
                calories: None,
                trimp: None,
                power_load: None,
                heart_rate_load: None,
                pace_load: None,
                strain_score: None,
            },
            CompletedWorkoutDetails {
                intervals: Vec::new(),
                interval_groups: Vec::new(),
                streams: Vec::new(),
                interval_summary: Vec::new(),
                skyline_chart: Vec::new(),
                power_zone_times: Vec::new(),
                heart_rate_zone_times: Vec::new(),
                pace_zone_times: Vec::new(),
                gap_zone_times: Vec::new(),
            },
            None,
        )
    }

    fn with_power_stream(mut workout: CompletedWorkout) -> CompletedWorkout {
        workout.details.streams.push(CompletedWorkoutStream {
            stream_type: "watts".to_string(),
            name: Some("Power".to_string()),
            primary_series: Some(CompletedWorkoutSeries::Integers(vec![180, 240, 310])),
            secondary_series: None,
            value_type_is_array: false,
            custom: false,
            all_null: false,
        });
        workout
    }

    #[test]
    fn has_power_details_requires_non_empty_watts_stream() {
        assert!(has_power_details(&with_power_stream(sample_workout(
            "completed-1",
            "2026-05-01",
        ))));
        assert!(!has_power_details(&sample_workout(
            "completed-1",
            "2026-05-01"
        )));
    }

    #[test]
    fn has_power_details_ignores_all_null_watts_stream() {
        let mut workout = with_power_stream(sample_workout("completed-1", "2026-05-01"));
        workout.details.streams[0].all_null = true;

        assert!(!has_power_details(&workout));
    }

    #[test]
    fn prefers_wahoo_when_wahoo_has_power_details() {
        let wahoo = with_power_stream(sample_workout("wahoo-workout:1", "2026-05-01"));
        let intervals = with_power_stream(sample_workout("intervals-activity:1", "2026-05-01"));

        let visible = select_visible_workouts_by_day(
            vec![intervals, wahoo.clone()],
            &HashSet::from([wahoo.completed_workout_id.clone()]),
        );

        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].completed_workout_id, wahoo.completed_workout_id);
    }

    #[test]
    fn prefers_other_provider_when_wahoo_lacks_power_details() {
        let wahoo = sample_workout("wahoo-workout:1", "2026-05-01");
        let intervals = with_power_stream(sample_workout("intervals-activity:1", "2026-05-01"));

        let visible = select_visible_workouts_by_day(
            vec![intervals.clone(), wahoo],
            &HashSet::from(["wahoo-workout:1".to_string()]),
        );

        assert_eq!(visible.len(), 1);
        assert_eq!(
            visible[0].completed_workout_id,
            intervals.completed_workout_id
        );
    }

    #[test]
    fn prefers_wahoo_when_nobody_has_power_details() {
        let wahoo = sample_workout("wahoo-workout:1", "2026-05-01");
        let intervals = sample_workout("intervals-activity:1", "2026-05-01");

        let visible = select_visible_workouts_by_day(
            vec![intervals, wahoo.clone()],
            &HashSet::from([wahoo.completed_workout_id.clone()]),
        );

        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].completed_workout_id, wahoo.completed_workout_id);
    }

    #[test]
    fn keeps_days_independent() {
        let wahoo_day_one = sample_workout("wahoo-workout:1", "2026-05-01");
        let intervals_day_one =
            with_power_stream(sample_workout("intervals-activity:1", "2026-05-01"));
        let intervals_day_two =
            with_power_stream(sample_workout("intervals-activity:2", "2026-05-02"));
        let intervals_day_one_id = intervals_day_one.completed_workout_id.clone();

        let visible = select_visible_workouts_by_day(
            vec![intervals_day_one, wahoo_day_one, intervals_day_two.clone()],
            &HashSet::from(["wahoo-workout:1".to_string()]),
        );

        assert_eq!(visible.len(), 2);
        assert!(visible
            .iter()
            .any(|workout| workout.completed_workout_id == intervals_day_one_id));
        assert!(visible
            .iter()
            .any(|workout| workout.completed_workout_id == intervals_day_two.completed_workout_id));
    }
}
