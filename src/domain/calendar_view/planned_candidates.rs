use std::collections::{HashMap, HashSet};

use crate::domain::{
    external_sync::ExternalSyncState, training_plan_supervisor::TrainingPlanSupervisorStatus,
};

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
    pub supervisor_status: Option<TrainingPlanSupervisorStatus>,
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
    select_visible_planned_workout_candidates_with_sync_states(candidates, &HashMap::new())
}

pub fn select_visible_planned_workout_candidates_with_sync_states(
    candidates: Vec<CalendarPlannedWorkoutCandidate>,
    sync_states_by_planned_id: &HashMap<String, Vec<ExternalSyncState>>,
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
    let imported_sync_states_by_planned_id = candidates
        .iter()
        .filter(|candidate| candidate.origin == CalendarPlannedWorkoutOrigin::Imported)
        .map(|candidate| {
            (
                candidate.workout.planned_workout_id.clone(),
                sync_states_by_planned_id
                    .get(&candidate.workout.planned_workout_id)
                    .cloned()
                    .unwrap_or_default(),
            )
        })
        .collect::<HashMap<_, _>>();
    candidates
        .into_iter()
        .filter(|candidate| {
            if candidate.origin == CalendarPlannedWorkoutOrigin::Projected {
                return !imported_ids.contains(&candidate.workout.planned_workout_id)
                    && !has_visible_imported_override_for_sync_key(
                        &candidate.sync_keys,
                        &imported_sync_states_by_planned_id,
                    );
            }

            projected_ids.contains(&candidate.workout.planned_workout_id)
                || imported_candidate_owns_sync_key(candidate, &imported_sync_states_by_planned_id)
                || !candidate
                    .sync_keys
                    .iter()
                    .any(|sync_key| projected_sync_keys.contains(sync_key))
        })
        .collect()
}

fn has_visible_imported_override_for_sync_key(
    sync_keys: &[CalendarPlannedSyncKey],
    sync_states_by_planned_id: &HashMap<String, Vec<ExternalSyncState>>,
) -> bool {
    sync_states_by_planned_id
        .iter()
        .any(|(planned_workout_id, states)| {
            if states.is_empty() {
                return false;
            }

            states.iter().any(|state| {
                state.canonical_entity.entity_id == *planned_workout_id
                    && matches_external_sync_key(
                        sync_keys,
                        state.provider.as_str(),
                        state.external_id.as_deref(),
                    )
            })
        })
}

fn imported_candidate_owns_sync_key(
    candidate: &CalendarPlannedWorkoutCandidate,
    sync_states_by_planned_id: &HashMap<String, Vec<ExternalSyncState>>,
) -> bool {
    sync_states_by_planned_id
        .get(&candidate.workout.planned_workout_id)
        .into_iter()
        .flat_map(|states| states.iter())
        .any(|state| {
            matches_external_sync_key(
                &candidate.sync_keys,
                state.provider.as_str(),
                state.external_id.as_deref(),
            )
        })
}

fn matches_external_sync_key(
    sync_keys: &[CalendarPlannedSyncKey],
    provider: &str,
    external_id: Option<&str>,
) -> bool {
    let Some(external_id) = external_id else {
        return false;
    };

    sync_keys
        .iter()
        .any(|sync_key| sync_key.provider == provider && sync_key.external_id == external_id)
}
