use std::collections::{HashMap, HashSet};

use crate::domain::external_sync::ExternalSyncState;

use crate::domain::planned_workouts::{PlannedWorkout, PlannedWorkoutError};

use super::BoxFuture;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CalendarPlannedSyncKey {
    pub provider: String,
    pub external_id: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

    fn delete_imported_for_user_date_keeping(
        &self,
        user_id: &str,
        date: &str,
        keep_planned_workout_ids: Vec<String>,
    ) -> BoxFuture<Result<u64, PlannedWorkoutError>>;
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
    let filtered = candidates
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
        .collect::<Vec<_>>();

    keep_newest_planned_workout_per_date(filtered)
}

pub fn imported_keep_ids_by_date_for_rewrite(
    candidates: &[CalendarPlannedWorkoutCandidate],
) -> HashMap<String, Vec<String>> {
    let dates_with_imported = candidates
        .iter()
        .filter(|c| c.origin == CalendarPlannedWorkoutOrigin::Imported)
        .map(|c| c.workout.date.clone())
        .collect::<HashSet<_>>();
    if dates_with_imported.is_empty() {
        return HashMap::new();
    }

    let mut keep = dates_with_imported
        .iter()
        .map(|date| (date.clone(), Vec::new()))
        .collect::<HashMap<_, _>>();

    for winner in keep_newest_planned_workout_per_date(candidates.to_vec()) {
        if winner.origin == CalendarPlannedWorkoutOrigin::Imported {
            if let Some(ids) = keep.get_mut(&winner.workout.date) {
                ids.push(winner.workout.planned_workout_id);
            }
        }
    }
    keep
}

fn keep_newest_planned_workout_per_date(
    candidates: Vec<CalendarPlannedWorkoutCandidate>,
) -> Vec<CalendarPlannedWorkoutCandidate> {
    let mut by_date = HashMap::<String, Vec<CalendarPlannedWorkoutCandidate>>::new();
    for candidate in candidates {
        by_date
            .entry(candidate.workout.date.clone())
            .or_default()
            .push(candidate);
    }

    let mut result = Vec::new();
    for mut day_candidates in by_date.into_values() {
        let only_unstamped_projected = day_candidates.iter().all(|c| {
            c.origin == CalendarPlannedWorkoutOrigin::Projected
                && c.workout.updated_at_epoch_seconds.is_none()
        });
        if only_unstamped_projected {
            result.append(&mut day_candidates);
            continue;
        }

        if let Some(winner) = day_candidates
            .into_iter()
            .max_by(cmp_planned_candidate_recency)
        {
            result.push(winner);
        }
    }

    result.sort_by(|left, right| {
        left.workout.date.cmp(&right.workout.date).then_with(|| {
            left.workout
                .planned_workout_id
                .cmp(&right.workout.planned_workout_id)
        })
    });
    result
}

