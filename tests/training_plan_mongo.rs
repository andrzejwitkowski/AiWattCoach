use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
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
use mongodb::{bson::doc, Client};

static TEST_DB_COUNTER: AtomicU64 = AtomicU64::new(0);

#[tokio::test]
async fn training_plan_generation_operation_repository_round_trips_and_reclaims_failed_operations()
{
    let fixture = MongoFixture::new().await;
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
}

#[tokio::test]
async fn training_plan_generation_operation_repository_round_trips_recap_timestamp() {
    let fixture = MongoFixture::new().await;
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
}

#[tokio::test]
async fn training_plan_snapshot_repository_finds_snapshot_by_operation_key() {
    let fixture = MongoFixture::new().await;
    let projection_repository =
        MongoTrainingPlanProjectionRepository::new(fixture.client.clone(), &fixture.database);
    projection_repository.ensure_indexes().await.unwrap();
    let snapshot_repository =
        MongoTrainingPlanSnapshotRepository::new(fixture.client.clone(), &fixture.database);

    let snapshot = sample_snapshot("training-plan:user-1:workout-1:1700000000", "2026-04-06");
    projection_repository
        .replace_window(
            snapshot.clone(),
            sample_projected_days(&snapshot),
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
}

#[tokio::test]
async fn training_plan_projection_repository_replaces_window_and_supersedes_overlapping_future_days(
) {
    let fixture = MongoFixture::new().await;
    let repository =
        MongoTrainingPlanProjectionRepository::new(fixture.client.clone(), &fixture.database);
    repository.ensure_indexes().await.unwrap();

    let first_snapshot = sample_snapshot("training-plan:user-1:workout-1:1700000000", "2026-04-06");
    repository
        .replace_window(
            first_snapshot.clone(),
            sample_projected_days(&first_snapshot),
            "2026-04-06",
            1_700_000_000,
        )
        .await
        .unwrap();

    let second_snapshot =
        sample_snapshot("training-plan:user-1:workout-1:1700086400", "2026-04-07");
    let (_, active_days) = repository
        .replace_window(
            second_snapshot.clone(),
            sample_projected_days(&second_snapshot),
            "2026-04-07",
            1_700_086_400,
        )
        .await
        .unwrap();

    assert_eq!(active_days.len(), 13);

    let active_for_user = repository.list_active_by_user_id("user-1").await.unwrap();
    assert!(active_for_user
        .iter()
        .all(|day| day.operation_key == second_snapshot.operation_key));
    assert!(!active_for_user.iter().any(|day| day.date == "2026-04-07"));

    let first_active = repository
        .find_active_by_operation_key(&first_snapshot.operation_key)
        .await
        .unwrap();
    assert!(first_active.is_empty());
}

#[tokio::test]
async fn training_plan_projection_repository_keeps_past_days_active_when_late_window_replacement_runs(
) {
    let fixture = MongoFixture::new().await;
    let repository =
        MongoTrainingPlanProjectionRepository::new(fixture.client.clone(), &fixture.database);
    repository.ensure_indexes().await.unwrap();

    let first_snapshot = sample_snapshot("training-plan:user-1:workout-1:1700000000", "2026-04-06");
    repository
        .replace_window(
            first_snapshot.clone(),
            sample_projected_days(&first_snapshot),
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
            sample_projected_days(&sample_snapshot(
                "training-plan:user-1:workout-1:1700432000",
                "2026-04-06",
            )),
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
}

#[tokio::test]
async fn training_plan_projection_repository_replay_heals_partial_same_operation_inserts() {
    let fixture = MongoFixture::new().await;
    let repository =
        MongoTrainingPlanProjectionRepository::new(fixture.client.clone(), &fixture.database);
    repository.ensure_indexes().await.unwrap();

    let snapshot = sample_snapshot("training-plan:user-1:workout-1:1700000000", "2026-04-06");
    let partial_projected_days = sample_projected_days(&snapshot)
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

    let (_, active_days) = repository
        .replace_window(
            snapshot.clone(),
            sample_projected_days(&snapshot),
            "2026-04-06",
            1_700_000_100,
        )
        .await
        .unwrap();

    assert_eq!(active_days.len(), 13);

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
}

#[tokio::test]
async fn training_plan_projection_repository_creates_operation_active_date_index() {
    let fixture = MongoFixture::new().await;
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
            == Some("training_plan_projected_days_operation_active_date")
            && index.keys == doc! { "operation_key": 1, "active": 1, "date": 1 }
    }));
}

struct MongoFixture {
    client: Client,
    database: String,
}

impl MongoFixture {
    async fn new() -> Self {
        let settings = Settings::test_defaults();
        let client = Client::with_uri_str(&settings.mongo.uri)
            .await
            .expect("test mongo client should be created");
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let counter = TEST_DB_COUNTER.fetch_add(1, Ordering::Relaxed);
        let database = format!("aiwattcoach_training_plan_mongo_{unique}_{counter}");
        Self { client, database }
    }
}

impl Drop for MongoFixture {
    fn drop(&mut self) {
        let client = self.client.clone();
        let database = self.database.clone();
        tokio::spawn(async move {
            let _ = client.database(&database).drop().await;
        });
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
                workout: (offset != 0).then(sample_planned_workout),
            }
        })
        .collect::<Vec<_>>();
    TrainingPlanSnapshot {
        user_id: "user-1".to_string(),
        workout_id: "workout-1".to_string(),
        operation_key: operation_key.to_string(),
        saved_at_epoch_seconds: 1_700_000_000,
        start_date: days.first().unwrap().date.clone(),
        end_date: days.last().unwrap().date.clone(),
        days,
        created_at_epoch_seconds: 1_700_000_000,
    }
}

fn sample_projected_days(snapshot: &TrainingPlanSnapshot) -> Vec<TrainingPlanProjectedDay> {
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
            active: day.date > snapshot.start_date,
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
