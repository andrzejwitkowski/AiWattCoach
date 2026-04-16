use aiwattcoach::domain::workout_summary::{
    WorkoutRecap, WorkoutSummaryError, WorkoutSummaryRepository, WorkoutSummaryService,
    WorkoutSummaryUseCases,
};
use std::sync::{Arc, Mutex};

use crate::shared::{
    existing_summary, existing_summary_with_finished_conversation, test_service,
    test_service_with_settings, test_service_with_training_plan,
    test_service_with_training_plan_and_latest_activity,
    test_service_with_training_plan_latest_activity_and_completed_target,
    InMemoryWorkoutSummaryRepository, PersistCheckingTrainingPlanService,
    RecordingCompletedWorkoutTargetService, RecordingLatestCompletedActivityService,
    RecordingTrainingPlanService, RefreshingTrainingPlanService, TestAvailabilitySettingsService,
};

#[derive(Clone, Default)]
struct RecordingMissingSettingsService {
    find_calls: Arc<Mutex<Vec<String>>>,
    get_calls: Arc<Mutex<Vec<String>>>,
}

impl RecordingMissingSettingsService {
    fn find_calls(&self) -> Vec<String> {
        self.find_calls.lock().unwrap().clone()
    }

    fn get_calls(&self) -> Vec<String> {
        self.get_calls.lock().unwrap().clone()
    }
}

impl aiwattcoach::domain::settings::UserSettingsUseCases for RecordingMissingSettingsService {
    fn find_settings(
        &self,
        user_id: &str,
    ) -> aiwattcoach::domain::settings::BoxFuture<
        Result<
            Option<aiwattcoach::domain::settings::UserSettings>,
            aiwattcoach::domain::settings::SettingsError,
        >,
    > {
        let find_calls = self.find_calls.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            find_calls.lock().unwrap().push(user_id);
            Ok(None)
        })
    }

    fn get_settings(
        &self,
        user_id: &str,
    ) -> aiwattcoach::domain::settings::BoxFuture<
        Result<
            aiwattcoach::domain::settings::UserSettings,
            aiwattcoach::domain::settings::SettingsError,
        >,
    > {
        let get_calls = self.get_calls.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            get_calls.lock().unwrap().push(user_id);
            Ok(aiwattcoach::domain::settings::UserSettings::new_defaults(
                "unexpected".to_string(),
                1,
            ))
        })
    }

    fn update_ai_agents(
        &self,
        _user_id: &str,
        _ai_agents: aiwattcoach::domain::settings::AiAgentsConfig,
    ) -> aiwattcoach::domain::settings::BoxFuture<
        Result<
            aiwattcoach::domain::settings::UserSettings,
            aiwattcoach::domain::settings::SettingsError,
        >,
    > {
        unreachable!()
    }

    fn update_intervals(
        &self,
        _user_id: &str,
        _intervals: aiwattcoach::domain::settings::IntervalsConfig,
    ) -> aiwattcoach::domain::settings::BoxFuture<
        Result<
            aiwattcoach::domain::settings::UserSettings,
            aiwattcoach::domain::settings::SettingsError,
        >,
    > {
        unreachable!()
    }

    fn update_options(
        &self,
        _user_id: &str,
        _options: aiwattcoach::domain::settings::AnalysisOptions,
    ) -> aiwattcoach::domain::settings::BoxFuture<
        Result<
            aiwattcoach::domain::settings::UserSettings,
            aiwattcoach::domain::settings::SettingsError,
        >,
    > {
        unreachable!()
    }

    fn update_availability(
        &self,
        _user_id: &str,
        _availability: aiwattcoach::domain::settings::AvailabilitySettings,
    ) -> aiwattcoach::domain::settings::BoxFuture<
        Result<
            aiwattcoach::domain::settings::UserSettings,
            aiwattcoach::domain::settings::SettingsError,
        >,
    > {
        unreachable!()
    }

    fn update_cycling(
        &self,
        _user_id: &str,
        _cycling: aiwattcoach::domain::settings::CyclingSettings,
    ) -> aiwattcoach::domain::settings::BoxFuture<
        Result<
            aiwattcoach::domain::settings::UserSettings,
            aiwattcoach::domain::settings::SettingsError,
        >,
    > {
        unreachable!()
    }
}

