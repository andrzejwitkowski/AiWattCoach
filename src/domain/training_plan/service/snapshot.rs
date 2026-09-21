use chrono::NaiveDate;

use std::collections::{BTreeMap, BTreeSet};

use crate::domain::{calendar_view::CalendarEntryViewRefreshPort, identity::Clock};

use super::TrainingPlanGenerationService;
use crate::domain::training_plan::{
    TrainingPlanDay, TrainingPlanError, TrainingPlanGenerationOperationRepository,
    TrainingPlanGenerator, TrainingPlanProjectedDay, TrainingPlanProjectionRepository,
    TrainingPlanSnapshot, TrainingPlanSnapshotRepository, TrainingPlanWorkoutSummaryPort,
};

impl<Snapshots, Projections, Operations, Generator, WorkoutSummary, Time, Refresh>
    TrainingPlanGenerationService<
        Snapshots,
        Projections,
        Operations,
        Generator,
        WorkoutSummary,
        Time,
        Refresh,
    >
where
    Snapshots: TrainingPlanSnapshotRepository + Clone,
    Projections: TrainingPlanProjectionRepository + Clone,
    Operations: TrainingPlanGenerationOperationRepository + Clone,
    Generator: TrainingPlanGenerator + Clone,
    WorkoutSummary: TrainingPlanWorkoutSummaryPort + Clone,
    Time: Clock + Clone,
    Refresh: CalendarEntryViewRefreshPort + Clone,
{
    fn days_are_contiguous(days: &[TrainingPlanDay]) -> bool {
        days.windows(2).all(|window| {
            let left = NaiveDate::parse_from_str(&window[0].date, "%Y-%m-%d").ok();
            let right = NaiveDate::parse_from_str(&window[1].date, "%Y-%m-%d").ok();
            match (left, right) {
                (Some(left), Some(right)) => right == left + chrono::Duration::days(1),
                _ => false,
            }
        })
    }

    /// When the model returns more than the snapshot window, keep the earliest
    /// `SNAPSHOT_DAY_COUNT` days and drop the rest. Shorter windows are unchanged
    /// (still a validation error later).
    pub(super) fn clip_days_to_window(
        &self,
        days_by_date: &mut BTreeMap<String, TrainingPlanDay>,
    ) -> Option<(String, String, usize)> {
        clip_plan_days_to_window(days_by_date, Self::SNAPSHOT_DAY_COUNT)
    }

    pub(super) fn clip_and_warn_overlong_window(
        &self,
        operation_key: &str,
        days_by_date: &mut BTreeMap<String, TrainingPlanDay>,
    ) {
        if let Some((first, last, dropped)) = self.clip_days_to_window(days_by_date) {
            tracing::warn!(
                operation_key = %operation_key,
                "training plan window longer than 14 days; keeping {first}..{last} (dropped {dropped} days)"
            );
        }
    }

    pub(super) fn validate_snapshot_days(
        &self,
        days_by_date: &BTreeMap<String, TrainingPlanDay>,
    ) -> Result<Vec<TrainingPlanDay>, TrainingPlanError> {
        let days = days_by_date.values().cloned().collect::<Vec<_>>();
        if days.len() != Self::SNAPSHOT_DAY_COUNT || !Self::days_are_contiguous(&days) {
            return Err(TrainingPlanError::Validation(format!(
                "training plan window must contain exactly {} contiguous dated days",
                Self::SNAPSHOT_DAY_COUNT
            )));
        }
        Ok(days)
    }

    pub(super) fn build_snapshot(
        &self,
        user_id: &str,
        workout_id: &str,
        operation_key: &str,
        saved_at_epoch_seconds: i64,
        days: Vec<TrainingPlanDay>,
    ) -> Result<TrainingPlanSnapshot, TrainingPlanError> {
        let start_date = days.first().map(|day| day.date.clone()).ok_or_else(|| {
            TrainingPlanError::Validation("training plan window is empty".to_string())
        })?;
        let end_date = days.last().map(|day| day.date.clone()).ok_or_else(|| {
            TrainingPlanError::Validation("training plan window is empty".to_string())
        })?;

        Ok(TrainingPlanSnapshot {
            user_id: user_id.to_string(),
            workout_id: workout_id.to_string(),
            operation_key: operation_key.to_string(),
            saved_at_epoch_seconds,
            start_date,
            end_date,
            days,
            created_at_epoch_seconds: self.clock.now_epoch_seconds(),
        })
    }

    pub(super) fn build_projected_days(
        &self,
        snapshot: &TrainingPlanSnapshot,
    ) -> Vec<TrainingPlanProjectedDay> {
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
                created_at_epoch_seconds: self.clock.now_epoch_seconds(),
                updated_at_epoch_seconds: self.clock.now_epoch_seconds(),
            })
            .collect()
    }

    pub(super) fn expected_active_projected_dates(
        &self,
        snapshot: &TrainingPlanSnapshot,
    ) -> BTreeSet<String> {
        let today = self.today_string();
        snapshot
            .days
            .iter()
            .filter(|day| day.date > today)
            .map(|day| day.date.clone())
            .collect()
    }

    pub(super) fn is_projection_persisted(
        &self,
        snapshot: &TrainingPlanSnapshot,
        active_projected_days: &[TrainingPlanProjectedDay],
    ) -> bool {
        let today = self.today_string();
        let expected_dates = self.expected_active_projected_dates(snapshot);
        let actual_dates = active_projected_days
            .iter()
            .filter(|day| day.operation_key == snapshot.operation_key && day.is_active_on(&today))
            .map(|day| day.date.clone())
            .collect::<BTreeSet<_>>();
        actual_dates == expected_dates
    }
}

