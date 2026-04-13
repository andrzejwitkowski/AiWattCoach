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