#[tokio::test]
async fn create_summary_is_idempotent_when_summary_already_exists() {
    let repository = InMemoryWorkoutSummaryRepository::with_summary(existing_summary());
    let service = test_service(repository.clone());

    let summary = service.create_summary("user-1", "workout-1").await.unwrap();

    assert_eq!(summary.id, "summary-1");
    assert_eq!(summary.workout_id, "workout-1");
    assert_eq!(summary.workout_recap_text, None);
    assert_eq!(summary.workout_recap_provider, None);
    assert_eq!(summary.workout_recap_model, None);
    assert_eq!(summary.workout_recap_generated_at_epoch_seconds, None);
    assert_eq!(repository.calls(), Vec::<String>::new());
}

#[tokio::test]
async fn create_summary_defaults_recap_fields_to_none() {
    let repository = InMemoryWorkoutSummaryRepository::default();
    let service = test_service(repository);

    let summary = service.create_summary("user-1", "workout-1").await.unwrap();

    assert_eq!(summary.workout_recap_text, None);
    assert_eq!(summary.workout_recap_provider, None);
    assert_eq!(summary.workout_recap_model, None);
    assert_eq!(summary.workout_recap_generated_at_epoch_seconds, None);
}

#[tokio::test]
async fn get_summary_returns_not_found_when_missing() {
    let service = test_service(InMemoryWorkoutSummaryRepository::default());

    let error = service
        .get_summary("user-1", "workout-1")
        .await
        .unwrap_err();

    assert_eq!(error, WorkoutSummaryError::NotFound);
}

#[tokio::test]
async fn update_rpe_rejects_values_outside_expected_range() {
    let repository = InMemoryWorkoutSummaryRepository::with_summary(existing_summary());
    let service = test_service(repository.clone());

    let error = service
        .update_rpe("user-1", "workout-1", 11)
        .await
        .unwrap_err();

    assert_eq!(
        error,
        WorkoutSummaryError::Validation("rpe must be between 1 and 10".to_string())
    );
    assert_eq!(repository.calls(), Vec::<String>::new());
}

#[tokio::test]
async fn mark_saved_returns_workflow_statuses_after_persisting_saved_state() {
    let repository = InMemoryWorkoutSummaryRepository::with_summary(existing_summary());
    let service = test_service(repository.clone());

    let result = service.mark_saved("user-1", "workout-1").await.unwrap();

    assert_eq!(result.summary.saved_at_epoch_seconds, Some(1_700_000_000));
    assert_eq!(result.workflow.recap_status.as_str(), "skipped");
    assert_eq!(result.workflow.plan_status.as_str(), "skipped");
    assert_eq!(
        repository.calls(),
        vec!["set_saved_state:workout-1:Some(1700000000)".to_string()]
    );
}

#[tokio::test]
async fn mark_saved_skips_recap_and_plan_without_finished_conversation() {
    let repository = InMemoryWorkoutSummaryRepository::with_summary(existing_summary());
    let training_plan = RecordingTrainingPlanService::default();
    let service = test_service_with_training_plan(
        repository.clone(),
        std::sync::Arc::new(training_plan.clone()),
    );

    let result = service.mark_saved("user-1", "workout-1").await.unwrap();

    assert_eq!(result.workflow.recap_status.as_str(), "skipped");
    assert_eq!(result.workflow.plan_status.as_str(), "skipped");
    assert_eq!(
        result.workflow.messages,
        vec!["No finished coach conversation to process.".to_string()]
    );
    assert_eq!(training_plan.calls(), Vec::<String>::new());
}

#[tokio::test]
async fn mark_saved_generates_recap_only_for_finished_conversation_on_non_latest_activity() {
    let mut summary = existing_summary_with_finished_conversation();
    summary.workout_id = "workout-older".to_string();
    let repository = InMemoryWorkoutSummaryRepository::with_summary(summary);
    let training_plan = RecordingTrainingPlanService::default();
    let latest_activity = RecordingLatestCompletedActivityService::new(Some("workout-latest"));
    let service = test_service_with_training_plan_and_latest_activity(
        repository,
        std::sync::Arc::new(training_plan.clone()),
        std::sync::Arc::new(latest_activity.clone()),
    );

    let result = service.mark_saved("user-1", "workout-older").await.unwrap();

    assert_eq!(result.workflow.recap_status.as_str(), "generated");
    assert_eq!(result.workflow.plan_status.as_str(), "skipped");
    assert_eq!(
        result.workflow.messages,
        vec![
            "Workout recap generated.".to_string(),
            "14-day schedule skipped because this is not the latest completed activity."
                .to_string(),
        ]
    );
    assert_eq!(
        training_plan.calls(),
        vec!["generate_recap_for_saved_workout:user-1:workout-older:1700000000".to_string()]
    );
    assert_eq!(
        latest_activity.calls(),
        vec!["latest_completed_activity_id:user-1".to_string()]
    );
}

