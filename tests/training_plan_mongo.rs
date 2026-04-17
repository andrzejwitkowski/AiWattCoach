use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use aiwattcoach::{
    adapters::mongo::{
        training_plan_generation_operations::MongoTrainingPlanGenerationOperationRepository,
        training_plan_projections::MongoTrainingPlanProjectionRepository,
        training_plan_snapshots::MongoTrainingPlanSnapshotRepository,
    },
    domain::{
        ai_workflow::{ValidationIssue, WorkflowPhase, WorkflowStatus},
        intervals::{
            PlannedWorkout, PlannedWorkoutLine, PlannedWorkoutStep, PlannedWorkoutStepKind,
            PlannedWorkoutTarget, PlannedWorkoutText,
        },
        training_plan::{
            TrainingPlanDay, TrainingPlanGenerationClaimResult, TrainingPlanGenerationOperation,
            TrainingPlanGenerationOperationRepository, TrainingPlanProjectedDay,
            TrainingPlanProjectionRepository, TrainingPlanSnapshot, TrainingPlanSnapshotRepository,
        },
    },
    Settings,
};
use futures::TryStreamExt;
use mongodb::{
    bson::{doc, Document},
    Client,
};

static TEST_DB_COUNTER: AtomicU64 = AtomicU64::new(0);

#[tokio::test]
async fn training_plan_generation_operation_repository_round_trips_and_reclaims_failed_operations()
{
    let Some(fixture) = mongo_fixture_or_skip().await else {
        return;
    };
    let repository = MongoTrainingPlanGenerationOperationRepository::new(
        fixture.client.clone(),
        &fixture.database,
    );
    repository.ensure_indexes().await.unwrap();

    let pending = sample_operation("training-plan:user-1:workout-1:1700000000");
    let claim = repository
        .claim_pending(pending.clone(), 1_699_999_900)
        .await
        .unwrap();

    assert_eq!(
        claim,
        TrainingPlanGenerationClaimResult::Claimed(pending.clone())
    );

    let failed = pending.mark_failed(
        WorkflowPhase::Correction,
        "validation failed".to_string(),
        vec![ValidationIssue {
            scope: "2026-04-10".to_string(),
            message: "invalid day".to_string(),
        }],
        1_700_000_100,
    );
    repository.upsert(failed.clone()).await.unwrap();

    let reclaimed = repository
        .claim_pending(
            sample_operation("training-plan:user-1:workout-1:1700000000"),
            1_700_000_200,
        )
        .await
        .unwrap();

    match reclaimed {
        TrainingPlanGenerationClaimResult::Claimed(operation) => {
            assert_eq!(operation.status, WorkflowStatus::Pending);
            assert_eq!(operation.attempt_count, failed.attempt_count + 1);
            assert_eq!(operation.failure, None);
        }
        other => panic!("expected reclaimed operation, got {other:?}"),
    }

    fixture.cleanup().await;
}

#[tokio::test]
async fn training_plan_generation_operation_repository_round_trips_recap_timestamp() {
    let Some(fixture) = mongo_fixture_or_skip().await else {
        return;
    };
    let repository = MongoTrainingPlanGenerationOperationRepository::new(
        fixture.client.clone(),
        &fixture.database,
    );
    repository.ensure_indexes().await.unwrap();

    let operation = sample_operation("training-plan:user-1:workout-1:1700000000")
        .with_workout_recap(
            "Strong day".to_string(),
            "gemini".to_string(),
            "gemini-3.1-pro".to_string(),
            1_699_999_400,
        )
        .mark_completed(1_700_000_000);
    repository.upsert(operation.clone()).await.unwrap();

    let found = repository
        .find_by_operation_key(&operation.operation_key)
        .await
        .unwrap()
        .expect("expected stored operation");

    assert_eq!(
        found.workout_recap_generated_at_epoch_seconds,
        Some(1_699_999_400)
    );
    assert_eq!(found.updated_at_epoch_seconds, 1_700_000_000);

    fixture.cleanup().await;
}

