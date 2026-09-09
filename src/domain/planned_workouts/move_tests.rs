use std::sync::{Arc, Mutex};

use crate::domain::{
    calendar::NoopWahooUseCases,
    completed_workouts::{
        BoxFuture as CompletedBoxFuture, CompletedWorkout, CompletedWorkoutError,
        CompletedWorkoutRepository,
    },
    external_sync::ExternalProvider,
    intervals::{DateRange, IntervalsUseCases},
    planned_completed_links::{
        BoxFuture as LinkBoxFuture, PlannedCompletedWorkoutLink, PlannedCompletedWorkoutLinkError,
        PlannedCompletedWorkoutLinkRepository,
    },
    planned_workouts::{
        PlannedWorkout, PlannedWorkoutContent, PlannedWorkoutLine, PlannedWorkoutText,
    },
    races::{BoxFuture as RaceBoxFuture, Race, RaceError, RaceRepository},
    training_plan::{
        BoxFuture as ProjectionBoxFuture, TrainingPlanError, TrainingPlanProjectedDay,
        TrainingPlanProjectionRepository, TrainingPlanReplacementResult, TrainingPlanSnapshot,
    },
    wahoo::WahooUseCases,
};

use super::{rekey_planned_workout_id, MovePlannedWorkoutCommand, PlannedWorkoutMoveService};
use crate::domain::planned_workouts::update::tests::support::{
    FixedClock, InMemoryExternalSyncStateRepository, RecordingCalendarRefresh,
    RecordingIntervalsService, RecordingPlannedWorkoutRepository, RecordingWahooService,
};

#[test]
fn rekey_updates_date_suffix() {
    assert_eq!(
        rekey_planned_workout_id(
            "training-plan:user-1:w1:2026-05-10",
            "2026-05-10",
            "2026-05-12"
        ),
        "training-plan:user-1:w1:2026-05-12"
    );
}

#[test]
fn rekey_leaves_non_date_ids_alone() {
    assert_eq!(
        rekey_planned_workout_id("intervals-event:77", "2026-05-10", "2026-05-12"),
        "intervals-event:77"
    );
}

#[tokio::test]
async fn move_rekeys_imported_and_refreshes_both_days() {
    let planned = RecordingPlannedWorkoutRepository::with_workouts(vec![existing_workout()]);
    let refresh = RecordingCalendarRefresh::default();
    let projections = Arc::new(RecordingProjectionRepository::default());
    let service = build_service(
        planned.clone(),
        InMemoryExternalSyncStateRepository::default(),
        RecordingIntervalsService::default(),
        NoopWahooUseCases,
        projections.clone(),
        EmptyCompletedRepository,
        Arc::new(EmptyRaceRepository),
        EmptyLinkRepository,
        refresh.clone(),
    );

    let outcome = service
        .move_planned_workout(move_command())
        .await
        .expect("move should succeed");

    assert_eq!(
        outcome.planned_workout.planned_workout_id,
        "training-plan:user-1:w1:2026-05-12"
    );
    assert_eq!(outcome.planned_workout.date, "2026-05-12");
    assert_eq!(
        planned.stored().len(),
        1,
        "old id should be deleted after rekey"
    );
    assert_eq!(
        planned.stored()[0].planned_workout_id,
        "training-plan:user-1:w1:2026-05-12"
    );
    assert_eq!(
        projections.superseded(),
        vec![vec!["2026-05-10".to_string(), "2026-05-12".to_string()]]
    );
    assert!(projections.relocated().is_empty());
    assert_eq!(
        refresh.calls(),
        vec![
            (
                "user-1".to_string(),
                "2026-05-10".to_string(),
                "2026-05-10".to_string()
            ),
            (
                "user-1".to_string(),
                "2026-05-12".to_string(),
                "2026-05-12".to_string()
            ),
        ]
    );
}