#[tokio::test]
async fn mark_saved_generates_recap_and_plan_for_latest_completed_activity() {
    let repository = InMemoryWorkoutSummaryRepository::with_summary(
        existing_summary_with_finished_conversation(),
    );
    let training_plan = RecordingTrainingPlanService::default();
    training_plan.succeed_next(aiwattcoach::domain::training_plan::GeneratedTrainingPlan {
        snapshot: aiwattcoach::domain::training_plan::TrainingPlanSnapshot {
            user_id: "user-1".to_string(),
            workout_id: "workout-1".to_string(),
            operation_key: "training-plan:user-1:workout-1:1700000000".to_string(),
            saved_at_epoch_seconds: 1_700_000_000,
            start_date: "2026-04-06".to_string(),
            end_date: "2026-04-19".to_string(),
            days: Vec::new(),
            created_at_epoch_seconds: 1_700_000_000,
        },
        active_projected_days: Vec::new(),
        was_generated: true,
    });
    let latest_activity = RecordingLatestCompletedActivityService::new(Some("workout-1"));
    let service = test_service_with_training_plan_and_latest_activity(
        repository,
        std::sync::Arc::new(training_plan.clone()),
        std::sync::Arc::new(latest_activity),
    );

    let result = service.mark_saved("user-1", "workout-1").await.unwrap();

    assert_eq!(result.workflow.recap_status.as_str(), "generated");
    assert_eq!(result.workflow.plan_status.as_str(), "generated");
    assert_eq!(
        result.workflow.messages,
        vec![
            "Workout recap generated.".to_string(),
            "14-day schedule generated.".to_string(),
        ]
    );
    assert_eq!(
        training_plan.calls(),
        vec![
            "generate_recap_for_saved_workout:user-1:workout-1:1700000000".to_string(),
            "generate_for_saved_workout:user-1:workout-1:1700000000".to_string(),
        ]
    );
}

#[tokio::test]
async fn mark_saved_rejects_planned_workout_targets() {
    let repository = InMemoryWorkoutSummaryRepository::with_summary(
        existing_summary_with_finished_conversation(),
    );
    let training_plan = RecordingTrainingPlanService::default();
    let latest_activity = RecordingLatestCompletedActivityService::new(Some("workout-1"));
    let completed_target = RecordingCompletedWorkoutTargetService::allowing(&["activity-1"]);
    let service = test_service_with_training_plan_latest_activity_and_completed_target(
        repository.clone(),
        std::sync::Arc::new(training_plan.clone()),
        std::sync::Arc::new(latest_activity.clone()),
        std::sync::Arc::new(completed_target.clone()),
    );

    let error = service.mark_saved("user-1", "workout-1").await.unwrap_err();

    assert_eq!(
        error,
        WorkoutSummaryError::Validation(
            "workout summary is only available for completed workouts".to_string()
        )
    );
    assert!(repository.calls().is_empty());
    assert!(training_plan.calls().is_empty());
    assert!(latest_activity.calls().is_empty());
    assert_eq!(
        completed_target.calls(),
        vec!["is_completed_workout_target:user-1:workout-1".to_string()]
    );
}

#[tokio::test]
async fn list_summaries_ignores_non_completed_workout_targets() {
    let repository = InMemoryWorkoutSummaryRepository::with_summary(existing_summary());
    repository.overwrite_summary(aiwattcoach::domain::workout_summary::WorkoutSummary {
        workout_id: "activity-1".to_string(),
        ..existing_summary()
    });
    let completed_target = RecordingCompletedWorkoutTargetService::allowing(&["activity-1"]);
    let service = WorkoutSummaryService::new(
        repository,
        crate::shared::InMemoryCoachReplyOperationRepository::default(),
        crate::shared::TestClock,
        crate::shared::TestIdGenerator::default(),
    )
    .with_completed_workout_target_service(std::sync::Arc::new(completed_target.clone()));

    let summaries = service
        .list_summaries(
            "user-1",
            vec!["workout-1".to_string(), "activity-1".to_string()],
        )
        .await
        .unwrap();

    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].workout_id, "activity-1");
    assert_eq!(
        completed_target.calls(),
        vec![
            "is_completed_workout_target:user-1:workout-1".to_string(),
            "is_completed_workout_target:user-1:activity-1".to_string(),
        ]
    );
}

