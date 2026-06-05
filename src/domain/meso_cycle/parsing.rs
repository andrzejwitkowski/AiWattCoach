use std::collections::BTreeMap;

use chrono::NaiveDate;

use crate::domain::{
    intervals::{parse_planned_workout_days, PlannedWorkoutDay},
    training_plan::split_into_day_blocks,
};

use super::{MesoCycleDay, MesoCycleError, MESO_CYCLE_WINDOW_DAY_COUNT};

fn map_parsed_day(day: PlannedWorkoutDay) -> MesoCycleDay {
    let date = day.date.clone();
    let rest_day = day.is_rest_day();
    let rest_day_reason = day.rest_day_reason().map(ToString::to_string);
    let workout = day.into_workout();
    MesoCycleDay {
        date,
        rest_day,
        rest_day_reason,
        workout,
    }
}

pub fn parse_meso_plan_window(
    raw_window: &str,
    meso_start: &str,
    meso_end: &str,
) -> Result<Vec<MesoCycleDay>, MesoCycleError> {
    let blocks = split_into_day_blocks(raw_window).map_err(|error| match error {
        crate::domain::training_plan::TrainingPlanError::Validation(message) => {
            MesoCycleError::Validation(message)
        }
        crate::domain::training_plan::TrainingPlanError::Unavailable(message) => {
            MesoCycleError::Unavailable(message)
        }
        crate::domain::training_plan::TrainingPlanError::Repository(message) => {
            MesoCycleError::Repository(message)
        }
    })?;

    let mut days_by_date = BTreeMap::new();
    for (date, block) in blocks {
        if days_by_date.contains_key(&date) {
            return Err(MesoCycleError::Validation(format!(
                "duplicate planned workout day: {date}"
            )));
        }

        let parsed = parse_planned_workout_days(&block).map_err(|error| {
            MesoCycleError::Validation(format!("invalid meso day {date}: {error}"))
        })?;
        let Some(day) = parsed.days.into_iter().next() else {
            return Err(MesoCycleError::Validation(format!(
                "meso day {date} did not parse"
            )));
        };
        days_by_date.insert(date, map_parsed_day(day));
    }

    let days = days_by_date.values().cloned().collect::<Vec<_>>();
    validate_contiguous_window(&days, meso_start, meso_end)?;
    Ok(days)
}

fn validate_contiguous_window(
    days: &[MesoCycleDay],
    meso_start: &str,
    meso_end: &str,
) -> Result<(), MesoCycleError> {
    if days.len() != MESO_CYCLE_WINDOW_DAY_COUNT {
        return Err(MesoCycleError::Validation(format!(
            "meso cycle window must contain exactly {MESO_CYCLE_WINDOW_DAY_COUNT} contiguous dated days"
        )));
    }

    let contiguous = days.windows(2).all(|window| {
        let left = NaiveDate::parse_from_str(&window[0].date, "%Y-%m-%d").ok();
        let right = NaiveDate::parse_from_str(&window[1].date, "%Y-%m-%d").ok();
        match (left, right) {
            (Some(left), Some(right)) => right == left + chrono::Duration::days(1),
            _ => false,
        }
    });

    if !contiguous {
        return Err(MesoCycleError::Validation(
            "meso cycle window days must be contiguous".to_string(),
        ));
    }

    if days.first().is_some_and(|day| day.date != meso_start) {
        return Err(MesoCycleError::Validation(format!(
            "meso cycle window must start on {meso_start}"
        )));
    }

    if days.last().is_some_and(|day| day.date != meso_end) {
        return Err(MesoCycleError::Validation(format!(
            "meso cycle window must end on {meso_end}"
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_window_that_does_not_match_requested_bounds() {
        let mut days = Vec::new();
        let start = NaiveDate::from_ymd_opt(2026, 6, 10).expect("valid date");
        for offset in 0..MESO_CYCLE_WINDOW_DAY_COUNT {
            let date = start + chrono::Duration::days(offset as i64);
            days.push(MesoCycleDay {
                date: date.format("%Y-%m-%d").to_string(),
                rest_day: true,
                rest_day_reason: Some("rest".to_string()),
                workout: None,
            });
        }

        let error = validate_contiguous_window(&days, "2026-06-01", "2026-06-30")
            .expect_err("misaligned window should fail");

        assert!(matches!(error, MesoCycleError::Validation(_)));
    }
}
