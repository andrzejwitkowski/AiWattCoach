use crate::domain::{
    completed_workouts::{CompletedWorkout, CompletedWorkoutRepository},
    external_sync::{
        CanonicalEntityKind, CanonicalEntityRef, ExternalProvider, ExternalSyncRepositoryError,
        ExternalSyncState, ExternalSyncStateRepository,
    },
    identity::Clock,
    planned_completed_links::{PlannedCompletedWorkoutLink, PlannedCompletedWorkoutLinkRepository},
    planned_workouts::{is_imported_row_removed_for_user_date, PlannedWorkout},
    races::{Race, RaceRepository},
    special_days::{SpecialDay, SpecialDayRepository},
};
use std::sync::atomic::{AtomicUsize, Ordering};

use super::samples::sample_calendar_entry_with_date;
use crate::domain::calendar_view::{
    CalendarEntryViewRefreshPort, CalendarPlannedSyncKey, CalendarPlannedWorkoutCandidate,
    CalendarPlannedWorkoutOrigin, CalendarPlannedWorkoutSource,
};

#[derive(Clone, Copy)]
pub(crate) struct FixedClock(pub(crate) i64);

impl Clock for FixedClock {
    fn now_epoch_seconds(&self) -> i64 {
        self.0
    }
}

#[derive(Clone, Default)]
pub(crate) struct RecordingCalendarRefresh {
    calls: std::sync::Arc<std::sync::Mutex<Vec<(String, String, String)>>>,
}

impl RecordingCalendarRefresh {
    pub(crate) fn calls(&self) -> Vec<(String, String, String)> {
        self.calls.lock().unwrap().clone()
    }
}

impl CalendarEntryViewRefreshPort for RecordingCalendarRefresh {
    fn refresh_range_for_user(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> crate::domain::calendar_view::BoxFuture<
        Result<
            Vec<crate::domain::calendar_view::CalendarEntryView>,
            crate::domain::calendar_view::CalendarEntryViewError,
        >,
    > {
        let calls = self.calls.clone();
        let user_id = user_id.to_string();
        let oldest = oldest.to_string();
        let newest = newest.to_string();
        Box::pin(async move {
            calls
                .lock()
                .unwrap()
                .push((user_id, oldest, newest.clone()));
            Ok(vec![sample_calendar_entry_with_date(&newest)])
        })
    }
}

#[derive(Clone, Default)]
pub(crate) struct TestCalendarPlannedWorkoutSource {
    stored: std::sync::Arc<std::sync::Mutex<Vec<CalendarPlannedWorkoutCandidate>>>,
}

impl TestCalendarPlannedWorkoutSource {
    pub(crate) fn upsert(
        &self,
        workout: PlannedWorkout,
        origin: CalendarPlannedWorkoutOrigin,
        sync_keys: Vec<CalendarPlannedSyncKey>,
    ) {
        let mut stored = self.stored.lock().unwrap();
        stored.retain(|existing| {
            !(existing.workout.user_id == workout.user_id
                && existing.workout.planned_workout_id == workout.planned_workout_id)
        });
        stored.push(CalendarPlannedWorkoutCandidate {
            workout,
            origin,
            sync_keys,
        });
    }