#[tokio::test]
async fn mark_saved_reports_failed_plan_generation_for_latest_completed_activity() {
    let repository = InMemoryWorkoutSummaryRepository::with_summary(
        existing_summary_with_finished_conversation(),
    );
    let training_plan = RecordingTrainingPlanService::default();
    training_plan.fail_next(
        aiwattcoach::domain::training_plan::TrainingPlanError::Unavailable(
            "llm temporarily unavailable".to_string(),
        ),
    );
    let latest_activity = RecordingLatestCompletedActivityService::new(Some("workout-1"));
    let service = test_service_with_training_plan_and_latest_activity(
        repository,
        std::sync::Arc::new(training_plan.clone()),
        std::sync::Arc::new(latest_activity),
    );

    let result = service.mark_saved("user-1", "workout-1").await.unwrap();

    assert_eq!(result.workflow.recap_status.as_str(), "generated");
    assert_eq!(result.workflow.plan_status.as_str(), "failed");
    assert_eq!(
        result.workflow.messages,
        vec![
            "Workout recap generated.".to_string(),
            "14-day schedule failed.".to_string(),
        ]
    );
    assert_eq!(
        training_plan.calls(),
        vec![
            "generate_recap_for_saved_workout:user-1:workout-1:1700000000".to_string(),
            "generate_for_saved_workout:user-1:workout-1:1700000000".to_string(),
        ]
    );
}

#[tokio::test]
async fn mark_saved_skips_generation_when_training_plan_service_is_not_configured() {
    let repository = InMemoryWorkoutSummaryRepository::with_summary(
        existing_summary_with_finished_conversation(),
    );
    let service = test_service(repository);

    let result = service.mark_saved("user-1", "workout-1").await.unwrap();

    assert_eq!(result.workflow.recap_status.as_str(), "skipped");
    assert_eq!(result.workflow.plan_status.as_str(), "skipped");
    assert_eq!(
        result.workflow.messages,
        vec![
            "Workout recap skipped.".to_string(),
            "14-day schedule skipped because this is not the latest completed activity."
                .to_string(),
        ]
    );
}

#[tokio::test]
async fn mark_saved_triggers_training_plan_generation_after_persisting_saved_state() {
    let repository = InMemoryWorkoutSummaryRepository::with_summary(
        existing_summary_with_finished_conversation(),
    );
    let training_plan = PersistCheckingTrainingPlanService::new(repository.clone());
    let latest_activity = RecordingLatestCompletedActivityService::new(Some("workout-1"));
    let service = test_service_with_training_plan_and_latest_activity(
        repository.clone(),
        std::sync::Arc::new(training_plan.clone()),
        std::sync::Arc::new(latest_activity),
    );

    let result = service.mark_saved("user-1", "workout-1").await.unwrap();

    assert_eq!(result.summary.saved_at_epoch_seconds, Some(1_700_000_000));
    assert_eq!(
        repository.calls(),
        vec!["set_saved_state:workout-1:Some(1700000000)".to_string()]
    );
    assert!(training_plan.observed_persisted_saved_at());
}

#[tokio::test]
async fn repeat_mark_saved_retries_training_plan_generation_for_already_saved_summary() {
    let mut summary = existing_summary_with_finished_conversation();
    summary.saved_at_epoch_seconds = Some(1_700_000_000);
    let repository = InMemoryWorkoutSummaryRepository::with_summary(summary);
    let training_plan = RecordingTrainingPlanService::default();
    let service = test_service_with_training_plan(
        repository.clone(),
        std::sync::Arc::new(training_plan.clone()),
    );

    let result = service.mark_saved("user-1", "workout-1").await.unwrap();

    assert_eq!(result.summary.saved_at_epoch_seconds, Some(1_700_000_000));
    assert_eq!(result.workflow.recap_status.as_str(), "unchanged");
    assert_eq!(result.workflow.plan_status.as_str(), "failed");
    assert_eq!(
        result.workflow.messages,
        vec!["14-day schedule failed on retry.".to_string()]
    );
    assert_eq!(repository.calls(), Vec::<String>::new());
    assert_eq!(
        training_plan.calls(),
        vec!["generate_for_saved_workout:user-1:workout-1:1700000000".to_string()]
    );
}

