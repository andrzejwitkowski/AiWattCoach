use std::sync::{Arc, Mutex};

use crate::domain::{
    external_sync::{
        CanonicalEntityKind, CanonicalEntityRef, ExternalProvider, ExternalSyncState,
        ExternalSyncStateRepository,
    },
    intervals::{PlannedWorkout, PlannedWorkoutLine, PlannedWorkoutText},
    races::{
        imported_intervals_race_id, Race, RaceDiscipline, RacePriority, RaceService, RaceUseCases,
    },
    training_plan::{
        BoxFuture as TrainingPlanBoxFuture, RaceProjectionCleanupService, TrainingPlanError,
        TrainingPlanProjectedDay, TrainingPlanProjectionRepository, TrainingPlanReplacementResult,
        TrainingPlanSnapshot,
    },
};

use super::support::*;

#[derive(Clone, Default)]
struct InMemoryProjectionRepository {
    days: Arc<Mutex<Vec<TrainingPlanProjectedDay>>>,
}

impl InMemoryProjectionRepository {
    fn with_days(days: Vec<TrainingPlanProjectedDay>) -> Self {
        Self {
            days: Arc::new(Mutex::new(days)),
        }
    }

    fn active_dates(&self) -> Vec<String> {
        self.days
            .lock()
            .unwrap()
            .iter()
            .filter(|day| day.superseded_at_epoch_seconds.is_none())
            .map(|day| day.date.clone())
            .collect()
    }
}

impl TrainingPlanProjectionRepository for InMemoryProjectionRepository {
    fn list_active_by_user_id(
        &self,
        user_id: &str,
    ) -> TrainingPlanBoxFuture<Result<Vec<TrainingPlanProjectedDay>, TrainingPlanError>> {
        let user_id = user_id.to_string();
        let days = self.days.lock().unwrap().clone();
        Box::pin(async move {
            Ok(days
                .into_iter()
                .filter(|day| day.user_id == user_id && day.superseded_at_epoch_seconds.is_none())
                .collect())
        })
    }

    fn find_active_by_operation_key(
        &self,
        operation_key: &str,
    ) -> TrainingPlanBoxFuture<Result<Vec<TrainingPlanProjectedDay>, TrainingPlanError>> {
        let operation_key = operation_key.to_string();
        let days = self.days.lock().unwrap().clone();
        Box::pin(async move {
            Ok(days
                .into_iter()
                .filter(|day| {
                    day.operation_key == operation_key && day.superseded_at_epoch_seconds.is_none()
                })
                .collect())
        })
    }

    fn find_active_by_user_id_and_operation_key(
        &self,
        user_id: &str,
        operation_key: &str,
    ) -> TrainingPlanBoxFuture<Result<Vec<TrainingPlanProjectedDay>, TrainingPlanError>> {
        let user_id = user_id.to_string();
        let operation_key = operation_key.to_string();
        let days = self.days.lock().unwrap().clone();
        Box::pin(async move {
            Ok(days
                .into_iter()
                .filter(|day| {
                    day.user_id == user_id
                        && day.operation_key == operation_key
                        && day.superseded_at_epoch_seconds.is_none()
                })
                .collect())
        })
    }

    fn replace_window(
        &self,
        snapshot: TrainingPlanSnapshot,
        projected_days: Vec<TrainingPlanProjectedDay>,
        _today: &str,
        _replaced_at_epoch_seconds: i64,
    ) -> TrainingPlanBoxFuture<Result<TrainingPlanReplacementResult, TrainingPlanError>> {
        Box::pin(async move {
            Ok(TrainingPlanReplacementResult {
                snapshot,
                projected_days,
                superseded_date_range: None,
            })
        })
    }

    fn supersede_active_dates(
        &self,
        user_id: &str,
        dates: &[String],
        superseded_at_epoch_seconds: i64,
    ) -> TrainingPlanBoxFuture<Result<Option<(String, String)>, TrainingPlanError>> {
        let user_id = user_id.to_string();
        let dates = dates.to_vec();
        let days = self.days.clone();
        Box::pin(async move {
            if dates.is_empty() {
                return Ok(None);
            }
            let mut matched = Vec::new();
            let mut stored = days.lock().unwrap();
            for day in stored.iter_mut() {
                if day.user_id == user_id
                    && day.superseded_at_epoch_seconds.is_none()
                    && dates.contains(&day.date)
                {
                    day.superseded_at_epoch_seconds = Some(superseded_at_epoch_seconds);
                    day.updated_at_epoch_seconds = superseded_at_epoch_seconds;
                    matched.push(day.date.clone());
                }
            }
            if matched.is_empty() {
                return Ok(None);
            }
            matched.sort();
            Ok(Some((
                matched.first().cloned().unwrap_or_default(),
                matched.last().cloned().unwrap_or_default(),
            )))
        })
    }
}