    pub(crate) fn stored_imported_ids(&self) -> Vec<String> {
        let mut ids = self
            .stored
            .lock()
            .unwrap()
            .iter()
            .filter(|c| c.origin == CalendarPlannedWorkoutOrigin::Imported)
            .map(|c| c.workout.planned_workout_id.clone())
            .collect::<Vec<_>>();
        ids.sort();
        ids
    }
}

impl CalendarPlannedWorkoutSource for TestCalendarPlannedWorkoutSource {
    fn list_candidates_by_user_id_and_date_range(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> crate::domain::calendar_view::BoxFuture<
        Result<
            Vec<CalendarPlannedWorkoutCandidate>,
            crate::domain::planned_workouts::PlannedWorkoutError,
        >,
    > {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let oldest = oldest.to_string();
        let newest = newest.to_string();
        Box::pin(async move {
            Ok(stored
                .lock()
                .unwrap()
                .iter()
                .filter(|candidate| candidate.workout.user_id == user_id)
                .filter(|candidate| {
                    candidate.workout.date >= oldest && candidate.workout.date <= newest
                })
                .cloned()
                .collect())
        })
    }

    fn delete_imported_for_user_date_keeping(
        &self,
        user_id: &str,
        date: &str,
        keep_planned_workout_ids: Vec<String>,
    ) -> crate::domain::calendar_view::BoxFuture<
        Result<u64, crate::domain::planned_workouts::PlannedWorkoutError>,
    > {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let date = date.to_string();
        Box::pin(async move {
            let mut stored = stored.lock().unwrap();
            let before = stored.len();
            stored.retain(|candidate| {
                candidate.origin != CalendarPlannedWorkoutOrigin::Imported
                    || !is_imported_row_removed_for_user_date(
                        &user_id,
                        &date,
                        &keep_planned_workout_ids,
                        &candidate.workout.user_id,
                        &candidate.workout.date,
                        &candidate.workout.planned_workout_id,
                    )
            });
            Ok((before - stored.len()) as u64)
        })
    }
}

#[derive(Clone, Default)]
pub(crate) struct TestCompletedWorkoutRepository {
    stored: std::sync::Arc<std::sync::Mutex<Vec<CompletedWorkout>>>,
}

#[derive(Clone, Default)]
pub(crate) struct TestPlannedCompletedWorkoutLinkRepository {
    stored: std::sync::Arc<std::sync::Mutex<Vec<PlannedCompletedWorkoutLink>>>,
}

impl CompletedWorkoutRepository for TestCompletedWorkoutRepository {
    fn find_by_user_id_and_completed_workout_id(
        &self,
        user_id: &str,
        completed_workout_id: &str,
    ) -> crate::domain::calendar_view::BoxFuture<
        Result<Option<CompletedWorkout>, crate::domain::completed_workouts::CompletedWorkoutError>,
    > {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let completed_workout_id = completed_workout_id.to_string();
        Box::pin(async move {
            Ok(stored.lock().unwrap().iter().find_map(|workout| {
                (workout.user_id == user_id && workout.completed_workout_id == completed_workout_id)
                    .then(|| workout.clone())
            }))
        })
    }

    fn find_by_user_id_and_source_activity_id(
        &self,
        user_id: &str,
        source_activity_id: &str,
    ) -> crate::domain::calendar_view::BoxFuture<
        Result<Option<CompletedWorkout>, crate::domain::completed_workouts::CompletedWorkoutError>,
    > {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let source_activity_id = source_activity_id.to_string();
        Box::pin(async move {
            Ok(stored.lock().unwrap().iter().find_map(|workout| {
                (workout.user_id == user_id
                    && workout.source_activity_id.as_deref() == Some(source_activity_id.as_str()))
                .then(|| workout.clone())
            }))
        })
    }

    fn find_latest_by_user_id(
        &self,
        user_id: &str,
    ) -> crate::domain::calendar_view::BoxFuture<
        Result<Option<CompletedWorkout>, crate::domain::completed_workouts::CompletedWorkoutError>,
    > {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let mut workouts = stored
                .lock()
                .unwrap()
                .iter()
                .filter(|workout| workout.user_id == user_id)
                .cloned()
                .collect::<Vec<_>>();
            workouts.sort_by(|left, right| {
                right
                    .start_date_local
                    .cmp(&left.start_date_local)
                    .then_with(|| right.completed_workout_id.cmp(&left.completed_workout_id))
            });
            Ok(workouts.into_iter().next())
        })
    }