#[tokio::test]
async fn repeat_mark_saved_skips_retry_when_training_plan_service_is_not_configured() {
    let mut summary = existing_summary_with_finished_conversation();
    summary.saved_at_epoch_seconds = Some(1_700_000_000);
    let repository = InMemoryWorkoutSummaryRepository::with_summary(summary);
    let service = test_service(repository.clone());

    let result = service.mark_saved("user-1", "workout-1").await.unwrap();

    assert_eq!(result.summary.saved_at_epoch_seconds, Some(1_700_000_000));
    assert_eq!(result.workflow.recap_status.as_str(), "unchanged");
    assert_eq!(result.workflow.plan_status.as_str(), "skipped");
    assert_eq!(result.workflow.messages, Vec::<String>::new());
    assert_eq!(repository.calls(), Vec::<String>::new());
}

#[tokio::test]
async fn repeat_mark_saved_reports_generated_recap_when_retry_persists_recap_before_failure() {
    let mut summary = existing_summary_with_finished_conversation();
    summary.saved_at_epoch_seconds = Some(1_700_000_000);
    let repository = InMemoryWorkoutSummaryRepository::with_summary(summary);
    let training_plan = RefreshingTrainingPlanService::new_with_failure(
        repository.clone(),
        {
            let mut refreshed = existing_summary_with_finished_conversation();
            refreshed.saved_at_epoch_seconds = Some(1_700_000_000);
            refreshed.workout_recap_text = Some("Retry recap".to_string());
            refreshed.workout_recap_provider = Some("openrouter".to_string());
            refreshed.workout_recap_model = Some("google/gemini-3-flash-preview".to_string());
            refreshed.workout_recap_generated_at_epoch_seconds = Some(1_700_000_123);
            refreshed.updated_at_epoch_seconds = 1_700_000_123;
            refreshed
        },
        aiwattcoach::domain::training_plan::TrainingPlanError::Unavailable(
            "plan generation failed after recap persisted".to_string(),
        ),
    );
    let service = test_service_with_training_plan(
        repository.clone(),
        std::sync::Arc::new(training_plan.clone()),
    );

    let result = service.mark_saved("user-1", "workout-1").await.unwrap();

    assert_eq!(result.workflow.recap_status.as_str(), "generated");
    assert_eq!(result.workflow.plan_status.as_str(), "failed");
    assert_eq!(
        result.workflow.messages,
        vec![
            "Workout recap generated on retry.".to_string(),
            "14-day schedule failed on retry.".to_string(),
        ]
    );
    assert_eq!(
        result.summary.workout_recap_text.as_deref(),
        Some("Retry recap")
    );
}

#[tokio::test]
async fn repeat_mark_saved_keeps_recap_unchanged_when_retry_fails_after_existing_recap() {
    let mut summary = existing_summary_with_finished_conversation();
    summary.saved_at_epoch_seconds = Some(1_700_000_000);
    summary.workout_recap_text = Some("Existing recap".to_string());
    summary.workout_recap_provider = Some("openrouter".to_string());
    summary.workout_recap_model = Some("google/gemini-3-flash-preview".to_string());
    summary.workout_recap_generated_at_epoch_seconds = Some(1_700_000_010);
    let repository = InMemoryWorkoutSummaryRepository::with_summary(summary);
    let training_plan = RecordingTrainingPlanService::default();
    let service = test_service_with_training_plan(
        repository.clone(),
        std::sync::Arc::new(training_plan.clone()),
    );

    let result = service.mark_saved("user-1", "workout-1").await.unwrap();

    assert_eq!(result.workflow.recap_status.as_str(), "unchanged");
    assert_eq!(result.workflow.plan_status.as_str(), "failed");
    assert_eq!(
        result.workflow.messages,
        vec!["14-day schedule failed on retry.".to_string()]
    );
}