#[tokio::test]
async fn training_plan_snapshot_repository_finds_snapshot_by_operation_key() {
    let Some(fixture) = mongo_fixture_or_skip().await else {
        return;
    };
    let projection_repository =
        MongoTrainingPlanProjectionRepository::new(fixture.client.clone(), &fixture.database);
    projection_repository.ensure_indexes().await.unwrap();
    let snapshot_repository =
        MongoTrainingPlanSnapshotRepository::new(fixture.client.clone(), &fixture.database);

    let snapshot = sample_snapshot("training-plan:user-1:workout-1:1700000000", "2026-04-06");
    projection_repository
        .replace_window(
            snapshot.clone(),
            sample_projected_days(&snapshot, "2026-04-06"),
            "2026-04-06",
            1_700_000_000,
        )
        .await
        .unwrap();

    let found = snapshot_repository
        .find_by_operation_key(&snapshot.operation_key)
        .await
        .unwrap();

    assert_eq!(found, Some(snapshot));

    fixture.cleanup().await;
}

#[tokio::test]
async fn training_plan_snapshot_repository_reads_legacy_days_without_rest_day_fields() {
    let Some(fixture) = mongo_fixture_or_skip().await else {
        return;
    };
    let repository =
        MongoTrainingPlanSnapshotRepository::new(fixture.client.clone(), &fixture.database);
    repository.ensure_indexes().await.unwrap();

    fixture
        .client
        .database(&fixture.database)
        .collection::<Document>("training_plan_snapshots")
        .insert_one(doc! {
            "user_id": "user-1",
            "workout_id": "workout-1",
            "operation_key": "training-plan:user-1:legacy-snapshot",
            "saved_at_epoch_seconds": 1_700_000_000_i64,
            "start_date": "2026-04-06",
            "end_date": "2026-04-19",
            "days": [
                {
                    "date": "2026-04-06",
                    "workout": {
                        "lines": [
                            { "kind": "text", "text": "AI Threshold" },
                            {
                                "kind": "step",
                                "duration_seconds": 600,
                                "step_kind": "steady",
                                "percent_min": 92.0,
                                "percent_max": 97.0,
                                "watts_min": mongodb::bson::Bson::Null,
                                "watts_max": mongodb::bson::Bson::Null,
                            },
                        ],
                    },
                },
            ],
            "created_at_epoch_seconds": 1_700_000_000_i64,
        })
        .await
        .unwrap();

    let found = repository
        .find_by_operation_key("training-plan:user-1:legacy-snapshot")
        .await
        .unwrap()
        .expect("expected stored snapshot");

    assert_eq!(found.days.len(), 1);
    assert!(!found.days[0].rest_day);
    assert_eq!(found.days[0].rest_day_reason, None);

    fixture.cleanup().await;
}

#[tokio::test]
async fn training_plan_projection_repository_replaces_window_and_supersedes_overlapping_future_days(
) {
    let Some(fixture) = mongo_fixture_or_skip().await else {
        return;
    };
    let repository =
        MongoTrainingPlanProjectionRepository::new(fixture.client.clone(), &fixture.database);
    repository.ensure_indexes().await.unwrap();

    let first_snapshot = sample_snapshot("training-plan:user-1:workout-1:1700000000", "2026-04-06");
    repository
        .replace_window(
            first_snapshot.clone(),
            sample_projected_days(&first_snapshot, "2026-04-06"),
            "2026-04-06",
            1_700_000_000,
        )
        .await
        .unwrap();

    let second_snapshot =
        sample_snapshot("training-plan:user-1:workout-1:1700086400", "2026-04-07");
    let (_, projected_days) = repository
        .replace_window(
            second_snapshot.clone(),
            sample_projected_days(&second_snapshot, "2026-04-07"),
            "2026-04-07",
            1_700_086_400,
        )
        .await
        .unwrap();

    let other_user_snapshot = sample_snapshot_for_user(
        "user-2",
        "workout-9",
        "training-plan:user-2:workout-9:1700086400",
        "2026-04-07",
    );
    repository
        .replace_window(
            other_user_snapshot.clone(),
            sample_projected_days(&other_user_snapshot, "2026-04-07"),
            "2026-04-07",
            1_700_086_400,
        )
        .await
        .unwrap();

    assert_eq!(projected_days.len(), 14);

    let active_for_user = repository.list_active_by_user_id("user-1").await.unwrap();
    assert!(active_for_user
        .iter()
        .all(|day| day.operation_key == second_snapshot.operation_key));
    assert!(!active_for_user.iter().any(|day| day.date == "2026-04-07"));
    assert!(!active_for_user
        .iter()
        .any(|day| day.operation_key == other_user_snapshot.operation_key));

    let first_active = repository
        .find_active_by_operation_key(&first_snapshot.operation_key)
        .await
        .unwrap();
    assert!(first_active.is_empty());

    fixture.cleanup().await;
}