    fn list_by_user_id(
        &self,
        user_id: &str,
    ) -> crate::domain::calendar_view::BoxFuture<
        Result<Vec<CompletedWorkout>, crate::domain::completed_workouts::CompletedWorkoutError>,
    > {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            Ok(stored
                .lock()
                .unwrap()
                .iter()
                .filter(|workout| workout.user_id == user_id)
                .cloned()
                .collect())
        })
    }

    fn list_by_user_id_and_date_range(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> crate::domain::calendar_view::BoxFuture<
        Result<Vec<CompletedWorkout>, crate::domain::completed_workouts::CompletedWorkoutError>,
    > {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let oldest = oldest.to_string();
        let newest = newest.to_string();
        Box::pin(async move {
            Ok(stored
                .lock()
                .unwrap()
                .iter()
                .filter(|workout| workout.user_id == user_id)
                .filter(|workout| {
                    let date = workout
                        .start_date_local
                        .get(..10)
                        .unwrap_or(workout.start_date_local.as_str());
                    date >= oldest.as_str() && date <= newest.as_str()
                })
                .cloned()
                .collect())
        })
    }

    fn upsert(
        &self,
        workout: CompletedWorkout,
    ) -> crate::domain::calendar_view::BoxFuture<
        Result<CompletedWorkout, crate::domain::completed_workouts::CompletedWorkoutError>,
    > {
        let stored = self.stored.clone();
        Box::pin(async move {
            let mut stored = stored.lock().unwrap();
            stored.retain(|existing| {
                !(existing.user_id == workout.user_id
                    && existing.completed_workout_id == workout.completed_workout_id)
            });
            stored.push(workout.clone());
            Ok(workout)
        })
    }
}

impl PlannedCompletedWorkoutLinkRepository for TestPlannedCompletedWorkoutLinkRepository {
    fn find_by_planned_workout_id(
        &self,
        user_id: &str,
        planned_workout_id: &str,
    ) -> crate::domain::planned_completed_links::BoxFuture<
        Result<
            Option<PlannedCompletedWorkoutLink>,
            crate::domain::planned_completed_links::PlannedCompletedWorkoutLinkError,
        >,
    > {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let planned_workout_id = planned_workout_id.to_string();
        Box::pin(async move {
            Ok(stored
                .lock()
                .unwrap()
                .iter()
                .find(|link| {
                    link.user_id == user_id && link.planned_workout_id == planned_workout_id
                })
                .cloned())
        })
    }

    fn find_by_completed_workout_id(
        &self,
        user_id: &str,
        completed_workout_id: &str,
    ) -> crate::domain::planned_completed_links::BoxFuture<
        Result<
            Option<PlannedCompletedWorkoutLink>,
            crate::domain::planned_completed_links::PlannedCompletedWorkoutLinkError,
        >,
    > {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let completed_workout_id = completed_workout_id.to_string();
        Box::pin(async move {
            Ok(stored
                .lock()
                .unwrap()
                .iter()
                .find(|link| {
                    link.user_id == user_id && link.completed_workout_id == completed_workout_id
                })
                .cloned())
        })
    }

    fn find_by_planned_workout_ids(
        &self,
        user_id: &str,
        planned_workout_ids: &[String],
    ) -> crate::domain::planned_completed_links::BoxFuture<
        Result<
            Vec<PlannedCompletedWorkoutLink>,
            crate::domain::planned_completed_links::PlannedCompletedWorkoutLinkError,
        >,
    > {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let planned_workout_ids = planned_workout_ids.to_vec();
        Box::pin(async move {
            Ok(stored
                .lock()
                .unwrap()
                .iter()
                .filter(|link| {
                    link.user_id == user_id
                        && planned_workout_ids.contains(&link.planned_workout_id)
                })
                .cloned()
                .collect())
        })
    }

    fn upsert(
        &self,
        link: PlannedCompletedWorkoutLink,
    ) -> crate::domain::planned_completed_links::BoxFuture<
        Result<
            PlannedCompletedWorkoutLink,
            crate::domain::planned_completed_links::PlannedCompletedWorkoutLinkError,
        >,
    > {
        let stored = self.stored.clone();
        Box::pin(async move {
            let mut stored = stored.lock().unwrap();
            stored.retain(|existing| {
                !(existing.user_id == link.user_id
                    && (existing.planned_workout_id == link.planned_workout_id
                        || existing.completed_workout_id == link.completed_workout_id))
            });
            stored.push(link.clone());
            Ok(link)
        })
    }