#[tokio::test]
async fn repeat_mark_saved_reloads_summary_after_successful_training_plan_retry() {
    let mut summary = existing_summary_with_finished_conversation();
    summary.saved_at_epoch_seconds = Some(1_700_000_000);
    let repository = InMemoryWorkoutSummaryRepository::with_summary(summary);
    let mut refreshed_summary = existing_summary_with_finished_conversation();
    refreshed_summary.saved_at_epoch_seconds = Some(1_700_000_000);
    refreshed_summary.workout_recap_text = Some("Refreshed recap after retry".to_string());
    refreshed_summary.updated_at_epoch_seconds = 1_700_000_111;
    let training_plan = RefreshingTrainingPlanService::new(repository.clone(), refreshed_summary);
    training_plan.succeed_next(aiwattcoach::domain::training_plan::GeneratedTrainingPlan {
        snapshot: aiwattcoach::domain::training_plan::TrainingPlanSnapshot {
            user_id: "user-1".to_string(),
            workout_id: "workout-1".to_string(),
            operation_key: "training-plan:user-1:workout-1:1700000000".to_string(),
            saved_at_epoch_seconds: 1_700_000_000,
            start_date: "2026-04-06".to_string(),
            end_date: "2026-04-19".to_string(),
            days: Vec::new(),
            created_at_epoch_seconds: 1_700_000_000,
        },
        active_projected_days: Vec::new(),
        was_generated: false,
    });
    let service = test_service_with_training_plan(
        repository.clone(),
        std::sync::Arc::new(training_plan.clone()),
    );

    let result = service.mark_saved("user-1", "workout-1").await.unwrap();

    assert_eq!(
        result.summary.workout_recap_text.as_deref(),
        Some("Refreshed recap after retry")
    );
    assert_eq!(result.summary.updated_at_epoch_seconds, 1_700_000_111);
    assert_eq!(result.workflow.recap_status.as_str(), "generated");
    assert_eq!(result.workflow.plan_status.as_str(), "unchanged");
    assert_eq!(
        result.workflow.messages,
        vec!["Workout recap generated on retry.".to_string()]
    );
    assert_eq!(repository.calls(), Vec::<String>::new());
    assert_eq!(
        training_plan.calls(),
        vec!["generate_for_saved_workout:user-1:workout-1:1700000000".to_string()]
    );
}

#[tokio::test]
async fn repeat_mark_saved_does_not_report_generated_recap_for_timestamp_only_retry_changes() {
    let mut summary = existing_summary_with_finished_conversation();
    summary.saved_at_epoch_seconds = Some(1_700_000_000);
    summary.workout_recap_text = Some("Existing recap".to_string());
    summary.workout_recap_provider = Some("openrouter".to_string());
    summary.workout_recap_model = Some("google/gemini-3-flash-preview".to_string());
    summary.workout_recap_generated_at_epoch_seconds = Some(1_700_000_010);
    let repository = InMemoryWorkoutSummaryRepository::with_summary(summary);
    let mut refreshed_summary = existing_summary_with_finished_conversation();
    refreshed_summary.saved_at_epoch_seconds = Some(1_700_000_000);
    refreshed_summary.workout_recap_text = Some("Existing recap".to_string());
    refreshed_summary.workout_recap_provider = Some("openrouter".to_string());
    refreshed_summary.workout_recap_model = Some("google/gemini-3-flash-preview".to_string());
    refreshed_summary.workout_recap_generated_at_epoch_seconds = Some(1_700_000_111);
    let training_plan = RefreshingTrainingPlanService::new(repository.clone(), refreshed_summary);
    training_plan.succeed_next(aiwattcoach::domain::training_plan::GeneratedTrainingPlan {
        snapshot: aiwattcoach::domain::training_plan::TrainingPlanSnapshot {
            user_id: "user-1".to_string(),
            workout_id: "workout-1".to_string(),
            operation_key: "training-plan:user-1:workout-1:1700000000".to_string(),
            saved_at_epoch_seconds: 1_700_000_000,
            start_date: "2026-04-06".to_string(),
            end_date: "2026-04-19".to_string(),
            days: Vec::new(),
            created_at_epoch_seconds: 1_700_000_000,
        },
        active_projected_days: Vec::new(),
        was_generated: false,
    });
    let service = test_service_with_training_plan(
        repository.clone(),
        std::sync::Arc::new(training_plan.clone()),
    );

    let result = service.mark_saved("user-1", "workout-1").await.unwrap();

    assert_eq!(result.workflow.recap_status.as_str(), "unchanged");
    assert_eq!(result.workflow.plan_status.as_str(), "unchanged");
    assert_eq!(result.workflow.messages, Vec::<String>::new());
}

#[tokio::test]
async fn mark_saved_maps_training_plan_failure_to_repository_error_after_persisting_save() {
    let repository = InMemoryWorkoutSummaryRepository::with_summary(
        existing_summary_with_finished_conversation(),
    );
    let training_plan = RecordingTrainingPlanService::default();
    training_plan.fail_next(
        aiwattcoach::domain::training_plan::TrainingPlanError::Unavailable(
            "llm temporarily unavailable".to_string(),
        ),
    );
    let service =
        test_service_with_training_plan(repository.clone(), std::sync::Arc::new(training_plan));

    let result = service.mark_saved("user-1", "workout-1").await.unwrap();

    assert_eq!(result.summary.saved_at_epoch_seconds, Some(1_700_000_000));
    assert_eq!(
        repository.calls(),
        vec!["set_saved_state:workout-1:Some(1700000000)".to_string()]
    );
    assert_eq!(
        repository
            .find_by_user_id_and_workout_id("user-1", "workout-1")
            .await
            .unwrap()
            .unwrap()
            .saved_at_epoch_seconds,
        Some(1_700_000_000)
    );
}