#[tokio::test]
async fn training_plan_projection_repository_reads_legacy_projected_days_without_rest_day_fields() {
    let Some(fixture) = mongo_fixture_or_skip().await else {
        return;
    };
    let repository =
        MongoTrainingPlanProjectionRepository::new(fixture.client.clone(), &fixture.database);
    repository.ensure_indexes().await.unwrap();

    fixture
        .client
        .database(&fixture.database)
        .collection::<Document>("training_plan_snapshots")
        .insert_one(doc! {
            "user_id": "user-1",
            "workout_id": "workout-1",
            "operation_key": "training-plan:user-1:legacy-projection",
            "saved_at_epoch_seconds": 1_700_000_000_i64,
            "start_date": "2026-04-06",
            "end_date": "2026-04-19",
            "days": [
                { "date": "2026-04-06", "workout": mongodb::bson::Bson::Null },
                { "date": "2026-04-07", "workout": mongodb::bson::Bson::Null },
            ],
            "created_at_epoch_seconds": 1_700_000_000_i64,
        })
        .await
        .unwrap();

    fixture
        .client
        .database(&fixture.database)
        .collection::<Document>("training_plan_projected_days")
        .insert_one(doc! {
            "user_id": "user-1",
            "workout_id": "workout-1",
            "operation_key": "training-plan:user-1:legacy-projection",
            "date": "2026-04-07",
            "workout": {
                "lines": [
                    { "kind": "text", "text": "AI Threshold" },
                ],
            },
            "superseded_at_epoch_seconds": mongodb::bson::Bson::Null,
            "created_at_epoch_seconds": 1_700_000_000_i64,
            "updated_at_epoch_seconds": 1_700_000_000_i64,
        })
        .await
        .unwrap();

    let found = repository
        .find_active_by_operation_key("training-plan:user-1:legacy-projection")
        .await
        .unwrap();

    assert_eq!(found.len(), 1);
    assert!(!found[0].rest_day);
    assert_eq!(found[0].rest_day_reason, None);

    fixture.cleanup().await;
}

#[tokio::test]
async fn training_plan_projection_repository_keeps_past_days_active_when_late_window_replacement_runs(
) {
    let Some(fixture) = mongo_fixture_or_skip().await else {
        return;
    };
    let repository =
        MongoTrainingPlanProjectionRepository::new(fixture.client.clone(), &fixture.database);
    repository.ensure_indexes().await.unwrap();

    let first_snapshot = sample_snapshot("training-plan:user-1:workout-1:1700000000", "2026-04-06");
    repository
        .replace_window(
            first_snapshot.clone(),
            sample_projected_days(&first_snapshot, "2026-04-06"),
            "2026-04-06",
            1_700_000_000,
        )
        .await
        .unwrap();

    let replacement_snapshot =
        sample_snapshot("training-plan:user-1:workout-1:1700432000", "2026-04-06");
    repository
        .replace_window(
            replacement_snapshot,
            sample_projected_days(
                &sample_snapshot("training-plan:user-1:workout-1:1700432000", "2026-04-06"),
                "2026-04-10",
            ),
            "2026-04-10",
            1_700_432_000,
        )
        .await
        .unwrap();

    let first_active = repository
        .find_active_by_operation_key(&first_snapshot.operation_key)
        .await
        .unwrap();

    assert!(first_active.iter().any(|day| day.date == "2026-04-07"));
    assert!(first_active.iter().any(|day| day.date == "2026-04-09"));

    fixture.cleanup().await;
}