#[tokio::test]
async fn move_rejects_race_day_target() {
    let planned = RecordingPlannedWorkoutRepository::with_workouts(vec![existing_workout()]);
    let service = build_service(
        planned,
        InMemoryExternalSyncStateRepository::default(),
        RecordingIntervalsService::default(),
        NoopWahooUseCases,
        Arc::new(RecordingProjectionRepository::default()),
        EmptyCompletedRepository,
        Arc::new(RaceOnDateRepository {
            date: "2026-05-12".to_string(),
        }),
        EmptyLinkRepository,
        RecordingCalendarRefresh::default(),
    );

    let error = service
        .move_planned_workout(move_command())
        .await
        .expect_err("race day should be rejected");
    assert!(error.to_string().contains("race"));
}

#[tokio::test]
async fn move_rejects_occupied_non_rest_target() {
    let planned = RecordingPlannedWorkoutRepository::with_workouts(vec![
        existing_workout(),
        PlannedWorkout::new(
            "other-workout".to_string(),
            "user-1".to_string(),
            "2026-05-12".to_string(),
            PlannedWorkoutContent {
                lines: vec![PlannedWorkoutLine::Text(PlannedWorkoutText {
                    text: "Other".to_string(),
                })],
            },
        ),
    ]);
    let service = build_service(
        planned,
        InMemoryExternalSyncStateRepository::default(),
        RecordingIntervalsService::default(),
        NoopWahooUseCases,
        Arc::new(RecordingProjectionRepository::default()),
        EmptyCompletedRepository,
        Arc::new(EmptyRaceRepository),
        EmptyLinkRepository,
        RecordingCalendarRefresh::default(),
    );

    let error = service
        .move_planned_workout(move_command())
        .await
        .expect_err("occupied day should be rejected");
    assert!(error.to_string().contains("planned workout"));
}

#[tokio::test]
async fn move_allows_rest_target_and_clears_rest_import() {
    let mut rest = PlannedWorkout::new(
        "rest-day-1".to_string(),
        "user-1".to_string(),
        "2026-05-12".to_string(),
        PlannedWorkoutContent { lines: vec![] },
    );
    rest.rest_day = true;
    let planned = RecordingPlannedWorkoutRepository::with_workouts(vec![existing_workout(), rest]);
    let service = build_service(
        planned.clone(),
        InMemoryExternalSyncStateRepository::default(),
        RecordingIntervalsService::default(),
        NoopWahooUseCases,
        Arc::new(RecordingProjectionRepository::default()),
        EmptyCompletedRepository,
        Arc::new(EmptyRaceRepository),
        EmptyLinkRepository,
        RecordingCalendarRefresh::default(),
    );

    service
        .move_planned_workout(move_command())
        .await
        .expect("rest target should be allowed");

    assert!(planned
        .stored()
        .iter()
        .all(|workout| workout.planned_workout_id != "rest-day-1"));
}

#[tokio::test]
async fn move_deletes_intervals_and_wahoo_remotes() {
    let shared_log = Arc::new(Mutex::new(Vec::new()));
    let planned = RecordingPlannedWorkoutRepository::with_workouts_and_shared_log(
        vec![existing_workout()],
        shared_log.clone(),
    );
    let sync_states = InMemoryExternalSyncStateRepository::with_states_and_shared_log(
        vec![intervals_sync_state(), wahoo_sync_state()],
        shared_log.clone(),
    );
    let intervals = RecordingIntervalsService::with_shared_log(shared_log.clone());
    let wahoo = RecordingWahooService::successful(shared_log.clone());
    let service = build_service(
        planned,
        sync_states.clone(),
        intervals,
        wahoo,
        Arc::new(RecordingProjectionRepository::default()),
        EmptyCompletedRepository,
        Arc::new(EmptyRaceRepository),
        EmptyLinkRepository,
        RecordingCalendarRefresh::default(),
    );

    let outcome = service
        .move_planned_workout(move_command())
        .await
        .expect("move with remotes should succeed");

    assert!(outcome
        .deleted_providers
        .contains(&ExternalProvider::Intervals));
    assert!(outcome.deleted_providers.contains(&ExternalProvider::Wahoo));
    assert!(sync_states.stored().is_empty());
    let log = shared_log.lock().expect("log poisoned").clone();
    assert!(
        log.iter()
            .any(|entry| entry.contains("delete_event") || entry.contains("intervals.delete")),
        "expected intervals delete in {log:?}"
    );
    assert!(log
        .iter()
        .any(|entry| entry.contains("wahoo.delete_workout:6001")));
    assert!(log
        .iter()
        .any(|entry| entry.contains("wahoo.delete_plan:5001")));
}

