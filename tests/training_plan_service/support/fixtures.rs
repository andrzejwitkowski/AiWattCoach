use chrono::TimeZone;

use super::{
    AttemptRecord, TrainingPlanGenerationOperation, TrainingPlanProjectedDay, TrainingPlanSnapshot,
    ValidationIssue, WorkflowPhase, WorkflowStatus, WorkoutRecap, FIRST_DAY, MODEL, SECOND_DAY,
    USER_ID, WORKOUT_ID,
};

pub(crate) fn workout_recap() -> WorkoutRecap {
    WorkoutRecap::generated(
        "Steady aerobic ride with moderate fatigue.",
        "openrouter",
        MODEL,
        date_epoch(FIRST_DAY),
    )
}

pub(crate) fn valid_plan_window(start_date: &str) -> String {
    (0..14)
        .map(|offset| {
            let date = add_days(start_date, offset);
            if offset % 4 == 0 {
                format!("{date}\nRest Day")
            } else {
                format!("{date}\nEndurance\n- 45m 65%")
            }
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub(crate) fn plan_with_invalid_day(start_date: &str, invalid_date: &str) -> String {
    (0..14)
        .map(|offset| {
            let date = add_days(start_date, offset);
            if date == invalid_date {
                format!("{date}\nBroken session\n- nope")
            } else if offset % 4 == 0 {
                format!("{date}\nRest Day")
            } else {
                format!("{date}\nEndurance\n- 45m 65%")
            }
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub(crate) fn window_with_duplicate_date(start_date: &str, duplicate_date: &str) -> String {
    let mut days = (0..14)
        .map(|offset| {
            let date = add_days(start_date, offset);
            format!("{date}\nEndurance\n- 45m 65%")
        })
        .collect::<Vec<_>>();
    days[5] = format!("{duplicate_date}\nTempo\n- 30m 80%");
    days.join("\n\n")
}

pub(crate) fn window_with_gap(start_date: &str, removed_date: &str) -> String {
    (0..14)
        .filter_map(|offset| {
            let date = add_days(start_date, offset);
            (date != removed_date).then(|| format!("{date}\nEndurance\n- 45m 65%"))
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub(crate) fn single_rest_day(date: &str) -> String {
    format!("{date}\nRest Day")
}

pub(crate) fn single_invalid_day(date: &str) -> String {
    format!("{date}\nBroken session\n- nope")
}

pub(crate) fn stale_pending_operation_with_checkpoints() -> TrainingPlanGenerationOperation {
    TrainingPlanGenerationOperation {
        operation_key: format!(
            "training-plan:{USER_ID}:{WORKOUT_ID}:{}",
            date_epoch(FIRST_DAY)
        ),
        user_id: USER_ID.to_string(),
        workout_id: WORKOUT_ID.to_string(),
        saved_at_epoch_seconds: date_epoch(FIRST_DAY),
        status: WorkflowStatus::Pending,
        workout_recap_text: Some(workout_recap().text),
        workout_recap_provider: Some("openrouter".to_string()),
        workout_recap_model: Some(MODEL.to_string()),
        workout_recap_generated_at_epoch_seconds: Some(date_epoch(FIRST_DAY) - 600),
        projection_persisted_at_epoch_seconds: None,
        raw_plan_response: Some(plan_with_invalid_day(FIRST_DAY, "2026-04-10")),
        raw_correction_response: Some(single_rest_day("2026-04-10")),
        validation_issues: Vec::new(),
        attempts: Vec::new(),
        failure: None,
        started_at_epoch_seconds: date_epoch(FIRST_DAY),
        last_attempt_at_epoch_seconds: date_epoch(FIRST_DAY),
        attempt_count: 1,
        created_at_epoch_seconds: date_epoch(FIRST_DAY),
        updated_at_epoch_seconds: date_epoch(FIRST_DAY),
    }
}

pub(crate) fn stale_pending_operation_with_recap_only() -> TrainingPlanGenerationOperation {
    TrainingPlanGenerationOperation {
        operation_key: format!(
            "training-plan:{USER_ID}:{WORKOUT_ID}:{}",
            date_epoch(FIRST_DAY)
        ),
        user_id: USER_ID.to_string(),
        workout_id: WORKOUT_ID.to_string(),
        saved_at_epoch_seconds: date_epoch(FIRST_DAY),
        status: WorkflowStatus::Pending,
        workout_recap_text: Some(workout_recap().text),
        workout_recap_provider: Some("openrouter".to_string()),
        workout_recap_model: Some(MODEL.to_string()),
        workout_recap_generated_at_epoch_seconds: Some(date_epoch(FIRST_DAY) - 600),
        projection_persisted_at_epoch_seconds: None,
        raw_plan_response: None,
        raw_correction_response: None,
        validation_issues: Vec::new(),
        attempts: vec![AttemptRecord {
            phase: WorkflowPhase::WorkoutRecap,
            attempt_number: 1,
            recorded_at_epoch_seconds: date_epoch(FIRST_DAY),
        }],
        failure: None,
        started_at_epoch_seconds: date_epoch(FIRST_DAY),
        last_attempt_at_epoch_seconds: date_epoch(FIRST_DAY),
        attempt_count: 1,
        created_at_epoch_seconds: date_epoch(FIRST_DAY),
        updated_at_epoch_seconds: date_epoch(SECOND_DAY),
    }
}

pub(crate) fn stale_pending_operation_with_invalid_correction_response(
) -> TrainingPlanGenerationOperation {
    TrainingPlanGenerationOperation {
        operation_key: format!(
            "training-plan:{USER_ID}:{WORKOUT_ID}:{}",
            date_epoch(FIRST_DAY)
        ),
        user_id: USER_ID.to_string(),
        workout_id: WORKOUT_ID.to_string(),
        saved_at_epoch_seconds: date_epoch(FIRST_DAY),
        status: WorkflowStatus::Pending,
        workout_recap_text: Some(workout_recap().text),
        workout_recap_provider: Some("openrouter".to_string()),
        workout_recap_model: Some(MODEL.to_string()),
        workout_recap_generated_at_epoch_seconds: Some(date_epoch(FIRST_DAY) - 600),
        projection_persisted_at_epoch_seconds: None,
        raw_plan_response: Some(plan_with_invalid_day(FIRST_DAY, "2026-04-10")),
        raw_correction_response: Some(single_invalid_day("2026-04-10")),
        validation_issues: vec![ValidationIssue {
            scope: "2026-04-10".to_string(),
            message: "invalid planned workout step: - nope".to_string(),
        }],
        attempts: Vec::new(),
        failure: None,
        started_at_epoch_seconds: date_epoch(FIRST_DAY),
        last_attempt_at_epoch_seconds: date_epoch(FIRST_DAY),
        attempt_count: 1,
        created_at_epoch_seconds: date_epoch(FIRST_DAY),
        updated_at_epoch_seconds: date_epoch(SECOND_DAY),
    }
}

pub(crate) fn stale_pending_operation_with_snapshot_mismatch() -> TrainingPlanGenerationOperation {
    TrainingPlanGenerationOperation {
        operation_key: format!(
            "training-plan:{USER_ID}:{WORKOUT_ID}:{}",
            date_epoch(FIRST_DAY)
        ),
        user_id: USER_ID.to_string(),
        workout_id: WORKOUT_ID.to_string(),
        saved_at_epoch_seconds: date_epoch(FIRST_DAY),
        status: WorkflowStatus::Pending,
        workout_recap_text: Some(workout_recap().text),
        workout_recap_provider: Some("openrouter".to_string()),
        workout_recap_model: Some(MODEL.to_string()),
        workout_recap_generated_at_epoch_seconds: Some(date_epoch(FIRST_DAY) - 600),
        projection_persisted_at_epoch_seconds: None,
        raw_plan_response: Some(valid_plan_window(FIRST_DAY)),
        raw_correction_response: None,
        validation_issues: Vec::new(),
        attempts: Vec::new(),
        failure: None,
        started_at_epoch_seconds: date_epoch(FIRST_DAY),
        last_attempt_at_epoch_seconds: date_epoch(FIRST_DAY),
        attempt_count: 1,
        created_at_epoch_seconds: date_epoch(FIRST_DAY),
        updated_at_epoch_seconds: date_epoch(FIRST_DAY),
    }
}

pub(crate) fn snapshot_projected_days_for_first_day() -> Vec<TrainingPlanProjectedDay> {
    let snapshot = snapshot_for_first_day();

    snapshot
        .days
        .iter()
        .map(|day| TrainingPlanProjectedDay {
            user_id: snapshot.user_id.clone(),
            workout_id: snapshot.workout_id.clone(),
            operation_key: snapshot.operation_key.clone(),
            date: day.date.clone(),
            rest_day: day.rest_day,
            rest_day_reason: day.rest_day_reason.clone(),
            workout: day.workout.clone(),
            superseded_at_epoch_seconds: None,
            created_at_epoch_seconds: date_epoch(FIRST_DAY),
            updated_at_epoch_seconds: date_epoch(FIRST_DAY),
        })
        .collect()
}

pub(crate) fn snapshot_for_first_day() -> TrainingPlanSnapshot {
    TrainingPlanSnapshot {
        user_id: USER_ID.to_string(),
        workout_id: WORKOUT_ID.to_string(),
        operation_key: format!(
            "training-plan:{USER_ID}:{WORKOUT_ID}:{}",
            date_epoch(FIRST_DAY)
        ),
        saved_at_epoch_seconds: date_epoch(FIRST_DAY),
        start_date: FIRST_DAY.to_string(),
        end_date: add_days(FIRST_DAY, 13),
        days: valid_plan_window_days(FIRST_DAY),
        created_at_epoch_seconds: date_epoch(FIRST_DAY),
    }
}

pub(crate) fn valid_plan_window_days(
    start_date: &str,
) -> Vec<aiwattcoach::domain::training_plan::TrainingPlanDay> {
    let raw = valid_plan_window(start_date);
    let mut days = Vec::new();
    for block in raw.split("\n\n") {
        let parsed = aiwattcoach::domain::intervals::parse_planned_workout_days(block).unwrap();
        let day = parsed.days.into_iter().next().unwrap();
        let date = day.date.clone();
        let rest_day = day.is_rest_day();
        let rest_day_reason = day.rest_day_reason().map(ToString::to_string);
        let workout = day.into_workout();
        days.push(aiwattcoach::domain::training_plan::TrainingPlanDay {
            date,
            rest_day,
            rest_day_reason,
            workout,
        });
    }
    days
}

pub(crate) fn date_epoch(date: &str) -> i64 {
    let parsed = super::NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap();
    super::Utc
        .from_utc_datetime(&parsed.and_hms_opt(0, 0, 0).unwrap())
        .timestamp()
}

pub(crate) fn add_days(date: &str, offset: i64) -> String {
    let parsed = super::NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap();
    parsed
        .checked_add_signed(chrono::Duration::days(offset))
        .unwrap()
        .format("%Y-%m-%d")
        .to_string()
}
