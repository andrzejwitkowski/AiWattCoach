use crate::domain::{
    calendar::{PlannedWorkoutSyncRecord, PlannedWorkoutSyncRepository, PlannedWorkoutSyncStatus},
    completed_workouts::CompletedWorkoutRepository,
    completed_workouts::{
        CompletedWorkout, CompletedWorkoutDetails, CompletedWorkoutMetrics, CompletedWorkoutStream,
        CompletedWorkoutZoneTime,
    },
    external_sync::{
        CanonicalEntityKind, CanonicalEntityRef, ExternalProvider, ExternalSyncRepositoryError,
        ExternalSyncState, ExternalSyncStateRepository,
    },
    planned_workouts::{
        PlannedWorkout, PlannedWorkoutContent, PlannedWorkoutLine, PlannedWorkoutRepository,
        PlannedWorkoutStep, PlannedWorkoutStepKind, PlannedWorkoutTarget, PlannedWorkoutText,
    },
    races::{Race, RaceDiscipline, RacePriority, RaceRepository},
    special_days::{SpecialDay, SpecialDayKind, SpecialDayRepository},
};

use super::ports::InMemoryCalendarEntryViewRepository;
use super::{
    project_completed_workout_entry, project_planned_workout_entry, project_race_entry,
    project_special_day_entry, verify_calendar_entry_integrity, CalendarEntryIntegrityIssue,
    CalendarEntryKind, CalendarEntryViewRefreshPort, CalendarEntryViewRefreshService,
    CalendarEntryViewRepository, CalendarEntryViewService,
};

#[tokio::test]
async fn calendar_entry_view_service_lists_mixed_entries_by_date_range() {
    let repository = InMemoryCalendarEntryViewRepository::default();
    let service = CalendarEntryViewService::new(repository.clone());

    service
        .upsert_planned_workout(&sample_planned_workout(), None)
        .await
        .unwrap();
    service
        .upsert_completed_workout(&sample_completed_workout())
        .await
        .unwrap();
    service
        .upsert_race(&sample_race(), Some(&sample_race_sync_state()))
        .await
        .unwrap();
    service
        .upsert_special_day(&sample_special_day())
        .await
        .unwrap();

    let entries = service
        .list_by_user_id_and_date_range("user-1", "2026-05-01", "2026-05-31")
        .await
        .unwrap();

    assert_eq!(entries.len(), 4);
    assert!(entries
        .iter()
        .any(|entry| entry.entry_kind == CalendarEntryKind::PlannedWorkout));
    assert!(entries
        .iter()
        .any(|entry| entry.entry_kind == CalendarEntryKind::CompletedWorkout));
    assert!(entries
        .iter()
        .any(|entry| entry.entry_kind == CalendarEntryKind::Race));
    assert!(entries
        .iter()
        .any(|entry| entry.entry_kind == CalendarEntryKind::SpecialDay));
}

#[tokio::test]
async fn rebuild_for_user_replaces_stale_entries_and_stays_idempotent() {
    let repository = InMemoryCalendarEntryViewRepository::default();
    let service = CalendarEntryViewService::new(repository.clone());

    repository
        .upsert(project_special_day_entry(&sample_other_special_day()))
        .await
        .unwrap();

    let rebuilt_once = service
        .rebuild_for_user(
            "user-1",
            &[sample_planned_workout()],
            &[sample_completed_workout()],
            &[sample_race()],
            &[sample_special_day()],
        )
        .await
        .unwrap();
    let rebuilt_twice = service
        .rebuild_for_user(
            "user-1",
            &[sample_planned_workout()],
            &[sample_completed_workout()],
            &[sample_race()],
            &[sample_special_day()],
        )
        .await
        .unwrap();

    assert_eq!(rebuilt_once, rebuilt_twice);

    let persisted = repository
        .list_by_user_id_and_date_range("user-1", "2026-05-01", "2026-05-31")
        .await
        .unwrap();
    assert_eq!(persisted.len(), 4);
    assert!(persisted
        .iter()
        .all(|entry| entry.entry_id != "special:special-stale"));
}