    fn delete_by_completed_workout_id(
        &self,
        user_id: &str,
        completed_workout_id: &str,
    ) -> crate::domain::planned_completed_links::BoxFuture<
        Result<(), crate::domain::planned_completed_links::PlannedCompletedWorkoutLinkError>,
    > {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let completed_workout_id = completed_workout_id.to_string();
        Box::pin(async move {
            stored.lock().unwrap().retain(|existing| {
                !(existing.user_id == user_id
                    && existing.completed_workout_id == completed_workout_id)
            });
            Ok(())
        })
    }
}

#[derive(Clone, Default)]
pub(crate) struct TestRaceRepository {
    stored: std::sync::Arc<std::sync::Mutex<Vec<Race>>>,
}

impl RaceRepository for TestRaceRepository {
    fn list_by_user_id(
        &self,
        user_id: &str,
    ) -> crate::domain::races::BoxFuture<Result<Vec<Race>, crate::domain::races::RaceError>> {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            Ok(stored
                .lock()
                .unwrap()
                .iter()
                .filter(|race| race.user_id == user_id)
                .cloned()
                .collect())
        })
    }

    fn list_by_user_id_and_range(
        &self,
        user_id: &str,
        range: &crate::domain::intervals::DateRange,
    ) -> crate::domain::races::BoxFuture<Result<Vec<Race>, crate::domain::races::RaceError>> {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let oldest = range.oldest.clone();
        let newest = range.newest.clone();
        Box::pin(async move {
            Ok(stored
                .lock()
                .unwrap()
                .iter()
                .filter(|race| race.user_id == user_id)
                .filter(|race| race.date >= oldest && race.date <= newest)
                .cloned()
                .collect())
        })
    }

    fn find_by_user_id_and_race_id(
        &self,
        user_id: &str,
        race_id: &str,
    ) -> crate::domain::races::BoxFuture<Result<Option<Race>, crate::domain::races::RaceError>>
    {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let race_id = race_id.to_string();
        Box::pin(async move {
            Ok(stored
                .lock()
                .unwrap()
                .iter()
                .find(|race| race.user_id == user_id && race.race_id == race_id)
                .cloned())
        })
    }

    fn upsert(
        &self,
        race: Race,
    ) -> crate::domain::races::BoxFuture<Result<Race, crate::domain::races::RaceError>> {
        let stored = self.stored.clone();
        Box::pin(async move {
            let mut stored = stored.lock().unwrap();
            stored.retain(|existing| {
                !(existing.user_id == race.user_id && existing.race_id == race.race_id)
            });
            stored.push(race.clone());
            Ok(race)
        })
    }

    fn delete(
        &self,
        user_id: &str,
        race_id: &str,
    ) -> crate::domain::races::BoxFuture<Result<(), crate::domain::races::RaceError>> {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let race_id = race_id.to_string();
        Box::pin(async move {
            stored
                .lock()
                .unwrap()
                .retain(|race| !(race.user_id == user_id && race.race_id == race_id));
            Ok(())
        })
    }
}

#[derive(Clone, Default)]
pub(crate) struct TestSpecialDayRepository {
    stored: std::sync::Arc<std::sync::Mutex<Vec<SpecialDay>>>,
}

impl SpecialDayRepository for TestSpecialDayRepository {
    fn list_by_user_id(
        &self,
        user_id: &str,
    ) -> crate::domain::calendar_view::BoxFuture<
        Result<Vec<SpecialDay>, crate::domain::special_days::SpecialDayError>,
    > {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            Ok(stored
                .lock()
                .unwrap()
                .iter()
                .filter(|day| day.user_id == user_id)
                .cloned()
                .collect())
        })
    }

    fn list_by_user_id_and_date_range(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> crate::domain::calendar_view::BoxFuture<
        Result<Vec<SpecialDay>, crate::domain::special_days::SpecialDayError>,
    > {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let oldest = oldest.to_string();
        let newest = newest.to_string();
        Box::pin(async move {
            Ok(stored
                .lock()
                .unwrap()
                .iter()
                .filter(|day| day.user_id == user_id)
                .filter(|day| day.date >= oldest && day.date <= newest)
                .cloned()
                .collect())
        })
    }

    fn upsert(
        &self,
        special_day: SpecialDay,
    ) -> crate::domain::calendar_view::BoxFuture<
        Result<SpecialDay, crate::domain::special_days::SpecialDayError>,
    > {
        let stored = self.stored.clone();
        Box::pin(async move {
            let mut stored = stored.lock().unwrap();
            stored.retain(|existing| {
                !(existing.user_id == special_day.user_id
                    && existing.special_day_id == special_day.special_day_id)
            });
            stored.push(special_day.clone());
            Ok(special_day)
        })
    }
}

