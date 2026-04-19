use std::sync::{Arc, Mutex};

use crate::domain::{
    completed_workouts::{CompletedWorkoutAdminUseCases, CompletedWorkoutRepository},
    external_sync::ExternalImportCommand,
};

use super::super::IntervalsCompletedWorkoutBackfillService;
use super::support::{
    sample_complete_workout, sample_detailed_activity, sample_metrics_unbackfillable_activity,
    sample_workout_missing_tss, RecordingImports, RecordingRecomputeService, TestApi, TestClock,
    TestCompletedWorkoutRepository, TestSettings,
};

#[tokio::test]
async fn backfill_missing_metrics_without_date_range_scans_all_user_workouts_and_recomputes_once() {
    let repository = TestCompletedWorkoutRepository::default();
    repository
        .upsert(sample_workout_missing_tss())
        .await
        .unwrap();
    repository.upsert(sample_complete_workout()).await.unwrap();
    let lookups = Arc::new(Mutex::new(Vec::new()));
    let imports = RecordingImports::default();
    let recompute = Arc::new(RecordingRecomputeService::default());
    let service = IntervalsCompletedWorkoutBackfillService::new(
        repository,
        TestSettings,
        TestApi {
            activity: sample_detailed_activity(),
            lookups: lookups.clone(),
        },
        imports.clone(),
        TestClock,
    )
    .with_training_load_recompute_service(recompute.clone());

    let result = service
        .backfill_missing_metrics("user-1", None, None)
        .await
        .unwrap();

    assert_eq!(result.scanned, 2);
    assert_eq!(result.enriched, 1);
    assert_eq!(result.skipped, 1);
    assert_eq!(result.failed, 0);
    assert_eq!(result.recomputed_from.as_deref(), Some("2026-04-16"));
    assert_eq!(lookups.lock().unwrap().as_slice(), &["i1".to_string()]);
    assert_eq!(imports.commands().len(), 1);
    let ExternalImportCommand::UpsertCompletedWorkout(import) = &imports.commands()[0] else {
        panic!("expected completed workout import");
    };
    assert!(import.workout.details.streams.is_empty());
    assert_eq!(import.workout.metrics.training_stress_score, Some(80));
    assert_eq!(import.workout.metrics.ftp_watts, Some(300));
    assert_eq!(
        recompute.calls(),
        vec![(
            "user-1".to_string(),
            "2026-04-16".to_string(),
            1_775_174_400,
        )]
    );
}

#[tokio::test]
async fn backfill_missing_metrics_with_date_range_limits_scanned_workouts() {
    let repository = TestCompletedWorkoutRepository::default();
    repository
        .upsert(sample_workout_missing_tss())
        .await
        .unwrap();
    let mut older = sample_workout_missing_tss();
    older.completed_workout_id = "intervals-activity:i2".to_string();
    older.source_activity_id = Some("i2".to_string());
    older.start_date_local = "2026-04-10T15:28:24".to_string();
    repository.upsert(older).await.unwrap();

    let lookups = Arc::new(Mutex::new(Vec::new()));
    let imports = RecordingImports::default();
    let recompute = Arc::new(RecordingRecomputeService::default());
    let service = IntervalsCompletedWorkoutBackfillService::new(
        repository,
        TestSettings,
        TestApi {
            activity: sample_detailed_activity(),
            lookups: lookups.clone(),
        },
        imports,
        TestClock,
    )
    .with_training_load_recompute_service(recompute.clone());

    let result = service
        .backfill_missing_metrics("user-1", Some("2026-04-16"), Some("2026-04-16"))
        .await
        .unwrap();

    assert_eq!(result.scanned, 1);
    assert_eq!(result.enriched, 1);
    assert_eq!(result.skipped, 0);
    assert_eq!(result.failed, 0);
    assert_eq!(lookups.lock().unwrap().as_slice(), &["i1".to_string()]);
    assert_eq!(
        recompute.calls(),
        vec![(
            "user-1".to_string(),
            "2026-04-16".to_string(),
            1_775_174_400,
        )]
    );
}

