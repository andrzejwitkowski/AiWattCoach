use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use aiwattcoach::{
    adapters::mongo::{
        training_plan_generation_operations::MongoTrainingPlanGenerationOperationRepository,
        training_plan_projections::MongoTrainingPlanProjectionRepository,
        training_plan_snapshots::MongoTrainingPlanSnapshotRepository,
        training_plan_supervisor_operations::MongoTrainingPlanSupervisorOperationRepository,
    },
    domain::{
        ai_workflow::{ValidationIssue, WorkflowPhase, WorkflowStatus},
        intervals::{
            PlannedWorkout, PlannedWorkoutLine, PlannedWorkoutStep, PlannedWorkoutStepKind,
            PlannedWorkoutTarget, PlannedWorkoutText,
        },
        llm::{LlmChatMessage, LlmChatResponse, LlmFinishReason, LlmProvider, LlmTokenUsage},
        llm_tools::LlmToolLoopOutput,
        training_plan::{
            TrainingPlanDay, TrainingPlanGenerationClaimResult, TrainingPlanGenerationOperation,
            TrainingPlanGenerationOperationRepository, TrainingPlanProjectedDay,
            TrainingPlanProjectionRepository, TrainingPlanSnapshot, TrainingPlanSnapshotRepository,
        },
        training_plan_supervisor::{
            TrainingPlanSupervisorDecision, TrainingPlanSupervisorOperation,
            TrainingPlanSupervisorOperationRepository, TrainingPlanSupervisorReview,
            TrainingPlanSupervisorStatus,
        },
    },
    Settings,
};
use futures::TryStreamExt;
use mongodb::{
    bson::{doc, Document},
    options::ClientOptions,
    Client,
};