#[derive(Clone, Default)]
pub(crate) struct TestExternalSyncStateRepository {
    states: std::sync::Arc<std::sync::Mutex<Vec<ExternalSyncState>>>,
    single_lookup_count: std::sync::Arc<AtomicUsize>,
    batch_lookup_count: std::sync::Arc<AtomicUsize>,
}

impl TestExternalSyncStateRepository {
    pub(crate) fn with_states(states: Vec<ExternalSyncState>) -> Self {
        Self {
            states: std::sync::Arc::new(std::sync::Mutex::new(states)),
            single_lookup_count: std::sync::Arc::new(AtomicUsize::new(0)),
            batch_lookup_count: std::sync::Arc::new(AtomicUsize::new(0)),
        }
    }

    pub(crate) fn lookup_counts(&self) -> (usize, usize) {
        (
            self.single_lookup_count.load(Ordering::Relaxed),
            self.batch_lookup_count.load(Ordering::Relaxed),
        )
    }
}

impl ExternalSyncStateRepository for TestExternalSyncStateRepository {
    fn upsert(
        &self,
        state: ExternalSyncState,
    ) -> crate::domain::external_sync::BoxFuture<
        Result<ExternalSyncState, ExternalSyncRepositoryError>,
    > {
        Box::pin(async move { Ok(state) })
    }

    fn find_by_canonical_entities(
        &self,
        user_id: &str,
        canonical_entities: &[CanonicalEntityRef],
    ) -> crate::domain::external_sync::BoxFuture<
        Result<Vec<ExternalSyncState>, ExternalSyncRepositoryError>,
    > {
        let states = self.states.clone();
        let batch_lookup_count = self.batch_lookup_count.clone();
        let user_id = user_id.to_string();
        let canonical_entities = canonical_entities.to_vec();
        Box::pin(async move {
            batch_lookup_count.fetch_add(1, Ordering::Relaxed);
            Ok(states
                .lock()
                .unwrap()
                .iter()
                .filter(|state| state.user_id == user_id)
                .filter(|state| canonical_entities.contains(&state.canonical_entity))
                .cloned()
                .collect())
        })
    }

    fn find_by_provider_and_canonical_entity(
        &self,
        user_id: &str,
        provider: ExternalProvider,
        canonical_entity: &CanonicalEntityRef,
    ) -> crate::domain::external_sync::BoxFuture<
        Result<Option<ExternalSyncState>, ExternalSyncRepositoryError>,
    > {
        let states = self.states.clone();
        let single_lookup_count = self.single_lookup_count.clone();
        let user_id = user_id.to_string();
        let canonical_entity = canonical_entity.clone();
        Box::pin(async move {
            single_lookup_count.fetch_add(1, Ordering::Relaxed);
            Ok(states
                .lock()
                .unwrap()
                .iter()
                .find(|state| {
                    state.user_id == user_id
                        && state.provider == provider
                        && state.canonical_entity == canonical_entity
                })
                .cloned())
        })
    }

    fn find_by_provider_and_canonical_entities(
        &self,
        user_id: &str,
        provider: ExternalProvider,
        canonical_entities: &[CanonicalEntityRef],
    ) -> crate::domain::external_sync::BoxFuture<
        Result<Vec<ExternalSyncState>, ExternalSyncRepositoryError>,
    > {
        let states = self.states.clone();
        let batch_lookup_count = self.batch_lookup_count.clone();
        let user_id = user_id.to_string();
        let canonical_entities = canonical_entities.to_vec();
        Box::pin(async move {
            batch_lookup_count.fetch_add(1, Ordering::Relaxed);
            Ok(states
                .lock()
                .unwrap()
                .iter()
                .filter(|state| {
                    state.user_id == user_id
                        && state.provider == provider
                        && canonical_entities.contains(&state.canonical_entity)
                })
                .cloned()
                .collect())
        })
    }