#[tokio::test]
async fn training_plan_projection_repository_replay_heals_partial_same_operation_inserts() {
    let Some(fixture) = mongo_fixture_or_skip().await else {
        return;
    };
    let repository =
        MongoTrainingPlanProjectionRepository::new(fixture.client.clone(), &fixture.database);
    repository.ensure_indexes().await.unwrap();

    let snapshot = sample_snapshot("training-plan:user-1:workout-1:1700000000", "2026-04-06");
    let partial_projected_days = sample_projected_days(&snapshot, "2026-04-06")
        .into_iter()
        .take(5)
        .collect::<Vec<_>>();

    repository
        .replace_window(
            snapshot.clone(),
            partial_projected_days,
            "2026-04-06",
            1_700_000_000,
        )
        .await
        .unwrap();

    let (_, projected_days) = repository
        .replace_window(
            snapshot.clone(),
            sample_projected_days(&snapshot, "2026-04-06"),
            "2026-04-06",
            1_700_000_100,
        )
        .await
        .unwrap();

    assert_eq!(projected_days.len(), 14);

    let stored_days = fixture
        .client
        .database(&fixture.database)
        .collection::<mongodb::bson::Document>("training_plan_projected_days")
        .find(doc! { "operation_key": &snapshot.operation_key })
        .await
        .unwrap()
        .try_collect::<Vec<_>>()
        .await
        .unwrap();
    assert_eq!(stored_days.len(), 14);

    let active_for_operation = repository
        .find_active_by_operation_key(&snapshot.operation_key)
        .await
        .unwrap();
    assert_eq!(active_for_operation.len(), 13);

    fixture.cleanup().await;
}

#[tokio::test]
async fn training_plan_projection_repository_creates_operation_unsuperseded_date_index() {
    let Some(fixture) = mongo_fixture_or_skip().await else {
        return;
    };
    let repository =
        MongoTrainingPlanProjectionRepository::new(fixture.client.clone(), &fixture.database);
    repository.ensure_indexes().await.unwrap();

    let indexes = fixture
        .client
        .database(&fixture.database)
        .collection::<mongodb::bson::Document>("training_plan_projected_days")
        .list_indexes()
        .await
        .unwrap()
        .try_collect::<Vec<_>>()
        .await
        .unwrap();

    assert!(indexes.iter().any(|index| {
        index
            .options
            .as_ref()
            .and_then(|options| options.name.as_deref())
            == Some("training_plan_projected_days_operation_unsuperseded_date")
            && index.keys
                == doc! { "operation_key": 1, "superseded_at_epoch_seconds": 1, "date": 1 }
    }));

    fixture.cleanup().await;
}

#[tokio::test]
async fn training_plan_snapshot_repository_creates_unique_operation_key_index() {
    let Some(fixture) = mongo_fixture_or_skip().await else {
        return;
    };
    let repository =
        MongoTrainingPlanSnapshotRepository::new(fixture.client.clone(), &fixture.database);
    repository.ensure_indexes().await.unwrap();

    let indexes = fixture
        .client
        .database(&fixture.database)
        .collection::<mongodb::bson::Document>("training_plan_snapshots")
        .list_indexes()
        .await
        .unwrap()
        .try_collect::<Vec<_>>()
        .await
        .unwrap();

    assert!(indexes.iter().any(|index| {
        index
            .options
            .as_ref()
            .and_then(|options| options.name.as_deref())
            == Some("training_plan_snapshots_operation_key_unique")
            && index.keys == doc! { "operation_key": 1 }
            && index.options.as_ref().and_then(|options| options.unique) == Some(true)
    }));

    fixture.cleanup().await;
}

struct MongoFixture {
    client: Client,
    database: String,
}

async fn mongo_fixture_or_skip() -> Option<MongoFixture> {
    match MongoFixture::new().await {
        Ok(fixture) => Some(fixture),
        Err(error) => {
            if std::env::var("REQUIRE_MONGO_IN_CI").as_deref() == Ok("true") {
                panic!("training_plan_mongo test requires Mongo in CI: {error}");
            }
            eprintln!("skipping training_plan_mongo test: {error}");
            None
        }
    }
}