#[tokio::test]
async fn replace_range_for_user_replaces_only_target_range_and_handles_date_moves() {
    let repository = InMemoryCalendarEntryViewRepository::default();

    repository
        .upsert(project_planned_workout_entry(
            &sample_planned_workout(),
            None,
        ))
        .await
        .unwrap();
    repository
        .upsert(project_race_entry(&sample_race(), None))
        .await
        .unwrap();
    repository
        .upsert(project_special_day_entry(&sample_other_special_day()))
        .await
        .unwrap();

    let mut moved_planned = sample_planned_workout();
    moved_planned.date = "2026-05-15".to_string();

    repository
        .replace_range_for_user(
            "user-1",
            "2026-05-10",
            "2026-05-12",
            vec![project_planned_workout_entry(&moved_planned, None)],
        )
        .await
        .unwrap();

    let entries = repository
        .list_by_user_id_and_date_range("user-1", "2026-05-01", "2026-05-31")
        .await
        .unwrap();

    assert_eq!(entries.len(), 2);
    assert!(entries
        .iter()
        .any(|entry| entry.entry_id == "planned:planned-1" && entry.date == "2026-05-15"));
    assert!(entries
        .iter()
        .any(|entry| entry.entry_id == "special:special-stale"));
    assert!(!entries.iter().any(|entry| entry.entry_id == "race:race-1"));
}

#[tokio::test]
async fn refresh_range_for_user_rebuilds_only_requested_dates() {
    let views = InMemoryCalendarEntryViewRepository::default();
    let planned = TestPlannedWorkoutRepository::default();
    let completed = TestCompletedWorkoutRepository::default();
    let races = TestRaceRepository::default();
    let special_days = TestSpecialDayRepository::default();
    let sync_states = TestExternalSyncStateRepository;
    let planned_syncs = TestPlannedWorkoutSyncRepository::default();

    planned.upsert(sample_planned_workout()).await.unwrap();
    completed.upsert(sample_completed_workout()).await.unwrap();
    races.upsert(sample_race()).await.unwrap();
    special_days.upsert(sample_special_day()).await.unwrap();
    views
        .upsert(project_special_day_entry(&sample_other_special_day()))
        .await
        .unwrap();

    let refresher = CalendarEntryViewRefreshService::new(
        views.clone(),
        planned,
        planned_syncs,
        completed,
        races,
        special_days,
        sync_states,
    );

    let refreshed = refresher
        .refresh_range_for_user("user-1", "2026-05-10", "2026-05-13")
        .await
        .unwrap();

    assert_eq!(refreshed.len(), 4);

    let all_entries = views
        .list_by_user_id_and_date_range("user-1", "2026-05-01", "2026-05-31")
        .await
        .unwrap();
    assert_eq!(all_entries.len(), 5);
    assert!(all_entries
        .iter()
        .any(|entry| entry.entry_id == "special:special-stale"));
}

#[tokio::test]
async fn refresh_range_for_user_uses_planned_workout_sync_records_for_planned_entries() {
    let views = InMemoryCalendarEntryViewRepository::default();
    let planned = TestPlannedWorkoutRepository::default();
    let planned_syncs = TestPlannedWorkoutSyncRepository::default();
    let completed = TestCompletedWorkoutRepository::default();
    let races = TestRaceRepository::default();
    let special_days = TestSpecialDayRepository::default();
    let sync_states = TestExternalSyncStateRepository;

    planned
        .upsert(sample_bridged_planned_workout("plan-op-1", "2026-05-10"))
        .await
        .unwrap();
    planned_syncs
        .upsert(PlannedWorkoutSyncRecord {
            user_id: "user-1".to_string(),
            operation_key: "plan-op-1".to_string(),
            date: "2026-05-10".to_string(),
            source_workout_id: "source-1".to_string(),
            intervals_event_id: Some(55),
            status: PlannedWorkoutSyncStatus::Synced,
            synced_payload_hash: Some("hash-1".to_string()),
            last_error: None,
            created_at_epoch_seconds: 1,
            updated_at_epoch_seconds: 2,
            last_synced_at_epoch_seconds: Some(2),
        })
        .await
        .unwrap();

    let refresher = CalendarEntryViewRefreshService::new(
        views.clone(),
        planned,
        planned_syncs,
        completed,
        races,
        special_days,
        sync_states,
    );

    let refreshed = refresher
        .refresh_range_for_user("user-1", "2026-05-10", "2026-05-10")
        .await
        .unwrap();

    assert_eq!(refreshed.len(), 1);
    assert_eq!(
        refreshed[0]
            .sync
            .as_ref()
            .and_then(|sync| sync.linked_intervals_event_id),
        Some(55)
    );
    assert_eq!(
        refreshed[0]
            .sync
            .as_ref()
            .and_then(|sync| sync.sync_status.as_deref()),
        Some("synced")
    );
}

#[derive(Clone, Default)]
struct TestPlannedWorkoutRepository {
    stored: std::sync::Arc<std::sync::Mutex<Vec<PlannedWorkout>>>,
}

