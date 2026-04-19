use std::sync::{Arc, Mutex};

use crate::domain::{
    completed_workouts::{CompletedWorkoutAdminUseCases, CompletedWorkoutRepository},
    external_sync::ExternalImportCommand,
};

use super::super::IntervalsCompletedWorkoutBackfillService;
use super::support::{
    sample_complete_workout, sample_detailed_activity, sample_sparse_workout,
    sample_unbackfillable_activity, RecordingImports, TestApi, TestClock,
    TestCompletedWorkoutRepository, TestSettings,
};

#[tokio::test]
async fn backfill_missing_details_imports_enriched_intervals_activity() {
    let repository = TestCompletedWorkoutRepository::default();
    repository.upsert(sample_sparse_workout()).await.unwrap();
    let lookups = Arc::new(Mutex::new(Vec::new()));
    let api = TestApi {
        activity: sample_detailed_activity(),
        lookups: lookups.clone(),
    };
    let imports = RecordingImports::default();
    let service = IntervalsCompletedWorkoutBackfillService::new(
        repository,
        TestSettings,
        api,
        imports.clone(),
        TestClock,
    );

    let result = service
        .backfill_missing_details("user-1", "2026-04-16", "2026-04-16")
        .await
        .unwrap();

    assert_eq!(result.scanned, 1);
    assert_eq!(result.enriched, 1);
    assert_eq!(result.skipped, 0);
    assert_eq!(result.failed, 0);
    assert_eq!(lookups.lock().unwrap().as_slice(), &["i1".to_string()]);

    let commands = imports.commands();
    let ExternalImportCommand::UpsertCompletedWorkout(import) = &commands[0] else {
        panic!("expected completed workout import");
    };
    assert!(!import.workout.details.streams.is_empty());
}

#[tokio::test]
async fn backfill_missing_details_reports_full_scanned_range() {
    let repository = TestCompletedWorkoutRepository::default();
    repository.upsert(sample_sparse_workout()).await.unwrap();
    repository.upsert(sample_complete_workout()).await.unwrap();
    let imports = RecordingImports::default();
    let service = IntervalsCompletedWorkoutBackfillService::new(
        repository,
        TestSettings,
        TestApi {
            activity: sample_detailed_activity(),
            lookups: Arc::new(Mutex::new(Vec::new())),
        },
        imports,
        TestClock,
    );

    let result = service
        .backfill_missing_details("user-1", "2026-04-16", "2026-04-16")
        .await
        .unwrap();

    assert_eq!(result.scanned, 2);
    assert_eq!(result.enriched, 1);
    assert_eq!(result.skipped, 1);
    assert_eq!(result.failed, 0);
}

#[tokio::test]
async fn backfill_missing_details_skips_activity_without_backfillable_details() {
    let repository = TestCompletedWorkoutRepository::default();
    repository.upsert(sample_sparse_workout()).await.unwrap();
    let lookups = Arc::new(Mutex::new(Vec::new()));
    let imports = RecordingImports::default();
    let service = IntervalsCompletedWorkoutBackfillService::new(
        repository,
        TestSettings,
        TestApi {
            activity: sample_unbackfillable_activity(),
            lookups: lookups.clone(),
        },
        imports.clone(),
        TestClock,
    );

    let result = service
        .backfill_missing_details("user-1", "2026-04-16", "2026-04-16")
        .await
        .unwrap();

    assert_eq!(result.scanned, 1);
    assert_eq!(result.enriched, 0);
    assert_eq!(result.skipped, 1);
    assert_eq!(result.failed, 0);
    assert!(imports.commands().is_empty());
    assert_eq!(lookups.lock().unwrap().as_slice(), &["i1".to_string()]);
}