#[tokio::test]
async fn move_loads_projected_only_source_and_upserts_imported() {
    let planned = RecordingPlannedWorkoutRepository::with_workouts(vec![]);
    let projections = Arc::new(RecordingProjectionRepository::with_active_day(
        projected_day("2026-05-10", false),
    ));
    let service = build_service(
        planned.clone(),
        InMemoryExternalSyncStateRepository::default(),
        RecordingIntervalsService::default(),
        NoopWahooUseCases,
        projections.clone(),
        EmptyCompletedRepository,
        Arc::new(EmptyRaceRepository),
        EmptyLinkRepository,
        RecordingCalendarRefresh::default(),
    );

    let outcome = service
        .move_planned_workout(move_command())
        .await
        .expect("projected-only move should succeed");

    assert_eq!(
        outcome.planned_workout.planned_workout_id,
        "training-plan:user-1:w1:2026-05-12"
    );
    assert_eq!(planned.stored().len(), 1);
    assert!(projections.relocated().is_empty());
    assert_eq!(
        projections.superseded(),
        vec![vec!["2026-05-10".to_string(), "2026-05-12".to_string()]]
    );
    assert!(
        projections
            .active_days()
            .iter()
            .all(|day| day.superseded_at_epoch_seconds.is_some()),
        "source projection should be superseded after materialize-then-move"
    );
}

#[tokio::test]
async fn move_rejects_projected_non_rest_target() {
    let planned = RecordingPlannedWorkoutRepository::with_workouts(vec![existing_workout()]);
    let projections = Arc::new(RecordingProjectionRepository::with_active_day(
        projected_day("2026-05-12", false),
    ));
    let service = build_service(
        planned,
        InMemoryExternalSyncStateRepository::default(),
        RecordingIntervalsService::default(),
        NoopWahooUseCases,
        projections,
        EmptyCompletedRepository,
        Arc::new(EmptyRaceRepository),
        EmptyLinkRepository,
        RecordingCalendarRefresh::default(),
    );

    let error = service
        .move_planned_workout(move_command())
        .await
        .expect_err("projected target workout should be rejected");
    assert!(error.to_string().contains("planned workout"));
}

fn projected_day(date: &str, rest_day: bool) -> TrainingPlanProjectedDay {
    TrainingPlanProjectedDay {
        user_id: "user-1".to_string(),
        workout_id: "w1".to_string(),
        operation_key: "training-plan:user-1:w1".to_string(),
        date: date.to_string(),
        rest_day,
        rest_day_reason: None,
        workout: if rest_day {
            None
        } else {
            Some(crate::domain::intervals::PlannedWorkout {
                lines: vec![crate::domain::intervals::PlannedWorkoutLine::Text(
                    crate::domain::intervals::PlannedWorkoutText {
                        text: "Projected Session".to_string(),
                    },
                )],
            })
        },
        superseded_at_epoch_seconds: None,
        created_at_epoch_seconds: 1,
        updated_at_epoch_seconds: 1,
    }
}