    fn delete_by_provider_and_canonical_entity(
        &self,
        _user_id: &str,
        _provider: ExternalProvider,
        _canonical_entity: &CanonicalEntityRef,
    ) -> crate::domain::external_sync::BoxFuture<Result<(), ExternalSyncRepositoryError>> {
        Box::pin(async { Ok(()) })
    }

    fn find_by_wahoo_plan_id(
        &self,
        user_id: &str,
        wahoo_plan_id: i64,
    ) -> crate::domain::external_sync::BoxFuture<
        Result<Option<ExternalSyncState>, ExternalSyncRepositoryError>,
    > {
        let states = self.states.clone();
        let single_lookup_count = self.single_lookup_count.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            single_lookup_count.fetch_add(1, Ordering::Relaxed);
            Ok(states
                .lock()
                .unwrap()
                .iter()
                .find(|state| {
                    state.user_id == user_id
                        && state.provider == ExternalProvider::Wahoo
                        && state.wahoo_plan_id == Some(wahoo_plan_id)
                })
                .cloned())
        })
    }

    fn find_by_wahoo_workout_token(
        &self,
        user_id: &str,
        wahoo_workout_token: &str,
    ) -> crate::domain::external_sync::BoxFuture<
        Result<Option<ExternalSyncState>, ExternalSyncRepositoryError>,
    > {
        let states = self.states.clone();
        let single_lookup_count = self.single_lookup_count.clone();
        let user_id = user_id.to_string();
        let wahoo_workout_token = wahoo_workout_token.to_string();
        Box::pin(async move {
            single_lookup_count.fetch_add(1, Ordering::Relaxed);
            Ok(states
                .lock()
                .unwrap()
                .iter()
                .find(|state| {
                    state.user_id == user_id
                        && state.provider == ExternalProvider::Wahoo
                        && state.wahoo_workout_token.as_deref()
                            == Some(wahoo_workout_token.as_str())
                })
                .cloned())
        })
    }

    fn find_by_provider_and_external_id(
        &self,
        user_id: &str,
        provider: ExternalProvider,
        external_id: &str,
    ) -> crate::domain::external_sync::BoxFuture<
        Result<Option<ExternalSyncState>, ExternalSyncRepositoryError>,
    > {
        let states = self.states.clone();
        let single_lookup_count = self.single_lookup_count.clone();
        let user_id = user_id.to_string();
        let external_id = external_id.to_string();
        Box::pin(async move {
            single_lookup_count.fetch_add(1, Ordering::Relaxed);
            Ok(states
                .lock()
                .unwrap()
                .iter()
                .find(|state| {
                    state.user_id == user_id
                        && state.provider == provider
                        && state.external_id.as_deref() == Some(external_id.as_str())
                })
                .cloned())
        })
    }

    fn find_planned_workout_by_provider_and_external_id(
        &self,
        user_id: &str,
        provider: ExternalProvider,
        external_id: &str,
    ) -> crate::domain::external_sync::BoxFuture<
        Result<Option<ExternalSyncState>, ExternalSyncRepositoryError>,
    > {
        let states = self.states.clone();
        let single_lookup_count = self.single_lookup_count.clone();
        let user_id = user_id.to_string();
        let external_id = external_id.to_string();
        Box::pin(async move {
            single_lookup_count.fetch_add(1, Ordering::Relaxed);
            Ok(states
                .lock()
                .unwrap()
                .iter()
                .find(|state| {
                    state.user_id == user_id
                        && state.provider == provider
                        && state.canonical_entity.entity_kind == CanonicalEntityKind::PlannedWorkout
                        && state.external_id.as_deref() == Some(external_id.as_str())
                })
                .cloned())
        })
    }
}
