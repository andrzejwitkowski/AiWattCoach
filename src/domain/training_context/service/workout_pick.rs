use std::collections::{HashMap, HashSet};

use crate::domain::{
    completed_workouts::{select_visible_workouts_by_day, CompletedWorkout},
    intervals::{find_best_activity_match, parse_workout_doc, Activity, Event},
    planned_workouts::PlannedWorkout,
    special_days::SpecialDay,
};

use super::{
    build_direct_event_matches, build_event_activity_matches, build_local_events,
    map_completed_workout_to_activity,
};

#[derive(Clone, Debug, PartialEq)]
pub struct DayWorkoutPick {
    pub workout: CompletedWorkout,
    pub method: DayWorkoutPickMethod,
    pub compliance_score: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DayWorkoutPickMethod {
    SingleWorkout,
    ComplianceMatch,
    TrainingStressFallback,
    LatestStartFallback,
}

impl DayWorkoutPickMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SingleWorkout => "single_workout",
            Self::ComplianceMatch => "compliance_match",
            Self::TrainingStressFallback => "tss_fallback",
            Self::LatestStartFallback => "latest_start_fallback",
        }
    }
}

pub fn pick_representative_completed_workout_for_day(
    day_workouts: Vec<CompletedWorkout>,
    wahoo_entity_ids: &HashSet<String>,
    planned_workouts: &[PlannedWorkout],
    special_days: &[SpecialDay],
    configured_ftp: Option<i32>,
) -> Option<DayWorkoutPick> {
    let visible = select_visible_workouts_by_day(day_workouts, wahoo_entity_ids);
    pick_from_visible_workouts(&visible, planned_workouts, special_days, configured_ftp)
}

fn pick_from_visible_workouts(
    visible: &[CompletedWorkout],
    planned_workouts: &[PlannedWorkout],
    special_days: &[SpecialDay],
    configured_ftp: Option<i32>,
) -> Option<DayWorkoutPick> {
    match visible.len() {
        0 => None,
        1 => Some(DayWorkoutPick {
            workout: visible[0].clone(),
            method: DayWorkoutPickMethod::SingleWorkout,
            compliance_score: None,
        }),
        _ => pick_best_among_visible(visible, planned_workouts, special_days, configured_ftp),
    }
}

fn pick_best_among_visible(
    visible: &[CompletedWorkout],
    planned_workouts: &[PlannedWorkout],
    special_days: &[SpecialDay],
    configured_ftp: Option<i32>,
) -> Option<DayWorkoutPick> {
    let events: Vec<Event> = build_local_events(planned_workouts, special_days);
    let activities: Vec<Activity> = visible
        .iter()
        .map(map_completed_workout_to_activity)
        .collect();
    let planned_events_by_id = events
        .iter()
        .map(|event| (event.id.to_string(), event.clone()))
        .collect::<HashMap<_, _>>();
    let direct_matches = build_direct_event_matches(visible, &planned_events_by_id);
    let matches =
        build_event_activity_matches(&events, &activities, &direct_matches, configured_ftp);

    let mut best: Option<(f64, CompletedWorkout)> = None;
    for workout in visible {
        let activity_id = map_completed_workout_to_activity(workout).id;
        let Some(event) = matches.activity_to_event.get(&activity_id) else {
            continue;
        };
        let effective_ftp = workout.metrics.ftp_watts.or(configured_ftp);
        let parsed = parse_workout_doc(event.structured_workout_text(), effective_ftp);
        let activity = map_completed_workout_to_activity(workout);
        let Some(candidate) =
            find_best_activity_match(&parsed, std::slice::from_ref(&activity), effective_ftp)
        else {
            continue;
        };
        if candidate.compliance_score < 0.45 {
            continue;
        }
        if best
            .as_ref()
            .is_none_or(|(score, _)| candidate.compliance_score > *score)
        {
            best = Some((candidate.compliance_score, workout.clone()));
        }
    }

    if let Some((score, workout)) = best {
        return Some(DayWorkoutPick {
            workout,
            method: DayWorkoutPickMethod::ComplianceMatch,
            compliance_score: Some(score),
        });
    }

    let tss_winner = visible.iter().max_by(|left, right| {
        left.metrics
            .training_stress_score
            .unwrap_or(0)
            .cmp(&right.metrics.training_stress_score.unwrap_or(0))
            .then_with(|| left.start_date_local.cmp(&right.start_date_local))
    })?;
    if tss_winner.metrics.training_stress_score.is_some() {
        return Some(DayWorkoutPick {
            workout: tss_winner.clone(),
            method: DayWorkoutPickMethod::TrainingStressFallback,
            compliance_score: None,
        });
    }

    let latest = visible
        .iter()
        .max_by(|left, right| left.start_date_local.cmp(&right.start_date_local))?;
    Some(DayWorkoutPick {
        workout: latest.clone(),
        method: DayWorkoutPickMethod::LatestStartFallback,
        compliance_score: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::completed_workouts::{CompletedWorkoutDetails, CompletedWorkoutMetrics};

    fn sample_workout(id: &str, day: &str, tss: Option<i32>) -> CompletedWorkout {
        CompletedWorkout::new(
            id.to_string(),
            "user-1".to_string(),
            format!("{day}T10:00:00"),
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
                training_stress_score: tss,
                normalized_power_watts: None,
                intensity_factor: None,
                efficiency_factor: None,
                variability_index: None,
                average_power_watts: None,
                ftp_watts: Some(250),
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

    #[test]
    fn single_visible_workout_uses_single_method() {
        let workout = sample_workout("ride-1", "2026-05-01", Some(50));
        let pick = pick_from_visible_workouts(std::slice::from_ref(&workout), &[], &[], Some(250))
            .expect("pick");

        assert_eq!(pick.method, DayWorkoutPickMethod::SingleWorkout);
        assert_eq!(pick.workout.completed_workout_id, "ride-1");
    }

    #[test]
    fn tss_fallback_prefers_higher_stress_when_no_compliance_match() {
        let low = sample_workout("ride-low", "2026-05-01", Some(30));
        let high = sample_workout("ride-high", "2026-05-01", Some(90));
        let pick = pick_from_visible_workouts(&[low, high], &[], &[], Some(250)).expect("pick");

        assert_eq!(pick.method, DayWorkoutPickMethod::TrainingStressFallback);
        assert_eq!(pick.workout.completed_workout_id, "ride-high");
    }
}