fn named_day(date: &str, name: &str) -> TrainingPlanProjectedDay {
    TrainingPlanProjectedDay {
        user_id: "user-1".to_string(),
        workout_id: "i1".to_string(),
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

fn sample_race() -> crate::domain::races::Race {
    crate::domain::races::Race {
        race_id: "race-1".to_string(),
        user_id: "user-1".to_string(),
        date: "2026-08-23".to_string(),
        name: "Warka".to_string(),
        distance_meters: 90_000,
        discipline: crate::domain::races::RaceDiscipline::Road,
        priority: crate::domain::races::RacePriority::B,
        result: None,
        created_at_epoch_seconds: 1,
        updated_at_epoch_seconds: 2,
    }
}

#[tokio::test]
async fn delete_race_supersedes_race_day_and_prior_openers_via_cleanup_service() {
    let projections = InMemoryProjectionRepository::with_days(vec![
        named_day("2026-08-21", "Active Recovery"),
        named_day("2026-08-22", "Race Openers"),
        named_day("2026-08-23", "Warka B Race"),
    ]);
    let cleanup = RaceProjectionCleanupService::new(projections.clone(), TestClock);
    let calendar_refresh = RecordingCalendarRefresh::default();
    let service = RaceService::new(
        InMemoryRaceRepository::with_races(vec![sample_race()]),
        RecordingIntervalsService::default(),
        InMemoryExternalSyncStateRepository::default(),
        TestClock,
        TestIdGenerator::default(),
    )
    .with_calendar_view_refresh(calendar_refresh.clone())
    .with_projection_cleanup(cleanup);

    service.delete_race("user-1", "race-1").await.unwrap();

    assert_eq!(projections.active_dates(), vec!["2026-08-21".to_string()]);
    assert_eq!(
        calendar_refresh.stored(),
        vec![(
            "user-1".to_string(),
            "2026-08-22".to_string(),
            "2026-08-23".to_string()
        )]
    );
}

#[tokio::test]
async fn delete_race_keeps_prior_endurance_day_when_not_race_prep() {
    let projections = InMemoryProjectionRepository::with_days(vec![
        named_day("2026-08-22", "Aerobic Endurance"),
        named_day("2026-08-23", "Warka B Race"),
    ]);
    let cleanup = RaceProjectionCleanupService::new(projections.clone(), TestClock);
    let service = RaceService::new(
        InMemoryRaceRepository::with_races(vec![sample_race()]),
        RecordingIntervalsService::default(),
        InMemoryExternalSyncStateRepository::default(),
        TestClock,
        TestIdGenerator::default(),
    )
    .with_calendar_view_refresh(RecordingCalendarRefresh::default())
    .with_projection_cleanup(cleanup);

    service.delete_race("user-1", "race-1").await.unwrap();

    assert_eq!(projections.active_dates(), vec!["2026-08-22".to_string()]);
}

#[tokio::test]
async fn cleanup_service_supersedes_orphan_race_projections_without_live_race() {
    let projections = InMemoryProjectionRepository::with_days(vec![
        named_day("2026-08-15", "Race Openers"),
        named_day("2026-08-16", "Szosomania C Race"),
        named_day("2026-08-22", "Race Openers"),
        named_day("2026-08-23", "Warka B Race"),
    ]);
    let cleanup = RaceProjectionCleanupService::new(projections.clone(), TestClock);
    let present = std::collections::BTreeSet::from(["2026-08-16".to_string()]);

    cleanup
        .supersede_orphan_race_projections("user-1", "2026-08-01", "2026-08-31", &present)
        .await
        .unwrap();

    assert_eq!(
        projections.active_dates(),
        vec!["2026-08-15".to_string(), "2026-08-16".to_string()]
    );
}

#[tokio::test]
async fn delete_race_supersedes_projections_on_distinct_imported_twin_date() {
    let local = Race {
        race_id: "race-1".to_string(),
        user_id: "user-1".to_string(),
        date: "2026-09-12".to_string(),
        name: "Local Race".to_string(),
        distance_meters: 90_000,
        discipline: RaceDiscipline::Road,
        priority: RacePriority::B,
        result: None,
        created_at_epoch_seconds: 1,
        updated_at_epoch_seconds: 2,
    };
    let twin = Race {
        race_id: imported_intervals_race_id(88),
        user_id: "user-1".to_string(),
        date: "2026-09-13".to_string(),
        name: "Imported Twin".to_string(),
        distance_meters: 90_000,
        discipline: RaceDiscipline::Road,
        priority: RacePriority::B,
        result: None,
        created_at_epoch_seconds: 1,
        updated_at_epoch_seconds: 2,
    };
    let projections = InMemoryProjectionRepository::with_days(vec![
        named_day("2026-09-12", "Local B Race"),
        named_day("2026-09-13", "Twin C Race"),
        named_day("2026-09-14", "Aerobic Endurance"),
    ]);
    let sync_states = InMemoryExternalSyncStateRepository::default();
    sync_states
        .upsert(
            ExternalSyncState::new(
                "user-1".to_string(),
                ExternalProvider::Intervals,
                CanonicalEntityRef::new(CanonicalEntityKind::Race, "race-1".to_string()),
            )
            .mark_synced("88".to_string(), "hash-local".to_string(), 2),
        )
        .await
        .expect("infallible sync state upsert");
    let cleanup = RaceProjectionCleanupService::new(projections.clone(), TestClock);
    let calendar_refresh = RecordingCalendarRefresh::default();
    let service = RaceService::new(
        InMemoryRaceRepository::with_races(vec![local, twin]),
        RecordingIntervalsService::default(),
        sync_states,
        TestClock,
        TestIdGenerator::default(),
    )
    .with_calendar_view_refresh(calendar_refresh.clone())
    .with_projection_cleanup(cleanup);

    service.delete_race("user-1", "race-1").await.unwrap();

    assert_eq!(projections.active_dates(), vec!["2026-09-14".to_string()]);
    assert_eq!(
        calendar_refresh.stored(),
        vec![(
            "user-1".to_string(),
            "2026-09-12".to_string(),
            "2026-09-13".to_string()
        )]
    );
}