fn cmp_planned_candidate_recency(
    left: &CalendarPlannedWorkoutCandidate,
    right: &CalendarPlannedWorkoutCandidate,
) -> std::cmp::Ordering {
    match (
        left.workout.updated_at_epoch_seconds,
        right.workout.updated_at_epoch_seconds,
    ) {
        (Some(l), Some(r)) if l != r => l.cmp(&r),
        (Some(_), None) => std::cmp::Ordering::Greater,
        (None, Some(_)) => std::cmp::Ordering::Less,
        _ => match (left.origin, right.origin) {
            (CalendarPlannedWorkoutOrigin::Imported, CalendarPlannedWorkoutOrigin::Projected) => {
                std::cmp::Ordering::Greater
            }
            (CalendarPlannedWorkoutOrigin::Projected, CalendarPlannedWorkoutOrigin::Imported) => {
                std::cmp::Ordering::Less
            }
            _ => left
                .workout
                .planned_workout_id
                .cmp(&right.workout.planned_workout_id),
        },
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::planned_workouts::{PlannedWorkout, PlannedWorkoutContent};

    fn candidate(
        id: &str,
        date: &str,
        origin: CalendarPlannedWorkoutOrigin,
        updated_at: Option<i64>,
        rest_day: bool,
    ) -> CalendarPlannedWorkoutCandidate {
        let mut workout = PlannedWorkout::new(
            id.to_string(),
            "user-1".to_string(),
            date.to_string(),
            PlannedWorkoutContent { lines: Vec::new() },
        )
        .with_updated_at(updated_at);
        if rest_day {
            workout = workout.as_rest_day(Some("recover".to_string()));
        }
        CalendarPlannedWorkoutCandidate {
            workout,
            origin,
            sync_keys: Vec::new(),
        }
    }

    #[test]
    fn newer_imported_wins_over_older_imported_on_same_day() {
        let visible = select_visible_planned_workout_candidates(vec![
            candidate(
                "older",
                "2026-09-10",
                CalendarPlannedWorkoutOrigin::Imported,
                Some(100),
                false,
            ),
            candidate(
                "newer",
                "2026-09-10",
                CalendarPlannedWorkoutOrigin::Imported,
                Some(200),
                false,
            ),
        ]);

        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].workout.planned_workout_id, "newer");
    }

    #[test]
    fn newer_rest_day_replaces_older_workout_on_same_day() {
        let visible = select_visible_planned_workout_candidates(vec![
            candidate(
                "sharpening",
                "2026-09-10",
                CalendarPlannedWorkoutOrigin::Projected,
                None,
                false,
            ),
            candidate(
                "rest-override",
                "2026-09-10",
                CalendarPlannedWorkoutOrigin::Imported,
                Some(1_700_000_000),
                true,
            ),
        ]);

        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].workout.planned_workout_id, "rest-override");
        assert!(visible[0].workout.rest_day);
    }

    #[test]
    fn timestamped_import_beats_legacy_without_timestamp() {
        let visible = select_visible_planned_workout_candidates(vec![
            candidate(
                "legacy",
                "2026-09-11",
                CalendarPlannedWorkoutOrigin::Imported,
                None,
                false,
            ),
            candidate(
                "fresh",
                "2026-09-11",
                CalendarPlannedWorkoutOrigin::Imported,
                Some(50),
                true,
            ),
        ]);

        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].workout.planned_workout_id, "fresh");
    }

    #[test]
    fn different_dates_keep_independent_winners() {
        let visible = select_visible_planned_workout_candidates(vec![
            candidate(
                "day-a",
                "2026-09-10",
                CalendarPlannedWorkoutOrigin::Imported,
                Some(10),
                false,
            ),
            candidate(
                "day-b",
                "2026-09-11",
                CalendarPlannedWorkoutOrigin::Imported,
                Some(20),
                true,
            ),
        ]);

        assert_eq!(visible.len(), 2);
        assert_eq!(visible[0].workout.planned_workout_id, "day-a");
        assert_eq!(visible[1].workout.planned_workout_id, "day-b");
    }

    #[test]
    fn unstamped_projected_workouts_on_same_day_all_remain() {
        let visible = select_visible_planned_workout_candidates(vec![
            candidate(
                "am",
                "2026-09-10",
                CalendarPlannedWorkoutOrigin::Projected,
                None,
                false,
            ),
            candidate(
                "pm",
                "2026-09-10",
                CalendarPlannedWorkoutOrigin::Projected,
                None,
                false,
            ),
        ]);

        assert_eq!(visible.len(), 2);
    }

    #[test]
    fn imported_without_timestamp_loses_when_sibling_has_timestamp() {
        let visible = select_visible_planned_workout_candidates(vec![
            candidate(
                "legacy-sharpening",
                "2026-09-10",
                CalendarPlannedWorkoutOrigin::Imported,
                None,
                false,
            ),
            candidate(
                "coach-rest",
                "2026-09-10",
                CalendarPlannedWorkoutOrigin::Imported,
                Some(9_999),
                true,
            ),
        ]);
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].workout.planned_workout_id, "coach-rest");
    }

    #[test]
    fn many_timestamped_planned_keep_only_newest() {
        let visible = select_visible_planned_workout_candidates(vec![
            candidate(
                "a",
                "2026-09-10",
                CalendarPlannedWorkoutOrigin::Imported,
                Some(10),
                false,
            ),
            candidate(
                "b",
                "2026-09-10",
                CalendarPlannedWorkoutOrigin::Imported,
                Some(30),
                false,
            ),
            candidate(
                "c",
                "2026-09-10",
                CalendarPlannedWorkoutOrigin::Imported,
                Some(20),
                false,
            ),
        ]);
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].workout.planned_workout_id, "b");
    }

    #[test]
    fn rewrite_keep_ids_drop_older_imported_when_day_has_timestamp() {
        let candidates = vec![
            candidate(
                "legacy",
                "2026-09-10",
                CalendarPlannedWorkoutOrigin::Imported,
                None,
                false,
            ),
            candidate(
                "fresh",
                "2026-09-10",
                CalendarPlannedWorkoutOrigin::Imported,
                Some(50),
                false,
            ),
            candidate(
                "other-day",
                "2026-09-11",
                CalendarPlannedWorkoutOrigin::Imported,
                None,
                false,
            ),
        ];
        let keep = imported_keep_ids_by_date_for_rewrite(&candidates);
        assert_eq!(keep.get("2026-09-10"), Some(&vec!["fresh".to_string()]));
        assert_eq!(keep.get("2026-09-11"), Some(&vec!["other-day".to_string()]));
    }

    #[test]
    fn coach_rewrite_lifecycle_same_id_vs_orphan_ids() {
        const DATE: &str = "2026-09-10";
        let projected_id = "training-plan:op:2026-09-10";

        let mut imported_a = candidate(
            projected_id,
            DATE,
            CalendarPlannedWorkoutOrigin::Imported,
            Some(100),
            false,
        );
        imported_a.workout.name = Some("workout1".into());
        imported_a.workout.name = Some("workout2".into());
        imported_a.workout.updated_at_epoch_seconds = Some(200);
        imported_a.workout.name = Some("Rest Day".into());
        imported_a.workout.rest_day = false;
        imported_a.workout.updated_at_epoch_seconds = Some(300);
        imported_a.workout.name = Some("workout3".into());
        imported_a.workout.updated_at_epoch_seconds = Some(400);

        let projected = candidate(
            projected_id,
            DATE,
            CalendarPlannedWorkoutOrigin::Projected,
            None,
            false,
        );
        let visible_a =
            select_visible_planned_workout_candidates(vec![projected.clone(), imported_a]);
        assert_eq!(visible_a.len(), 1);
        assert_eq!(visible_a[0].workout.name.as_deref(), Some("workout3"));
        assert_eq!(visible_a[0].workout.updated_at_epoch_seconds, Some(400));

        let mut db_b = vec![projected];
        db_b.push({
            let mut w = candidate(
                "imported-w1",
                DATE,
                CalendarPlannedWorkoutOrigin::Imported,
                Some(100),
                false,
            );
            w.workout.name = Some("workout1".into());
            w
        });
        db_b.push({
            let mut w = candidate(
                "imported-w2",
                DATE,
                CalendarPlannedWorkoutOrigin::Imported,
                Some(200),
                false,
            );
            w.workout.name = Some("workout2".into());
            w
        });
        db_b.push({
            let mut w = candidate(
                "imported-rest",
                DATE,
                CalendarPlannedWorkoutOrigin::Imported,
                Some(300),
                true,
            );
            w.workout.name = Some("Rest Day".into());
            w
        });
        db_b.push({
            let mut w = candidate(
                "imported-w3",
                DATE,
                CalendarPlannedWorkoutOrigin::Imported,
                Some(400),
                false,
            );
            w.workout.name = Some("workout3".into());
            w
        });

        assert_eq!(db_b.len(), 5);
        let keep = imported_keep_ids_by_date_for_rewrite(&db_b);
        assert_eq!(keep.get(DATE), Some(&vec!["imported-w3".to_string()]));
        let visible_b = select_visible_planned_workout_candidates(db_b);
        assert_eq!(visible_b.len(), 1);
        assert_eq!(visible_b[0].workout.planned_workout_id, "imported-w3");
        assert_eq!(visible_b[0].workout.name.as_deref(), Some("workout3"));
        assert!(!visible_b[0].workout.rest_day);
    }
}