/// Keep the earliest `window_len` dated days (BTreeMap order). Returns
/// `(first_kept, last_kept, dropped_count)` when clipping occurred.
pub(super) fn clip_plan_days_to_window(
    days_by_date: &mut BTreeMap<String, TrainingPlanDay>,
    window_len: usize,
) -> Option<(String, String, usize)> {
    let original_len = days_by_date.len();
    if original_len <= window_len {
        return None;
    }
    let keep: Vec<String> = days_by_date.keys().take(window_len).cloned().collect();
    let first = keep.first()?.clone();
    let last = keep.last()?.clone();
    let dropped = original_len - window_len;
    days_by_date.retain(|date, _| keep.binary_search(date).is_ok());
    Some((first, last, dropped))
}

#[cfg(test)]
mod tests {
    use super::clip_plan_days_to_window;
    use crate::domain::training_plan::TrainingPlanDay;
    use std::collections::BTreeMap;

    fn rest_day(date: &str) -> TrainingPlanDay {
        TrainingPlanDay {
            date: date.to_string(),
            rest_day: true,
            rest_day_reason: None,
            workout: None,
        }
    }

    fn contiguous_days(start: &str, count: usize) -> BTreeMap<String, TrainingPlanDay> {
        let start = chrono::NaiveDate::parse_from_str(start, "%Y-%m-%d").unwrap();
        (0..count)
            .map(|offset| {
                let date = (start + chrono::Duration::days(offset as i64))
                    .format("%Y-%m-%d")
                    .to_string();
                (date.clone(), rest_day(&date))
            })
            .collect()
    }

    #[test]
    fn clip_keeps_first_fourteen_of_twenty_one_contiguous_days() {
        let mut days = contiguous_days("2026-09-21", 21);
        let clipped = clip_plan_days_to_window(&mut days, 14).expect("should clip");
        assert_eq!(clipped, ("2026-09-21".into(), "2026-10-04".into(), 7));
        assert_eq!(days.len(), 14);
        assert!(days.contains_key("2026-09-21"));
        assert!(days.contains_key("2026-10-04"));
        assert!(!days.contains_key("2026-10-05"));
    }

    #[test]
    fn clip_is_noop_for_exact_window() {
        let mut days = contiguous_days("2026-04-06", 14);
        assert!(clip_plan_days_to_window(&mut days, 14).is_none());
        assert_eq!(days.len(), 14);
    }

    #[test]
    fn clip_is_noop_for_shorter_than_window() {
        let mut days = contiguous_days("2026-04-06", 13);
        assert!(clip_plan_days_to_window(&mut days, 14).is_none());
        assert_eq!(days.len(), 13);
    }
}
