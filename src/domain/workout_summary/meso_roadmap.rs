use serde_json::json;

use crate::domain::{
    intervals::serialize_planned_workout,
    meso_cycle::{MesoCycleProjectedDay, MesoCycleProjectionRepository},
};

pub const MESO_CYCLE_ROADMAP_GUIDANCE: &str = "This section is a predicted, suggested mesocycle roadmap for upcoming training. It may change as the athlete trains and plans evolve. It was created as general strategic orientation and help, not a rigid schedule the athlete must follow or that you must defend. Do not cite it as current vacation, configured rest, or proof the athlete has free time.";

pub fn format_meso_roadmap_stable_context(days: &[MesoCycleProjectedDay]) -> Option<String> {
    if days.is_empty() {
        return None;
    }

    let mut sorted = days.to_vec();
    sorted.sort_by(|left, right| left.date.cmp(&right.date));

    let window_start = sorted.first()?.date.clone();
    let window_end = sorted.last()?.date.clone();
    let day_entries = sorted
        .iter()
        .map(|day| {
            let mut entry = json!({
                "date": day.date,
                "restDay": day.rest_day,
            });
            if let Some(reason) = &day.rest_day_reason {
                entry["restDayReason"] = json!(reason);
            }
            if let Some(name) = projected_workout_name(day) {
                entry["name"] = json!(name);
            }
            if let Some(workout) = &day.workout {
                entry["rawWorkoutDoc"] = json!(serialize_planned_workout(workout));
            }
            entry
        })
        .collect::<Vec<_>>();

    let roadmap = json!({
        "windowStart": window_start,
        "windowEnd": window_end,
        "days": day_entries,
    });
    let roadmap_json = serde_json::to_string(&roadmap).ok()?;

    Some(format!(
        "meso_cycle_roadmap_guidance={MESO_CYCLE_ROADMAP_GUIDANCE}\nmeso_cycle_roadmap={roadmap_json}"
    ))
}

pub async fn try_load_meso_roadmap_stable_context(
    repository: &dyn MesoCycleProjectionRepository,
    user_id: &str,
) -> Option<String> {
    let days = repository.list_active_by_user_id(user_id).await.ok()?;
    format_meso_roadmap_stable_context(&days)
}

fn projected_workout_name(day: &MesoCycleProjectedDay) -> Option<String> {
    if day.rest_day {
        return Some("Rest Day".to_string());
    }

    day.workout.as_ref().and_then(|workout| {
        workout
            .lines
            .iter()
            .find_map(|line| line.text().map(ToString::to_string))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::intervals::{PlannedWorkout, PlannedWorkoutLine, PlannedWorkoutText};

    #[test]
    fn format_meso_roadmap_includes_guidance_and_sorted_days() {
        let context = format_meso_roadmap_stable_context(&[
            MesoCycleProjectedDay {
                user_id: "user-1".to_string(),
                operation_key: "meso-cycle:user-1".to_string(),
                date: "2026-06-10".to_string(),
                rest_day: false,
                rest_day_reason: None,
                workout: Some(PlannedWorkout {
                    lines: vec![PlannedWorkoutLine::Text(PlannedWorkoutText {
                        text: "Endurance Ride".to_string(),
                    })],
                }),
                superseded_at_epoch_seconds: None,
                created_at_epoch_seconds: 1,
                updated_at_epoch_seconds: 1,
            },
            MesoCycleProjectedDay {
                user_id: "user-1".to_string(),
                operation_key: "meso-cycle:user-1".to_string(),
                date: "2026-06-09".to_string(),
                rest_day: true,
                rest_day_reason: Some("Recovery".to_string()),
                workout: None,
                superseded_at_epoch_seconds: None,
                created_at_epoch_seconds: 1,
                updated_at_epoch_seconds: 1,
            },
        ])
        .expect("roadmap context");

        assert!(context.contains("meso_cycle_roadmap_guidance="));
        assert!(context.contains("predicted, suggested mesocycle roadmap"));
        assert!(context.contains(r#""windowStart":"2026-06-09""#));
        assert!(context.contains(r#""windowEnd":"2026-06-10""#));
        assert!(context.contains(r#""restDayReason":"Recovery""#));
        assert!(context.contains(r#""name":"Endurance Ride""#));
    }

    #[test]
    fn format_meso_roadmap_returns_none_for_empty_days() {
        assert!(format_meso_roadmap_stable_context(&[]).is_none());
    }
}