impl PlannedWorkoutRepository for TestPlannedWorkoutRepository {
    fn list_by_user_id(
        &self,
        user_id: &str,
    ) -> super::BoxFuture<
        Result<Vec<PlannedWorkout>, crate::domain::planned_workouts::PlannedWorkoutError>,
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
    ) -> super::BoxFuture<
        Result<Vec<PlannedWorkout>, crate::domain::planned_workouts::PlannedWorkoutError>,
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
                .filter(|workout| workout.date >= oldest && workout.date <= newest)
                .cloned()
                .collect())
        })
    }

    fn upsert(
        &self,
        workout: PlannedWorkout,
    ) -> super::BoxFuture<
        Result<PlannedWorkout, crate::domain::planned_workouts::PlannedWorkoutError>,
    > {
        let stored = self.stored.clone();
        Box::pin(async move {
            let mut stored = stored.lock().unwrap();
            stored.retain(|existing| {
                !(existing.user_id == workout.user_id
                    && existing.planned_workout_id == workout.planned_workout_id)
            });
            stored.push(workout.clone());
            Ok(workout)
        })
    }
}

#[derive(Clone, Default)]
struct TestCompletedWorkoutRepository {
    stored: std::sync::Arc<std::sync::Mutex<Vec<CompletedWorkout>>>,
}

#[derive(Clone, Default)]
struct TestPlannedWorkoutSyncRepository {
    stored: std::sync::Arc<std::sync::Mutex<Vec<PlannedWorkoutSyncRecord>>>,
}

impl PlannedWorkoutSyncRepository for TestPlannedWorkoutSyncRepository {
    fn find_by_user_id_and_projection(
        &self,
        user_id: &str,
        operation_key: &str,
        date: &str,
    ) -> crate::domain::calendar::BoxFuture<
        Result<Option<PlannedWorkoutSyncRecord>, crate::domain::calendar::CalendarError>,
    > {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let operation_key = operation_key.to_string();
        let date = date.to_string();
        Box::pin(async move {
            Ok(stored
                .lock()
                .unwrap()
                .iter()
                .find(|record| {
                    record.user_id == user_id
                        && record.operation_key == operation_key
                        && record.date == date
                })
                .cloned())
        })
    }

    fn list_by_user_id_and_range(
        &self,
        user_id: &str,
        range: &crate::domain::intervals::DateRange,
    ) -> crate::domain::calendar::BoxFuture<
        Result<Vec<PlannedWorkoutSyncRecord>, crate::domain::calendar::CalendarError>,
    > {
        let stored = self.stored.clone();
        let user_id = user_id.to_string();
        let oldest = range.oldest.clone();
        let newest = range.newest.clone();
        Box::pin(async move {
            Ok(stored
                .lock()
                .unwrap()
                .iter()
                .filter(|record| record.user_id == user_id)
                .filter(|record| record.date >= oldest && record.date <= newest)
                .cloned()
                .collect())
        })
    }

    fn upsert(
        &self,
        record: PlannedWorkoutSyncRecord,
    ) -> crate::domain::calendar::BoxFuture<
        Result<PlannedWorkoutSyncRecord, crate::domain::calendar::CalendarError>,
    > {
        let stored = self.stored.clone();
        Box::pin(async move {
            let mut stored = stored.lock().unwrap();
            stored.retain(|existing| {
                !(existing.user_id == record.user_id
                    && existing.operation_key == record.operation_key
                    && existing.date == record.date)
            });
            stored.push(record.clone());
            Ok(record)
        })
    }
}

impl CompletedWorkoutRepository for TestCompletedWorkoutRepository {
    fn list_by_user_id(
        &self,
        user_id: &str,
    ) -> super::BoxFuture<
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
    ) -> super::BoxFuture<
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
    ) -> super::BoxFuture<
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

