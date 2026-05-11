use std::collections::HashSet;

use crate::domain::planned_workouts::{PlannedWorkout, PlannedWorkoutError};

use super::BoxFuture;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CalendarPlannedSyncKey {
    pub provider: String,
    pub external_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CalendarPlannedWorkoutOrigin {
    Projected,
    Imported,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CalendarPlannedWorkoutCandidate {
    pub workout: PlannedWorkout,
    pub origin: CalendarPlannedWorkoutOrigin,
    pub sync_keys: Vec<CalendarPlannedSyncKey>,
}

pub trait CalendarPlannedWorkoutSource: Clone + Send + Sync + 'static {
    fn list_candidates_by_user_id_and_date_range(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> BoxFuture<Result<Vec<CalendarPlannedWorkoutCandidate>, PlannedWorkoutError>>;

    fn list_visible_planned_workout_ids_by_user_id(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<Vec<String>, PlannedWorkoutError>> {
        let source = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            Ok(select_visible_planned_workout_candidates(
                source
                    .list_candidates_by_user_id_and_date_range(&user_id, "0000-01-01", "9999-12-31")
                    .await?,
            )
            .into_iter()
            .map(|candidate| candidate.workout.planned_workout_id)
            .collect())
        })
    }
}

pub fn select_visible_planned_workout_candidates(
    candidates: Vec<CalendarPlannedWorkoutCandidate>,
) -> Vec<CalendarPlannedWorkoutCandidate> {
    let imported_ids = candidates
        .iter()
        .filter(|candidate| candidate.origin == CalendarPlannedWorkoutOrigin::Imported)
        .map(|candidate| candidate.workout.planned_workout_id.clone())
        .collect::<HashSet<_>>();
    let projected_ids = candidates
        .iter()
        .filter(|candidate| candidate.origin == CalendarPlannedWorkoutOrigin::Projected)
        .map(|candidate| candidate.workout.planned_workout_id.clone())
        .collect::<HashSet<_>>();
    let projected_sync_keys = candidates
        .iter()
        .filter(|candidate| candidate.origin == CalendarPlannedWorkoutOrigin::Projected)
        .flat_map(|candidate| candidate.sync_keys.iter().cloned())
        .collect::<HashSet<_>>();
    candidates
        .into_iter()
        .filter(|candidate| {
            if candidate.origin == CalendarPlannedWorkoutOrigin::Projected {
                return !imported_ids.contains(&candidate.workout.planned_workout_id);
            }

            projected_ids.contains(&candidate.workout.planned_workout_id)
                || !candidate
                    .sync_keys
                    .iter()
                    .any(|sync_key| projected_sync_keys.contains(sync_key))
        })
        .collect()
}
