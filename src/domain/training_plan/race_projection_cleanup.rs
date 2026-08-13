use std::collections::BTreeSet;

use chrono::NaiveDate;

use crate::domain::{identity::Clock, intervals::DEFAULT_PLANNED_WORKOUT_NAME};

use super::{
    BoxFuture, TrainingPlanError, TrainingPlanProjectedDay, TrainingPlanProjectionRepository,
};

#[derive(Clone)]
pub struct RaceProjectionCleanupService<Projections, Time>
where
    Projections: TrainingPlanProjectionRepository + Clone + 'static,
    Time: Clock + Clone + 'static,
{
    projections: Projections,
    clock: Time,
}

impl<Projections, Time> RaceProjectionCleanupService<Projections, Time>
where
    Projections: TrainingPlanProjectionRepository + Clone + 'static,
    Time: Clock + Clone + 'static,
{
    pub fn new(projections: Projections, clock: Time) -> Self {
        Self { projections, clock }
    }

    pub fn supersede_for_deleted_race_date(
        &self,
        user_id: &str,
        race_date: &str,
    ) -> BoxFuture<Result<Option<(String, String)>, TrainingPlanError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        let race_date = race_date.to_string();
        Box::pin(async move {
            let active = service.projections.list_active_by_user_id(&user_id).await?;
            let dates = dates_to_supersede_for_race_date(&active, &race_date);
            service
                .projections
                .supersede_active_dates(&user_id, &dates, service.clock.now_epoch_seconds())
                .await
        })
    }

    pub fn supersede_orphan_race_projections(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
        race_dates_present: &BTreeSet<String>,
    ) -> BoxFuture<Result<(), TrainingPlanError>> {
        let service = self.clone();
        let user_id = user_id.to_string();
        let oldest = oldest.to_string();
        let newest = newest.to_string();
        let race_dates_present = race_dates_present.clone();
        Box::pin(async move {
            let active = service.projections.list_active_by_user_id(&user_id).await?;
            let dates =
                orphan_race_dates_to_supersede(&active, &oldest, &newest, &race_dates_present);
            if dates.is_empty() {
                return Ok(());
            }
            service
                .projections
                .supersede_active_dates(&user_id, &dates, service.clock.now_epoch_seconds())
                .await?;
            Ok(())
        })
    }
}

pub fn projected_day_name(day: &TrainingPlanProjectedDay) -> String {
    if day.rest_day {
        return "Rest Day".to_string();
    }
    day.workout
        .as_ref()
        .and_then(|workout| {
            workout
                .lines
                .iter()
                .find_map(|line| line.text().map(ToString::to_string))
        })
        .unwrap_or_else(|| DEFAULT_PLANNED_WORKOUT_NAME.to_string())
}

pub fn is_race_prep_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.contains("opener") || lower.contains("pre-race") || lower.contains("pre race")
}

pub fn is_race_placeholder_name(name: &str) -> bool {
    if is_race_prep_name(name) {
        return false;
    }
    name.to_ascii_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .rfind(|word| !word.is_empty())
        .is_some_and(|word| word == "race")
}

pub fn previous_calendar_date(date: &str) -> Option<String> {
    NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .ok()?
        .pred_opt()
        .map(|day| day.format("%Y-%m-%d").to_string())
}

fn insert_prior_race_prep_if_present(
    dates: &mut BTreeSet<String>,
    active_days: &[TrainingPlanProjectedDay],
    race_date: &str,
) {
    let Some(prior) = previous_calendar_date(race_date) else {
        return;
    };
    if active_days.iter().any(|day| {
        day.date == prior
            && day.superseded_at_epoch_seconds.is_none()
            && is_race_prep_name(&projected_day_name(day))
    }) {
        dates.insert(prior);
    }
}