#[derive(Clone, Default)]
struct TestRaceRepository {
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
struct TestSpecialDayRepository {
    stored: std::sync::Arc<std::sync::Mutex<Vec<SpecialDay>>>,
}

impl SpecialDayRepository for TestSpecialDayRepository {
    fn list_by_user_id(
        &self,
        user_id: &str,
    ) -> super::BoxFuture<Result<Vec<SpecialDay>, crate::domain::special_days::SpecialDayError>>
    {
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
    ) -> super::BoxFuture<Result<Vec<SpecialDay>, crate::domain::special_days::SpecialDayError>>
    {
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
    ) -> super::BoxFuture<Result<SpecialDay, crate::domain::special_days::SpecialDayError>> {
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
struct TestExternalSyncStateRepository;

impl ExternalSyncStateRepository for TestExternalSyncStateRepository {
    fn upsert(
        &self,
        state: ExternalSyncState,
    ) -> crate::domain::external_sync::BoxFuture<
        Result<ExternalSyncState, ExternalSyncRepositoryError>,
    > {
        Box::pin(async move { Ok(state) })
    }

    fn find_by_provider_and_canonical_entity(
        &self,
        _user_id: &str,
        _provider: ExternalProvider,
        _canonical_entity: &CanonicalEntityRef,
    ) -> crate::domain::external_sync::BoxFuture<
        Result<Option<ExternalSyncState>, ExternalSyncRepositoryError>,
    > {
        Box::pin(async { Ok(None) })
    }

    fn delete_by_provider_and_canonical_entity(
        &self,
        _user_id: &str,
        _provider: ExternalProvider,
        _canonical_entity: &CanonicalEntityRef,
    ) -> crate::domain::external_sync::BoxFuture<Result<(), ExternalSyncRepositoryError>> {
        Box::pin(async { Ok(()) })
    }
}

#[test]
fn integrity_report_flags_missing_duplicate_type_mismatch_and_orphan_rows() {
    let expected = vec![project_planned_workout_entry(
        &sample_planned_workout(),
        None,
    )];
    let actual = vec![
        project_completed_workout_entry(&sample_completed_workout()),
        project_completed_workout_entry(&sample_completed_workout()),
        sample_orphan_entry(),
        sample_type_mismatch_entry(),
    ];

    let report = verify_calendar_entry_integrity(&expected, &actual);

    assert!(report
        .issues
        .contains(&CalendarEntryIntegrityIssue::MissingEntry {
            entry_id: "planned:planned-1".to_string(),
        }));
    assert!(report
        .issues
        .contains(&CalendarEntryIntegrityIssue::DuplicateEntry {
            entry_id: "completed:completed-1".to_string(),
            count: 2,
        }));
    assert!(report
        .issues
        .contains(&CalendarEntryIntegrityIssue::OrphanEntry {
            entry_id: "special:orphan-1".to_string(),
        }));
    assert!(report
        .issues
        .contains(&CalendarEntryIntegrityIssue::TypeMismatch {
            entry_id: "planned:planned-1".to_string(),
            expected_kind: CalendarEntryKind::PlannedWorkout,
            actual_kind: CalendarEntryKind::Race,
        }));
}

#[test]
fn planned_workout_projection_builds_local_entry() {
    let entry = project_planned_workout_entry(&sample_planned_workout(), None);

    assert_eq!(entry.entry_id, "planned:planned-1");
    assert_eq!(entry.title, "Threshold builder");
    assert_eq!(entry.planned_workout_id.as_deref(), Some("planned-1"));
    assert_eq!(entry.sync, None);
}

#[test]
fn completed_workout_projection_carries_local_summary() {
    let entry = project_completed_workout_entry(&sample_completed_workout());

    assert_eq!(entry.entry_id, "completed:completed-1");
    assert_eq!(
        entry
            .summary
            .as_ref()
            .and_then(|summary| summary.training_stress_score),
        Some(82)
    );
    assert_eq!(entry.completed_workout_id.as_deref(), Some("completed-1"));
}

#[test]
fn completed_workout_projection_handles_short_start_date_local_without_panicking() {
    let mut workout = sample_completed_workout();
    workout.start_date_local = "2026-05".to_string();

    let entry = project_completed_workout_entry(&workout);

    assert_eq!(entry.date, "2026-05");
}

#[test]
fn race_projection_keeps_label_shape_and_sync_metadata() {
    let entry = project_race_entry(&sample_race(), Some(&sample_race_sync_state()));

    assert_eq!(entry.entry_id, "race:race-1");
    assert_eq!(entry.title, "Race Gravel Attack");
    assert_eq!(entry.subtitle.as_deref(), Some("120 km • Kat. B"));
    assert_eq!(
        entry.description.as_deref(),
        Some("distance_meters=120000\ndiscipline=gravel\npriority=B")
    );
    assert_eq!(
        entry
            .sync
            .as_ref()
            .and_then(|sync| sync.linked_intervals_event_id),
        Some(41)
    );
}

#[test]
fn special_day_projection_keeps_meaningful_title() {
    let entry = project_special_day_entry(&sample_special_day());

    assert_eq!(entry.entry_id, "special:special-1");
    assert_eq!(entry.title, "Illness");
    assert_eq!(entry.special_day_id.as_deref(), Some("special-1"));
}

fn sample_planned_workout() -> PlannedWorkout {
    PlannedWorkout::new(
        "planned-1".to_string(),
        "user-1".to_string(),
        "2026-05-10".to_string(),
        PlannedWorkoutContent {
            lines: vec![
                PlannedWorkoutLine::Text(PlannedWorkoutText {
                    text: "Threshold builder".to_string(),
                }),
                PlannedWorkoutLine::Step(PlannedWorkoutStep {
                    duration_seconds: 600,
                    kind: PlannedWorkoutStepKind::Steady,
                    target: PlannedWorkoutTarget::PercentFtp {
                        min: 90.0,
                        max: 95.0,
                    },
                }),
            ],
        },
    )
}

fn sample_bridged_planned_workout(operation_key: &str, date: &str) -> PlannedWorkout {
    PlannedWorkout::new(
        format!("{operation_key}:{date}"),
        "user-1".to_string(),
        date.to_string(),
        sample_planned_workout().workout,
    )
}

fn sample_completed_workout() -> CompletedWorkout {
    CompletedWorkout::new(
        "completed-1".to_string(),
        "user-1".to_string(),
        "2026-05-11T08:00:00".to_string(),
        CompletedWorkoutMetrics {
            training_stress_score: Some(82),
            normalized_power_watts: Some(252),
            intensity_factor: Some(0.86),
            efficiency_factor: None,
            variability_index: Some(1.05),
            average_power_watts: Some(228),
            ftp_watts: Some(295),
            total_work_joules: None,
            calories: None,
            trimp: None,
            power_load: None,
            heart_rate_load: None,
            pace_load: None,
            strain_score: None,
        },
        CompletedWorkoutDetails {
            intervals: Vec::new(),
            interval_groups: Vec::new(),
            streams: vec![CompletedWorkoutStream {
                stream_type: "watts".to_string(),
                name: Some("Power".to_string()),
                primary_series: Some(serde_json::json!([180, 240, 310])),
                secondary_series: None,
                value_type_is_array: false,
                custom: false,
                all_null: false,
            }],
            interval_summary: vec!["steady threshold".to_string()],
            skyline_chart: Vec::new(),
            power_zone_times: vec![CompletedWorkoutZoneTime {
                zone_id: "z4".to_string(),
                seconds: 1400,
            }],
            heart_rate_zone_times: vec![700],
            pace_zone_times: Vec::new(),
            gap_zone_times: Vec::new(),
        },
    )
}

fn sample_race() -> Race {
    Race {
        race_id: "race-1".to_string(),
        user_id: "user-1".to_string(),
        date: "2026-05-12".to_string(),
        name: "Gravel Attack".to_string(),
        distance_meters: 120_000,
        discipline: RaceDiscipline::Gravel,
        priority: RacePriority::B,
        result: None,
        created_at_epoch_seconds: 1,
        updated_at_epoch_seconds: 2,
    }
}

fn sample_race_sync_state() -> ExternalSyncState {
    ExternalSyncState::new(
        "user-1".to_string(),
        ExternalProvider::Intervals,
        CanonicalEntityRef::new(CanonicalEntityKind::Race, "race-1".to_string()),
    )
    .mark_synced("41".to_string(), "hash-1".to_string(), 1_700_000_000)
}

fn sample_special_day() -> SpecialDay {
    SpecialDay::new(
        "special-1".to_string(),
        "user-1".to_string(),
        "2026-05-13".to_string(),
        SpecialDayKind::Illness,
    )
}

fn sample_other_special_day() -> SpecialDay {
    SpecialDay::new(
        "special-stale".to_string(),
        "user-1".to_string(),
        "2026-05-09".to_string(),
        SpecialDayKind::Other,
    )
}

fn sample_orphan_entry() -> super::CalendarEntryView {
    project_special_day_entry(&SpecialDay::new(
        "orphan-1".to_string(),
        "user-1".to_string(),
        "2026-05-14".to_string(),
        SpecialDayKind::Other,
    ))
}

fn sample_type_mismatch_entry() -> super::CalendarEntryView {
    let mut entry = project_race_entry(&sample_race(), None);
    entry.entry_id = "planned:planned-1".to_string();
    entry.entry_kind = CalendarEntryKind::Race;
    entry.planned_workout_id = Some("planned-1".to_string());
    entry.race_id = Some("race-1".to_string());
    entry
}