static TEST_DB_COUNTER: AtomicU64 = AtomicU64::new(0);
const TEST_MONGO_SERVER_SELECTION_TIMEOUT: Duration = Duration::from_secs(1);

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
async fn training_plan_generation_operation_repository_round_trips_completed_tool_loop_state() {
    let Some(fixture) = mongo_fixture_or_skip().await else {
        return;
    };
    let repository = MongoTrainingPlanGenerationOperationRepository::new(
        fixture.client.clone(),
        &fixture.database,
    );
    repository.ensure_indexes().await.unwrap();

    let operation_key = "training-plan:user-1:workout-1:1700000000";
    let initial_tool_loop_state = LlmToolLoopOutput::from_response(LlmChatResponse {
        provider: LlmProvider::Gemini,
        model: "gemini-3.1-pro".to_string(),
        message: LlmChatMessage::assistant("2026-04-06\nRest Day"),
        finish_reason: Some(LlmFinishReason::Stop),
        provider_request_id: Some("req-mongo-initial".to_string()),
        usage: LlmTokenUsage::default(),
        cache: Default::default(),
    })
    .state;
    let correction_tool_loop_state = LlmToolLoopOutput::from_response(LlmChatResponse {
        provider: LlmProvider::Gemini,
        model: "gemini-3.1-pro".to_string(),
        message: LlmChatMessage::assistant("2026-04-07\nEndurance\n- 45m 65%"),
        finish_reason: Some(LlmFinishReason::Stop),
        provider_request_id: Some("req-mongo-correction".to_string()),
        usage: LlmTokenUsage::default(),
        cache: Default::default(),
    })
    .state;
    let operation = sample_operation(operation_key)
        .with_raw_plan_response(
            "2026-04-06\nRest Day".to_string(),
            initial_tool_loop_state.clone(),
            1_700_000_050,
        )
        .with_correction_response(
            "2026-04-07\nEndurance\n- 45m 65%".to_string(),
            correction_tool_loop_state.clone(),
            1_700_000_060,
        );
    repository.upsert(operation.clone()).await.unwrap();

    let found = repository
        .find_by_operation_key(operation_key)
        .await
        .unwrap()
        .expect("expected stored operation");

    let initial_completed = found
        .initial_plan_tool_loop_state
        .as_ref()
        .and_then(|state| state.completed_response.as_ref())
        .expect("initial completed response should round-trip");
    assert_eq!(initial_completed.message.content, "2026-04-06\nRest Day");
    assert_eq!(initial_completed.provider, LlmProvider::Gemini);
    assert_eq!(initial_completed.model, "gemini-3.1-pro");
    assert_eq!(initial_completed.finish_reason, Some(LlmFinishReason::Stop));
    assert_eq!(
        initial_completed.provider_request_id.as_deref(),
        Some("req-mongo-initial")
    );

    let correction_completed = found
        .correction_tool_loop_state
        .as_ref()
        .and_then(|state| state.completed_response.as_ref())
        .expect("correction completed response should round-trip");
    assert_eq!(
        correction_completed.message.content,
        "2026-04-07\nEndurance\n- 45m 65%"
    );
    assert_eq!(correction_completed.provider, LlmProvider::Gemini);
    assert_eq!(correction_completed.model, "gemini-3.1-pro");
    assert_eq!(
        correction_completed.finish_reason,
        Some(LlmFinishReason::Stop)
    );
    assert_eq!(
        correction_completed.provider_request_id.as_deref(),
        Some("req-mongo-correction")
    );
    assert_eq!(
        found
            .initial_plan_tool_loop_state
            .as_ref()
            .map(|state| state.round_count),
        Some(1)
    );

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
async fn training_plan_supervisor_operation_repository_round_trips_completed_review() {
    let Some(fixture) = mongo_fixture_or_skip().await else {
        return;
    };
    let repository = MongoTrainingPlanSupervisorOperationRepository::new(
        fixture.client.clone(),
        &fixture.database,
    );
    repository.ensure_indexes().await.unwrap();

    let operation = TrainingPlanSupervisorOperation::pending(
        "training-plan:user-1:workout-1:1700000000".to_string(),
        "user-1".to_string(),
        1_700_000_000,
        "gemini-2.5-pro".to_string(),
        1_700_000_050,
    )
    .complete_review(
        TrainingPlanSupervisorReview {
            decision: TrainingPlanSupervisorDecision::Replace,
            reason: "needs more recovery before intensity".to_string(),
            plan: Some("2026-04-06\nRest Day\n\n2026-04-07\nEndurance\n- 45m 65%".to_string()),
        },
        1_700_000_100,
    )
    .unwrap();
    repository.upsert(operation.clone()).await.unwrap();

    let found = repository
        .find_by_worker_operation_key(&operation.worker_operation_key)
        .await
        .unwrap()
        .expect("expected stored supervisor operation");

    assert_eq!(found.status, TrainingPlanSupervisorStatus::Replaced);
    assert_eq!(
        found.review,
        Some(TrainingPlanSupervisorReview {
            decision: TrainingPlanSupervisorDecision::Replace,
            reason: "needs more recovery before intensity".to_string(),
            plan: Some("2026-04-06\nRest Day\n\n2026-04-07\nEndurance\n- 45m 65%".to_string()),
        })
    );
    assert_eq!(found.updated_at_epoch_seconds, 1_700_000_100);

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
    let result = repository
        .replace_window(
            second_snapshot.clone(),
            sample_projected_days(&second_snapshot, "2026-04-07"),
            "2026-04-07",
            1_700_086_400,
        )
        .await
        .unwrap();
    let projected_days = result.projected_days;

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
    assert!(active_for_user.iter().any(|day| {
        day.operation_key == first_snapshot.operation_key && day.date == "2026-04-06"
    }));
    assert!(active_for_user.iter().any(|day| day.date == "2026-04-07"));
    assert!(active_for_user.iter().all(|day| {
        day.operation_key == first_snapshot.operation_key
            || day.operation_key == second_snapshot.operation_key
    }));
    assert!(!active_for_user
        .iter()
        .any(|day| day.operation_key == other_user_snapshot.operation_key));

    let first_active = repository
        .find_active_by_operation_key(&first_snapshot.operation_key)
        .await
        .unwrap();
    assert_eq!(first_active.len(), 1);
    assert_eq!(first_active[0].date, "2026-04-06");

    fixture.cleanup().await;
}

#[tokio::test]
async fn training_plan_projection_repository_updates_supervisor_status_for_active_operation_days() {
    let Some(fixture) = mongo_fixture_or_skip().await else {
        return;
    };
    let repository =
        MongoTrainingPlanProjectionRepository::new(fixture.client.clone(), &fixture.database);
    repository.ensure_indexes().await.unwrap();

    let snapshot = sample_snapshot("training-plan:user-1:workout-1:1700000000", "2026-04-06");
    repository
        .replace_window(
            snapshot.clone(),
            sample_projected_days(&snapshot, "2026-04-06"),
            "2026-04-06",
            1_700_000_000,
        )
        .await
        .unwrap();

    repository
        .update_supervisor_status(
            &snapshot.user_id,
            &snapshot.operation_key,
            Some(TrainingPlanSupervisorStatus::Accepted),
            1_700_000_200,
        )
        .await
        .unwrap();

    let active = repository
        .find_active_by_operation_key(&snapshot.operation_key)
        .await
        .unwrap();

    assert_eq!(active.len(), 14);
    assert!(active.iter().all(|day| {
        day.supervisor_status == Some(TrainingPlanSupervisorStatus::Accepted)
            && day.updated_at_epoch_seconds == 1_700_000_200
    }));

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

    let result = repository
        .replace_window(
            snapshot.clone(),
            sample_projected_days(&snapshot, "2026-04-06"),
            "2026-04-06",
            1_700_000_100,
        )
        .await
        .unwrap();
    let projected_days = result.projected_days;

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
    assert_eq!(active_for_operation.len(), 14);

    fixture.cleanup().await;
}

#[tokio::test]
async fn training_plan_projection_repository_supersedes_stale_leading_days_for_same_operation_key()
{
    let Some(fixture) = mongo_fixture_or_skip().await else {
        return;
    };
    let repository =
        MongoTrainingPlanProjectionRepository::new(fixture.client.clone(), &fixture.database);
    repository.ensure_indexes().await.unwrap();

    let original_snapshot =
        sample_snapshot("training-plan:user-1:workout-1:1700000000", "2026-05-01");
    repository
        .replace_window(
            original_snapshot.clone(),
            sample_projected_days(&original_snapshot, "2026-05-01"),
            "2026-05-01",
            1_700_000_000,
        )
        .await
        .unwrap();

    let replacement_snapshot =
        sample_snapshot("training-plan:user-1:workout-1:1700000000", "2026-05-02");
    repository
        .replace_window(
            replacement_snapshot.clone(),
            sample_projected_days(&replacement_snapshot, "2026-05-02"),
            "2026-05-02",
            1_700_086_400,
        )
        .await
        .unwrap();

    let stored_days = fixture
        .client
        .database(&fixture.database)
        .collection::<mongodb::bson::Document>("training_plan_projected_days")
        .find(doc! { "operation_key": &replacement_snapshot.operation_key })
        .await
        .unwrap()
        .try_collect::<Vec<_>>()
        .await
        .unwrap();

    let stale_leading_day = stored_days
        .iter()
        .find(|day| day.get_str("date").ok() == Some("2026-05-01"))
        .expect("expected old leading day to remain stored for audit trail");
    assert_eq!(
        stale_leading_day
            .get_i64("superseded_at_epoch_seconds")
            .ok(),
        Some(1_700_086_400)
    );

    let active_for_operation = repository
        .find_active_by_operation_key(&replacement_snapshot.operation_key)
        .await
        .unwrap();
    assert!(!active_for_operation
        .iter()
        .any(|day| day.date == "2026-05-01"));
    assert!(active_for_operation
        .iter()
        .any(|day| day.date == "2026-05-02"));

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
        let mut options = ClientOptions::parse(&settings.mongo.uri)
            .await
            .map_err(|error| format!("failed to create test mongo client: {error}"))?;
        options.server_selection_timeout = Some(TEST_MONGO_SERVER_SELECTION_TIMEOUT);
        let client = Client::with_options(options)
            .map_err(|error| format!("failed to create test mongo client: {error}"))?;
        client
            .database("admin")
            .run_command(doc! { "ping": 1 })
            .await
            .map_err(|error| format!("failed to connect to Mongo: {error}"))?;
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
        initial_plan_tool_loop_state: None,
        raw_correction_response: None,
        correction_tool_loop_state: None,
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
            supervisor_status: None,
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