pub fn dates_to_supersede_for_race_date(
    active_days: &[TrainingPlanProjectedDay],
    race_date: &str,
) -> Vec<String> {
    let mut dates = BTreeSet::new();
    dates.insert(race_date.to_string());
    insert_prior_race_prep_if_present(&mut dates, active_days, race_date);
    dates.into_iter().collect()
}

pub fn orphan_race_dates_to_supersede(
    active_days: &[TrainingPlanProjectedDay],
    oldest: &str,
    newest: &str,
    race_dates_present: &BTreeSet<String>,
) -> Vec<String> {
    let mut dates = BTreeSet::new();
    for day in active_days {
        if day.superseded_at_epoch_seconds.is_some() {
            continue;
        }
        if day.date.as_str() < oldest || day.date.as_str() > newest {
            continue;
        }
        if !is_race_placeholder_name(&projected_day_name(day)) {
            continue;
        }
        if race_dates_present.contains(&day.date) {
            continue;
        }
        dates.insert(day.date.clone());
        insert_prior_race_prep_if_present(&mut dates, active_days, &day.date);
    }
    dates.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::intervals::{PlannedWorkout, PlannedWorkoutLine, PlannedWorkoutText};

    fn day(date: &str, name: &str) -> TrainingPlanProjectedDay {
        TrainingPlanProjectedDay {
            user_id: "user-1".to_string(),
            workout_id: "w1".to_string(),
            operation_key: "op1".to_string(),
            date: date.to_string(),
            rest_day: false,
            rest_day_reason: None,
            workout: Some(PlannedWorkout {
                lines: vec![PlannedWorkoutLine::Text(PlannedWorkoutText {
                    text: name.to_string(),
                })],
            }),
            superseded_at_epoch_seconds: None,
            created_at_epoch_seconds: 1,
            updated_at_epoch_seconds: 1,
        }
    }

    #[test]
    fn race_placeholder_excludes_openers() {
        assert!(is_race_placeholder_name("Warka B Race"));
        assert!(is_race_placeholder_name("Szosomania C Race"));
        assert!(!is_race_placeholder_name("Race Openers"));
        assert!(!is_race_placeholder_name("Pre-Race Spin"));
        assert!(!is_race_placeholder_name("Aerobic Endurance"));
        assert!(!is_race_placeholder_name("Race Pace Intervals"));
    }

    #[test]
    fn supersede_dates_include_prior_openers_only() {
        let active = vec![
            day("2026-08-21", "Active Recovery"),
            day("2026-08-22", "Race Openers"),
            day("2026-08-23", "Warka B Race"),
        ];
        assert_eq!(
            dates_to_supersede_for_race_date(&active, "2026-08-23"),
            vec!["2026-08-22".to_string(), "2026-08-23".to_string()]
        );

        let no_opener = vec![
            day("2026-08-22", "Aerobic Endurance"),
            day("2026-08-23", "Warka B Race"),
        ];
        assert_eq!(
            dates_to_supersede_for_race_date(&no_opener, "2026-08-23"),
            vec!["2026-08-23".to_string()]
        );
    }

    #[test]
    fn orphan_cleanup_skips_days_with_live_race() {
        let active = vec![
            day("2026-08-15", "Race Openers"),
            day("2026-08-16", "Szosomania C Race"),
            day("2026-08-22", "Race Openers"),
            day("2026-08-23", "Warka B Race"),
        ];
        let present = BTreeSet::from(["2026-08-16".to_string()]);
        assert_eq!(
            orphan_race_dates_to_supersede(&active, "2026-08-01", "2026-08-31", &present),
            vec!["2026-08-22".to_string(), "2026-08-23".to_string()]
        );
    }

    #[test]
    fn orphan_cleanup_skips_workouts_that_only_mention_race() {
        let active = vec![day("2026-08-10", "Race Pace Intervals")];
        assert!(orphan_race_dates_to_supersede(
            &active,
            "2026-08-01",
            "2026-08-31",
            &BTreeSet::new()
        )
        .is_empty());
    }
}