#[tokio::test]
async fn backfill_missing_metrics_recomputes_from_earliest_of_existing_and_imported_dates() {
    let repository = TestCompletedWorkoutRepository::default();
    repository
        .upsert(sample_workout_missing_tss())
        .await
        .unwrap();
    let imports = RecordingImports::default();
    let recompute = Arc::new(RecordingRecomputeService::default());
    let mut shifted_activity = sample_detailed_activity();
    shifted_activity.start_date_local = "2026-04-14T15:28:24".to_string();
    let service = IntervalsCompletedWorkoutBackfillService::new(
        repository,
        TestSettings,
        TestApi {
            activity: shifted_activity,
            lookups: Arc::new(Mutex::new(Vec::new())),
        },
        imports.clone(),
        TestClock,
    )
    .with_training_load_recompute_service(recompute.clone());

    let result = service
        .backfill_missing_metrics("user-1", None, None)
        .await
        .unwrap();

    assert_eq!(result.recomputed_from.as_deref(), Some("2026-04-14"));
    assert_eq!(
        recompute.calls(),
        vec![(
            "user-1".to_string(),
            "2026-04-14".to_string(),
            1_775_174_400,
        )]
    );
    let ExternalImportCommand::UpsertCompletedWorkout(import) = &imports.commands()[0] else {
        panic!("expected completed workout import");
    };
    assert_eq!(import.workout.start_date_local, "2026-04-14T15:28:24");
}

#[tokio::test]
async fn backfill_missing_metrics_recomputes_existing_range_when_no_workouts_need_backfill() {
    let repository = TestCompletedWorkoutRepository::default();
    repository.upsert(sample_complete_workout()).await.unwrap();
    let recompute = Arc::new(RecordingRecomputeService::default());
    let service = IntervalsCompletedWorkoutBackfillService::new(
        repository,
        TestSettings,
        TestApi {
            activity: sample_detailed_activity(),
            lookups: Arc::new(Mutex::new(Vec::new())),
        },
        RecordingImports::default(),
        TestClock,
    )
    .with_training_load_recompute_service(recompute.clone());

    let result = service
        .backfill_missing_metrics("user-1", None, None)
        .await
        .unwrap();

    assert_eq!(result.scanned, 1);
    assert_eq!(result.enriched, 0);
    assert_eq!(result.skipped, 1);
    assert_eq!(result.failed, 0);
    assert_eq!(result.recomputed_from.as_deref(), Some("2026-04-16"));
    assert_eq!(
        recompute.calls(),
        vec![(
            "user-1".to_string(),
            "2026-04-16".to_string(),
            1_775_174_400,
        )]
    );
}

#[tokio::test]
async fn backfill_missing_metrics_rejects_partial_date_range() {
    let service = IntervalsCompletedWorkoutBackfillService::new(
        TestCompletedWorkoutRepository::default(),
        TestSettings,
        TestApi {
            activity: sample_detailed_activity(),
            lookups: Arc::new(Mutex::new(Vec::new())),
        },
        RecordingImports::default(),
        TestClock,
    );

    let error = service
        .backfill_missing_metrics("user-1", Some("2026-04-16"), None)
        .await
        .unwrap_err();

    assert!(
        matches!(error, crate::domain::completed_workouts::CompletedWorkoutError::Repository(message) if message.contains("requires both oldest and newest"))
    );
}

#[tokio::test]
async fn backfill_missing_metrics_skips_activity_without_metrics_to_fill() {
    let repository = TestCompletedWorkoutRepository::default();
    repository
        .upsert(sample_workout_missing_tss())
        .await
        .unwrap();
    let imports = RecordingImports::default();
    let recompute = Arc::new(RecordingRecomputeService::default());
    let service = IntervalsCompletedWorkoutBackfillService::new(
        repository,
        TestSettings,
        TestApi {
            activity: sample_metrics_unbackfillable_activity(),
            lookups: Arc::new(Mutex::new(Vec::new())),
        },
        imports.clone(),
        TestClock,
    )
    .with_training_load_recompute_service(recompute.clone());

    let result = service
        .backfill_missing_metrics("user-1", None, None)
        .await
        .unwrap();

    assert_eq!(result.scanned, 1);
    assert_eq!(result.enriched, 0);
    assert_eq!(result.skipped, 1);
    assert_eq!(result.failed, 0);
    assert_eq!(result.recomputed_from, None);
    assert!(imports.commands().is_empty());
    assert!(recompute.calls().is_empty());
}