#[tokio::test]
async fn update_rpe_rejects_saved_summary() {
    let mut summary = existing_summary();
    summary.saved_at_epoch_seconds = Some(1_700_000_000);
    let repository = InMemoryWorkoutSummaryRepository::with_summary(summary);
    let service = test_service(repository.clone());

    let error = service
        .update_rpe("user-1", "workout-1", 8)
        .await
        .unwrap_err();

    assert_eq!(error, WorkoutSummaryError::Locked);
    assert_eq!(repository.calls(), Vec::<String>::new());
}

#[tokio::test]
async fn reopen_summary_clears_saved_state() {
    let mut summary = existing_summary();
    summary.saved_at_epoch_seconds = Some(1_700_000_000);
    let repository = InMemoryWorkoutSummaryRepository::with_summary(summary);
    let service = test_service(repository.clone());

    let summary = service.reopen_summary("user-1", "workout-1").await.unwrap();

    assert_eq!(summary.saved_at_epoch_seconds, None);
    assert_eq!(
        repository.calls(),
        vec!["set_saved_state:workout-1:None".to_string()]
    );
}

#[tokio::test]
async fn reopen_summary_is_a_no_op_when_already_editable() {
    let repository = InMemoryWorkoutSummaryRepository::with_summary(existing_summary());
    let service = test_service(repository.clone());

    let summary = service.reopen_summary("user-1", "workout-1").await.unwrap();

    assert_eq!(summary.saved_at_epoch_seconds, None);
    assert_eq!(repository.calls(), Vec::<String>::new());
}

#[tokio::test]
async fn persist_workout_recap_updates_recap_fields_and_timestamp() {
    let repository = InMemoryWorkoutSummaryRepository::with_summary(existing_summary());
    let service = test_service(repository.clone());

    let summary = service
        .persist_workout_recap(
            "user-1",
            "workout-1",
            WorkoutRecap::generated(
                "Strong finish after a rough middle block.",
                "openai",
                "gpt-5.4-mini",
                1_700_000_123,
            ),
        )
        .await
        .unwrap();

    assert_eq!(
        summary.workout_recap_text,
        Some("Strong finish after a rough middle block.".to_string())
    );
    assert_eq!(summary.workout_recap_provider, Some("openai".to_string()));
    assert_eq!(
        summary.workout_recap_model,
        Some("gpt-5.4-mini".to_string())
    );
    assert_eq!(
        summary.workout_recap_generated_at_epoch_seconds,
        Some(1_700_000_123)
    );
    assert_eq!(summary.updated_at_epoch_seconds, 1_700_000_000);
    assert_eq!(
        repository.calls(),
        vec!["persist_workout_recap:workout-1".to_string()]
    );
}

#[tokio::test]
async fn persist_workout_recap_is_idempotent_for_repeated_values() {
    let repository = InMemoryWorkoutSummaryRepository::with_summary(existing_summary());
    let service = test_service(repository.clone());

    service
        .persist_workout_recap(
            "user-1",
            "workout-1",
            WorkoutRecap::generated(
                "Legs came around late and cadence stayed smooth.",
                "openrouter",
                "google/gemini-3-flash-preview",
                1_700_000_321,
            ),
        )
        .await
        .unwrap();

    let summary = service
        .persist_workout_recap(
            "user-1",
            "workout-1",
            WorkoutRecap::generated(
                "Legs came around late and cadence stayed smooth.",
                "openrouter",
                "google/gemini-3-flash-preview",
                1_700_000_321,
            ),
        )
        .await
        .unwrap();

    assert_eq!(
        summary.workout_recap_text,
        Some("Legs came around late and cadence stayed smooth.".to_string())
    );
    assert_eq!(
        summary.workout_recap_provider,
        Some("openrouter".to_string())
    );
    assert_eq!(
        summary.workout_recap_model,
        Some("google/gemini-3-flash-preview".to_string())
    );
    assert_eq!(
        summary.workout_recap_generated_at_epoch_seconds,
        Some(1_700_000_321)
    );
    assert_eq!(summary.updated_at_epoch_seconds, 1_700_000_000);
    assert_eq!(
        repository.calls(),
        vec!["persist_workout_recap:workout-1".to_string()]
    );
}

