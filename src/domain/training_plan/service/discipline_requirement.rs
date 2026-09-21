use crate::domain::{
    intervals::{PlannedWorkoutLine, PlannedWorkoutTarget},
    llm_tools::duration_from_distance_meters,
    training_context::RaceContext,
    training_plan::TrainingPlanDay,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TargetEventRequirement {
    pub discipline: String,
    pub date: String,
    pub estimated_duration_seconds: i64,
    pub priority: String,
}

/// Returns a human-readable gap when the plan window fails the discipline session rule.
pub fn missing_discipline_requirement(
    days: &[TrainingPlanDay],
    target: &TargetEventRequirement,
) -> Option<String> {
    let discipline = target.discipline.trim().to_ascii_lowercase();
    if discipline == "timetrial" || target.estimated_duration_seconds <= 1_800 {
        return missing_timetrial_requirement(days, target);
    }
    match discipline.as_str() {
        "road" | "cyclocross" | "mtb" | "gravel" => {
            missing_repeatability_requirement(days, &discipline)
        }
        _ => None,
    }
}

pub fn select_target_event_requirement(
    races: &[RaceContext],
    today: &str,
) -> Option<TargetEventRequirement> {
    let upcoming = races.iter().filter(|race| race.date.as_str() >= today);
    let chosen = upcoming
        .clone()
        .filter(|race| race.priority.eq_ignore_ascii_case("A"))
        .min_by_key(|race| race.date.as_str())
        .or_else(|| upcoming.min_by_key(|race| race.date.as_str()))?;
    let estimated_duration_seconds =
        i64::from(duration_from_distance_meters(chosen.distance_meters).unwrap_or(0));
    Some(TargetEventRequirement {
        discipline: chosen.discipline.clone(),
        date: chosen.date.clone(),
        estimated_duration_seconds,
        priority: chosen.priority.clone(),
    })
}

fn missing_timetrial_requirement(
    days: &[TrainingPlanDay],
    target: &TargetEventRequirement,
) -> Option<String> {
    let min_seconds = ((target.estimated_duration_seconds as f64) * 0.6).ceil() as i32;
    let min_seconds = min_seconds.max(1);
    if has_continuous_near_threshold_step(days, min_seconds) {
        return None;
    }
    let min_minutes = (min_seconds + 59) / 60;
    Some(format!(
        "timetrial target on {} (~{}s estimated) requires a continuous step of at least {min_minutes}m at 88-96% FTP; short repeats do not count",
        target.date, target.estimated_duration_seconds
    ))
}

fn missing_repeatability_requirement(days: &[TrainingPlanDay], discipline: &str) -> Option<String> {
    if count_supra_threshold_efforts(days) >= 2 {
        return None;
    }
    Some(format!(
        "{discipline} target requires at least two efforts at >=100% FTP for repeatability work"
    ))
}

fn has_continuous_near_threshold_step(days: &[TrainingPlanDay], min_seconds: i32) -> bool {
    days.iter()
        .filter_map(|day| day.workout.as_ref())
        .flat_map(|workout| workout.lines.iter())
        .filter_map(PlannedWorkoutLine::step)
        .any(|step| {
            step.duration_seconds >= min_seconds && percent_overlaps_band(&step.target, 88.0, 96.0)
        })
}

fn count_supra_threshold_efforts(days: &[TrainingPlanDay]) -> usize {
    days.iter()
        .filter_map(|day| day.workout.as_ref())
        .flat_map(|workout| workout.lines.iter())
        .filter_map(PlannedWorkoutLine::step)
        .filter(|step| matches!(step.target, PlannedWorkoutTarget::PercentFtp { min, .. } if min >= 100.0))
        .count()
}

fn percent_overlaps_band(target: &PlannedWorkoutTarget, low: f64, high: f64) -> bool {
    match target {
        PlannedWorkoutTarget::PercentFtp { min, max } => *min <= high && *max >= low,
        PlannedWorkoutTarget::WattsRange { .. } => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        intervals::{
            parse_planned_workout_days, PlannedWorkout, PlannedWorkoutLine, PlannedWorkoutStep,
            PlannedWorkoutStepKind, PlannedWorkoutTarget, PlannedWorkoutText,
        },
        training_plan::TrainingPlanDay,
    };

    fn day_from_text(block: &str) -> TrainingPlanDay {
        let parsed = parse_planned_workout_days(block).expect("parse");
        let day = parsed.days.into_iter().next().expect("day");
        let date = day.date.clone();
        let rest_day = day.is_rest_day();
        let rest_day_reason = day.rest_day_reason().map(ToString::to_string);
        let workout = day.into_workout();
        TrainingPlanDay {
            date,
            rest_day,
            rest_day_reason,
            workout,
        }
    }

    fn tt_target() -> TargetEventRequirement {
        TargetEventRequirement {
            discipline: "timetrial".to_string(),
            date: "2026-10-25".to_string(),
            estimated_duration_seconds: 1_320,
            priority: "A".to_string(),
        }
    }

    #[test]
    fn timetrial_rejects_short_repeat_blocks() {
        let day = day_from_text(
            "2026-09-25\nTime-Trial Durability\nMain Set 2x\n- 2m 105-110%\n- 3m 88-92%\n",
        );
        let gap = missing_discipline_requirement(&[day], &tt_target());
        assert!(gap.is_some_and(|g| g.contains("continuous step")));
    }

    #[test]
    fn timetrial_accepts_continuous_fifteen_minute_threshold() {
        let day = day_from_text("2026-10-01\nTime-Trial Threshold\n- 15m 92%\n");
        assert!(missing_discipline_requirement(&[day], &tt_target()).is_none());
    }

    #[test]
    fn road_accepts_repeatability_work() {
        let day = day_from_text(
            "2026-09-25\nVO2 Repeatability\n- 2m 105%\n- 2m 65%\n- 2m 110%\n- 2m 65%\n",
        );
        let target = TargetEventRequirement {
            discipline: "road".to_string(),
            date: "2026-10-01".to_string(),
            estimated_duration_seconds: 7_200,
            priority: "A".to_string(),
        };
        assert!(missing_discipline_requirement(&[day], &target).is_none());
    }

    #[test]
    fn select_target_prefers_soonest_upcoming_a_race() {
        let races = vec![
            RaceContext {
                race_id: "b".into(),
                date: "2026-09-22".into(),
                name: "B".into(),
                distance_meters: 40_000,
                discipline: "road".into(),
                priority: "B".into(),
            },
            RaceContext {
                race_id: "a".into(),
                date: "2026-10-25".into(),
                name: "A TT".into(),
                distance_meters: 11_000,
                discipline: "timetrial".into(),
                priority: "A".into(),
            },
        ];
        let selected = select_target_event_requirement(&races, "2026-09-21").expect("target");
        assert_eq!(selected.discipline, "timetrial");
        assert_eq!(selected.date, "2026-10-25");
        assert_eq!(selected.estimated_duration_seconds, 1_320);
    }

    #[test]
    fn select_target_returns_none_when_no_upcoming_race() {
        let races = vec![RaceContext {
            race_id: "past".into(),
            date: "2026-09-01".into(),
            name: "Past".into(),
            distance_meters: 40_000,
            discipline: "road".into(),
            priority: "A".into(),
        }];
        assert!(select_target_event_requirement(&races, "2026-09-21").is_none());
    }

    #[test]
    fn short_event_without_timetrial_disc_still_needs_continuous_block() {
        let day = TrainingPlanDay {
            date: "2026-09-25".into(),
            rest_day: false,
            rest_day_reason: None,
            workout: Some(PlannedWorkout {
                lines: vec![
                    PlannedWorkoutLine::Text(PlannedWorkoutText {
                        text: "Threshold".into(),
                    }),
                    PlannedWorkoutLine::Step(PlannedWorkoutStep {
                        duration_seconds: 180,
                        kind: PlannedWorkoutStepKind::Steady,
                        target: PlannedWorkoutTarget::PercentFtp {
                            min: 92.0,
                            max: 95.0,
                        },
                    }),
                ],
            }),
        };
        let target = TargetEventRequirement {
            discipline: "road".to_string(),
            date: "2026-10-01".to_string(),
            estimated_duration_seconds: 1_200,
            priority: "A".to_string(),
        };
        assert!(missing_discipline_requirement(&[day], &target).is_some());
    }
}