impl MongoFixture {
    async fn new() -> Result<Self, String> {
        let settings = Settings::test_defaults();
        let mongo_uri = settings.mongo.uri.clone();
        let client = Client::with_uri_str(&settings.mongo.uri)
            .await
            .map_err(|error| {
                format!("failed to create test mongo client for {mongo_uri}: {error}")
            })?;
        tokio::time::timeout(
            Duration::from_secs(1),
            client.database("admin").run_command(doc! { "ping": 1 }),
        )
        .await
        .map_err(|_| format!("timed out connecting to Mongo at {mongo_uri}"))?
        .map_err(|error| format!("failed to connect to Mongo at {mongo_uri}: {error}"))?;
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let counter = TEST_DB_COUNTER.fetch_add(1, Ordering::Relaxed);
        let database = format!("aiwattcoach_training_plan_mongo_{unique}_{counter}");
        Ok(Self { client, database })
    }

    async fn cleanup(self) {
        let _ = self.client.database(&self.database).drop().await;
    }
}

fn sample_operation(operation_key: &str) -> TrainingPlanGenerationOperation {
    TrainingPlanGenerationOperation {
        operation_key: operation_key.to_string(),
        user_id: "user-1".to_string(),
        workout_id: "workout-1".to_string(),
        saved_at_epoch_seconds: 1_700_000_000,
        status: WorkflowStatus::Pending,
        workout_recap_text: Some("Strong day".to_string()),
        workout_recap_provider: Some("gemini".to_string()),
        workout_recap_model: Some("gemini-3.1-pro".to_string()),
        workout_recap_generated_at_epoch_seconds: Some(1_699_999_400),
        projection_persisted_at_epoch_seconds: None,
        raw_plan_response: Some("2026-04-06\nrest day".to_string()),
        raw_correction_response: None,
        validation_issues: Vec::new(),
        attempts: Vec::new(),
        failure: None,
        started_at_epoch_seconds: 1_700_000_000,
        last_attempt_at_epoch_seconds: 1_700_000_000,
        attempt_count: 1,
        created_at_epoch_seconds: 1_700_000_000,
        updated_at_epoch_seconds: 1_700_000_000,
    }
}

fn sample_snapshot(operation_key: &str, start_date: &str) -> TrainingPlanSnapshot {
    sample_snapshot_for_user("user-1", "workout-1", operation_key, start_date)
}

fn sample_snapshot_for_user(
    user_id: &str,
    workout_id: &str,
    operation_key: &str,
    start_date: &str,
) -> TrainingPlanSnapshot {
    let start = chrono::NaiveDate::parse_from_str(start_date, "%Y-%m-%d").unwrap();
    let days = (0..14)
        .map(|offset| {
            let date = start
                .checked_add_signed(chrono::Duration::days(offset))
                .unwrap()
                .format("%Y-%m-%d")
                .to_string();
            TrainingPlanDay {
                date,
                rest_day: offset == 0,
                rest_day_reason: (offset == 0)
                    .then(|| "Need recovery after prior block".to_string()),
                workout: (offset != 0).then(sample_planned_workout),
            }
        })
        .collect::<Vec<_>>();
    TrainingPlanSnapshot {
        user_id: user_id.to_string(),
        workout_id: workout_id.to_string(),
        operation_key: operation_key.to_string(),
        saved_at_epoch_seconds: 1_700_000_000,
        start_date: days.first().unwrap().date.clone(),
        end_date: days.last().unwrap().date.clone(),
        days,
        created_at_epoch_seconds: 1_700_000_000,
    }
}

fn sample_projected_days(
    snapshot: &TrainingPlanSnapshot,
    _today: &str,
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
            created_at_epoch_seconds: snapshot.created_at_epoch_seconds,
            updated_at_epoch_seconds: snapshot.created_at_epoch_seconds,
        })
        .collect()
}

fn sample_planned_workout() -> PlannedWorkout {
    PlannedWorkout {
        lines: vec![
            PlannedWorkoutLine::Text(PlannedWorkoutText {
                text: "AI Threshold".to_string(),
            }),
            PlannedWorkoutLine::Step(PlannedWorkoutStep {
                duration_seconds: 600,
                kind: PlannedWorkoutStepKind::Steady,
                target: PlannedWorkoutTarget::PercentFtp {
                    min: 92.0,
                    max: 97.0,
                },
            }),
        ],
    }
}