#[tokio::test]
async fn persist_workout_recap_does_not_bump_updated_at_when_recap_is_already_stored() {
    let mut summary = existing_summary();
    summary.workout_recap_text = Some("Strong close after a controlled opener.".to_string());
    summary.workout_recap_provider = Some("openai".to_string());
    summary.workout_recap_model = Some("gpt-5.4-mini".to_string());
    summary.workout_recap_generated_at_epoch_seconds = Some(1_700_000_123);
    summary.updated_at_epoch_seconds = 1_699_999_999;

    let repository = InMemoryWorkoutSummaryRepository::with_summary(summary);
    let service = test_service(repository.clone());

    let summary = service
        .persist_workout_recap(
            "user-1",
            "workout-1",
            WorkoutRecap::generated(
                "Strong close after a controlled opener.",
                "openai",
                "gpt-5.4-mini",
                1_700_000_123,
            ),
        )
        .await
        .unwrap();

    assert_eq!(summary.updated_at_epoch_seconds, 1_699_999_999);
    assert_eq!(repository.calls(), Vec::<String>::new());
}

#[tokio::test]
async fn mark_saved_requires_rpe() {
    let mut summary = existing_summary();
    summary.rpe = None;
    let repository = InMemoryWorkoutSummaryRepository::with_summary(summary);
    let service = test_service(repository.clone());

    let error = service.mark_saved("user-1", "workout-1").await.unwrap_err();

    assert_eq!(
        error,
        WorkoutSummaryError::Validation(
            "rpe must be set before saving workout summary".to_string()
        )
    );
    assert_eq!(repository.calls(), Vec::<String>::new());
}

#[tokio::test]
async fn append_user_message_requires_configured_availability_before_chat() {
    let repository = InMemoryWorkoutSummaryRepository::with_summary(existing_summary());
    let service = test_service_with_settings(
        repository.clone(),
        TestAvailabilitySettingsService::unconfigured(),
    );

    let error = service
        .append_user_message("user-1", "workout-1", "Need feedback".to_string())
        .await
        .unwrap_err();

    assert_eq!(
        error,
        WorkoutSummaryError::Validation(
            "availability must be configured before chatting with coach".to_string()
        )
    );
    assert_eq!(repository.calls(), Vec::<String>::new());
}

#[tokio::test]
async fn append_user_message_checks_summary_before_missing_availability() {
    let repository = InMemoryWorkoutSummaryRepository::default();
    let settings_service = Arc::new(RecordingMissingSettingsService::default());
    let service = test_service_with_settings(repository, settings_service.clone());

    let error = service
        .append_user_message("user-1", "workout-1", "Need feedback".to_string())
        .await
        .unwrap_err();

    assert_eq!(error, WorkoutSummaryError::NotFound);
    assert!(settings_service.find_calls().is_empty());
    assert!(settings_service.get_calls().is_empty());
}

#[tokio::test]
async fn append_user_message_uses_find_settings_without_creating_defaults() {
    let repository = InMemoryWorkoutSummaryRepository::with_summary(existing_summary());
    let settings_service = Arc::new(RecordingMissingSettingsService::default());
    let service = test_service_with_settings(repository.clone(), settings_service.clone());

    let error = service
        .append_user_message("user-1", "workout-1", "Need feedback".to_string())
        .await
        .unwrap_err();

    assert_eq!(
        error,
        WorkoutSummaryError::Validation(
            "availability must be configured before chatting with coach".to_string()
        )
    );
    assert_eq!(settings_service.find_calls(), vec!["user-1".to_string()]);
    assert!(settings_service.get_calls().is_empty());
    assert_eq!(repository.calls(), Vec::<String>::new());
}

#[tokio::test]
async fn append_user_message_allows_chat_when_availability_is_configured() {
    let repository = InMemoryWorkoutSummaryRepository::with_summary(existing_summary());
    let service = test_service_with_settings(
        repository.clone(),
        TestAvailabilitySettingsService::configured(),
    );

    let persisted = service
        .append_user_message("user-1", "workout-1", "Need feedback".to_string())
        .await
        .unwrap();

    assert_eq!(
        persisted.user_message.role,
        aiwattcoach::domain::workout_summary::MessageRole::User
    );
    assert_eq!(
        repository.calls(),
        vec!["append_message:workout-1:user".to_string()]
    );
}