fn existing_workout() -> PlannedWorkout {
    PlannedWorkout::new(
        "training-plan:user-1:w1:2026-05-10".to_string(),
        "user-1".to_string(),
        "2026-05-10".to_string(),
        PlannedWorkoutContent {
            lines: vec![PlannedWorkoutLine::Text(PlannedWorkoutText {
                text: "Existing Session".to_string(),
            })],
        },
    )
}

fn move_command() -> MovePlannedWorkoutCommand {
    MovePlannedWorkoutCommand {
        user_id: "user-1".to_string(),
        planned_workout_id: "training-plan:user-1:w1:2026-05-10".to_string(),
        from_date: "2026-05-10".to_string(),
        to_date: "2026-05-12".to_string(),
    }
}

fn intervals_sync_state() -> crate::domain::external_sync::ExternalSyncState {
    crate::domain::planned_workouts::update::tests::fixtures::intervals_sync_state()
}

fn wahoo_sync_state() -> crate::domain::external_sync::ExternalSyncState {
    crate::domain::planned_workouts::update::tests::fixtures::wahoo_sync_state()
}

#[allow(clippy::too_many_arguments)]
fn build_service<Planned, SyncStates, Intervals, Wahoo, Completed, Links, Refresh>(
    planned: Planned,
    sync_states: SyncStates,
    intervals: Intervals,
    wahoo: Wahoo,
    projections: Arc<dyn TrainingPlanProjectionRepository>,
    completed: Completed,
    races: Arc<dyn RaceRepository>,
    links: Links,
    refresh: Refresh,
) -> PlannedWorkoutMoveService<
    Planned,
    SyncStates,
    Intervals,
    Wahoo,
    Completed,
    Links,
    Refresh,
    FixedClock,
>
where
    Planned: crate::domain::planned_workouts::PlannedWorkoutRepository,
    SyncStates: crate::domain::external_sync::ExternalSyncStateRepository,
    Intervals: IntervalsUseCases + Clone,
    Wahoo: WahooUseCases + Clone,
    Completed: CompletedWorkoutRepository,
    Links: PlannedCompletedWorkoutLinkRepository,
    Refresh: crate::domain::calendar_view::CalendarEntryViewRefreshPort,
{
    PlannedWorkoutMoveService::new(
        planned,
        sync_states,
        intervals,
        wahoo,
        projections,
        completed,
        races,
        links,
        refresh,
        FixedClock,
    )
}

#[derive(Clone, Default)]
struct EmptyCompletedRepository;

impl CompletedWorkoutRepository for EmptyCompletedRepository {
    fn find_by_user_id_and_completed_workout_id(
        &self,
        _user_id: &str,
        _completed_workout_id: &str,
    ) -> CompletedBoxFuture<Result<Option<CompletedWorkout>, CompletedWorkoutError>> {
        Box::pin(async { Ok(None) })
    }

    fn find_by_user_id_and_source_activity_id(
        &self,
        _user_id: &str,
        _source_activity_id: &str,
    ) -> CompletedBoxFuture<Result<Option<CompletedWorkout>, CompletedWorkoutError>> {
        Box::pin(async { Ok(None) })
    }

    fn find_latest_by_user_id(
        &self,
        _user_id: &str,
    ) -> CompletedBoxFuture<Result<Option<CompletedWorkout>, CompletedWorkoutError>> {
        Box::pin(async { Ok(None) })
    }

    fn list_by_user_id(
        &self,
        _user_id: &str,
    ) -> CompletedBoxFuture<Result<Vec<CompletedWorkout>, CompletedWorkoutError>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn list_by_user_id_and_date_range(
        &self,
        _user_id: &str,
        _oldest: &str,
        _newest: &str,
    ) -> CompletedBoxFuture<Result<Vec<CompletedWorkout>, CompletedWorkoutError>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn upsert(
        &self,
        workout: CompletedWorkout,
    ) -> CompletedBoxFuture<Result<CompletedWorkout, CompletedWorkoutError>> {
        Box::pin(async move { Ok(workout) })
    }
}

#[derive(Clone, Default)]
struct EmptyLinkRepository;

impl PlannedCompletedWorkoutLinkRepository for EmptyLinkRepository {
    fn find_by_planned_workout_id(
        &self,
        _user_id: &str,
        _planned_workout_id: &str,
    ) -> LinkBoxFuture<Result<Option<PlannedCompletedWorkoutLink>, PlannedCompletedWorkoutLinkError>>
    {
        Box::pin(async { Ok(None) })
    }

    fn find_by_completed_workout_id(
        &self,
        _user_id: &str,
        _completed_workout_id: &str,
    ) -> LinkBoxFuture<Result<Option<PlannedCompletedWorkoutLink>, PlannedCompletedWorkoutLinkError>>
    {
        Box::pin(async { Ok(None) })
    }

    fn find_by_planned_workout_ids(
        &self,
        _user_id: &str,
        _planned_workout_ids: &[String],
    ) -> LinkBoxFuture<Result<Vec<PlannedCompletedWorkoutLink>, PlannedCompletedWorkoutLinkError>>
    {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn upsert(
        &self,
        link: PlannedCompletedWorkoutLink,
    ) -> LinkBoxFuture<Result<PlannedCompletedWorkoutLink, PlannedCompletedWorkoutLinkError>> {
        Box::pin(async move { Ok(link) })
    }

    fn delete_by_completed_workout_id(
        &self,
        _user_id: &str,
        _completed_workout_id: &str,
    ) -> LinkBoxFuture<Result<(), PlannedCompletedWorkoutLinkError>> {
        Box::pin(async { Ok(()) })
    }
}

struct EmptyRaceRepository;

impl RaceRepository for EmptyRaceRepository {
    fn list_by_user_id(&self, _user_id: &str) -> RaceBoxFuture<Result<Vec<Race>, RaceError>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn list_by_user_id_and_range(
        &self,
        _user_id: &str,
        _range: &DateRange,
    ) -> RaceBoxFuture<Result<Vec<Race>, RaceError>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn find_by_user_id_and_race_id(
        &self,
        _user_id: &str,
        _race_id: &str,
    ) -> RaceBoxFuture<Result<Option<Race>, RaceError>> {
        Box::pin(async { Ok(None) })
    }

    fn upsert(&self, race: Race) -> RaceBoxFuture<Result<Race, RaceError>> {
        Box::pin(async move { Ok(race) })
    }

    fn delete(&self, _user_id: &str, _race_id: &str) -> RaceBoxFuture<Result<(), RaceError>> {
        Box::pin(async { Ok(()) })
    }
}

struct RaceOnDateRepository {
    date: String,
}

impl RaceRepository for RaceOnDateRepository {
    fn list_by_user_id(&self, _user_id: &str) -> RaceBoxFuture<Result<Vec<Race>, RaceError>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn list_by_user_id_and_range(
        &self,
        _user_id: &str,
        range: &DateRange,
    ) -> RaceBoxFuture<Result<Vec<Race>, RaceError>> {
        let date = self.date.clone();
        let range = range.clone();
        Box::pin(async move {
            if range.oldest <= date && date <= range.newest {
                Ok(vec![Race {
                    race_id: "race-1".to_string(),
                    user_id: "user-1".to_string(),
                    date,
                    name: "Race".to_string(),
                    distance_meters: 40_000,
                    discipline: crate::domain::races::RaceDiscipline::Road,
                    priority: crate::domain::races::RacePriority::A,
                    result: None,
                    created_at_epoch_seconds: 1,
                    updated_at_epoch_seconds: 1,
                }])
            } else {
                Ok(Vec::new())
            }
        })
    }

    fn find_by_user_id_and_race_id(
        &self,
        _user_id: &str,
        _race_id: &str,
    ) -> RaceBoxFuture<Result<Option<Race>, RaceError>> {
        Box::pin(async { Ok(None) })
    }

    fn upsert(&self, race: Race) -> RaceBoxFuture<Result<Race, RaceError>> {
        Box::pin(async move { Ok(race) })
    }

    fn delete(&self, _user_id: &str, _race_id: &str) -> RaceBoxFuture<Result<(), RaceError>> {
        Box::pin(async { Ok(()) })
    }
}

#[derive(Default)]
struct RecordingProjectionRepository {
    relocated: Mutex<Vec<(String, String, String)>>,
    superseded: Mutex<Vec<Vec<String>>>,
    active: Mutex<Vec<TrainingPlanProjectedDay>>,
}

impl RecordingProjectionRepository {
    fn with_active_day(day: TrainingPlanProjectedDay) -> Self {
        Self {
            relocated: Mutex::new(Vec::new()),
            superseded: Mutex::new(Vec::new()),
            active: Mutex::new(vec![day]),
        }
    }

    fn relocated(&self) -> Vec<(String, String, String)> {
        self.relocated.lock().expect("poisoned").clone()
    }

    fn superseded(&self) -> Vec<Vec<String>> {
        self.superseded.lock().expect("poisoned").clone()
    }

    fn active_days(&self) -> Vec<TrainingPlanProjectedDay> {
        self.active.lock().expect("poisoned").clone()
    }
}

impl TrainingPlanProjectionRepository for RecordingProjectionRepository {
    fn list_active_by_user_id(
        &self,
        user_id: &str,
    ) -> ProjectionBoxFuture<Result<Vec<TrainingPlanProjectedDay>, TrainingPlanError>> {
        let user_id = user_id.to_string();
        let days = self.active.lock().expect("poisoned").clone();
        Box::pin(async move {
            Ok(days
                .into_iter()
                .filter(|day| day.user_id == user_id && day.superseded_at_epoch_seconds.is_none())
                .collect())
        })
    }

    fn find_active_by_operation_key(
        &self,
        _operation_key: &str,
    ) -> ProjectionBoxFuture<Result<Vec<TrainingPlanProjectedDay>, TrainingPlanError>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn find_active_by_user_id_and_operation_key(
        &self,
        _user_id: &str,
        _operation_key: &str,
    ) -> ProjectionBoxFuture<Result<Vec<TrainingPlanProjectedDay>, TrainingPlanError>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn replace_window(
        &self,
        snapshot: TrainingPlanSnapshot,
        projected_days: Vec<TrainingPlanProjectedDay>,
        _today: &str,
        _replaced_at_epoch_seconds: i64,
    ) -> ProjectionBoxFuture<Result<TrainingPlanReplacementResult, TrainingPlanError>> {
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
    ) -> ProjectionBoxFuture<Result<Option<(String, String)>, TrainingPlanError>> {
        self.superseded
            .lock()
            .expect("poisoned")
            .push(dates.to_vec());
        {
            let mut active = self.active.lock().expect("poisoned");
            for day in active.iter_mut() {
                if day.user_id == user_id
                    && dates.iter().any(|date| date == &day.date)
                    && day.superseded_at_epoch_seconds.is_none()
                {
                    day.superseded_at_epoch_seconds = Some(superseded_at_epoch_seconds);
                }
            }
        }
        let dates = dates.to_vec();
        Box::pin(async move { Ok(dates.first().cloned().zip(dates.last().cloned())) })
    }

    fn relocate_active_date(
        &self,
        user_id: &str,
        from_date: &str,
        to_date: &str,
        _updated_at_epoch_seconds: i64,
    ) -> ProjectionBoxFuture<Result<Option<TrainingPlanProjectedDay>, TrainingPlanError>> {
        self.relocated.lock().expect("poisoned").push((
            user_id.to_string(),
            from_date.to_string(),
            to_date.to_string(),
        ));
        Box::pin(async { Ok(None) })
    }
}
