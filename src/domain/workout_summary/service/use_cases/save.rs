use std::sync::{Arc, OnceLock};

use tokio::sync::Semaphore;
use tracing::{info, warn};

use super::*;

const BACKGROUND_SAVE_WORKFLOW_CONCURRENCY_LIMIT: usize = 2;

fn background_save_workflow_semaphore() -> Arc<Semaphore> {
    static SEMAPHORE: OnceLock<Arc<Semaphore>> = OnceLock::new();
    SEMAPHORE
        .get_or_init(|| Arc::new(Semaphore::new(BACKGROUND_SAVE_WORKFLOW_CONCURRENCY_LIMIT)))
        .clone()
}

#[derive(Clone, PartialEq, Eq)]
struct RecapSnapshot {
    text: Option<String>,
    provider: Option<String>,
    model: Option<String>,
}

impl RecapSnapshot {
    fn from_summary(summary: &WorkoutSummary) -> Self {
        Self {
            text: summary.workout_recap_text.clone(),
            provider: summary.workout_recap_provider.clone(),
            model: summary.workout_recap_model.clone(),
        }
    }
}

fn has_finished_conversation(summary: &WorkoutSummary) -> bool {
    summary
        .messages
        .last()
        .is_some_and(|message| message.role == MessageRole::Coach)
}

struct BackgroundSaveWorkflow {
    training_plan_service: Arc<dyn TrainingPlanUseCases>,
    save_completion_port: Option<Arc<dyn SaveWorkflowCompletionPort>>,
    concurrency: Arc<Semaphore>,
    user_id: String,
    storage_workout_id: String,
    completion_workout_id: String,
    saved_at_epoch_seconds: i64,
}

fn processing_workflow_result() -> SaveWorkflowResult {
    SaveWorkflowResult {
        recap_status: SaveWorkflowStatus::Processing,
        plan_status: SaveWorkflowStatus::Processing,
        messages: vec![
            "Workout recap is being generated in the background.".to_string(),
            "14-day schedule is being generated in the background.".to_string(),
        ],
    }
}

fn skipped_generation_workflow_result() -> SaveWorkflowResult {
    SaveWorkflowResult {
        recap_status: SaveWorkflowStatus::Skipped,
        plan_status: SaveWorkflowStatus::Skipped,
        messages: vec![
            "Workout recap skipped.".to_string(),
            "14-day schedule skipped.".to_string(),
        ],
    }
}

fn outcome_status(ok: bool) -> SaveWorkflowStatus {
    if ok {
        SaveWorkflowStatus::Generated
    } else {
        SaveWorkflowStatus::Failed
    }
}

fn completion_workflow(
    recap_ok: bool,
    plan_ok: bool,
) -> (SaveWorkflowStatus, SaveWorkflowStatus, Vec<String>) {
    (
        outcome_status(recap_ok),
        outcome_status(plan_ok),
        vec![
            if recap_ok {
                "Workout recap generated.".to_string()
            } else {
                "Workout recap failed.".to_string()
            },
            if plan_ok {
                "14-day schedule generated.".to_string()
            } else {
                "14-day schedule failed.".to_string()
            },
        ],
    )
}

async fn run_background_save_workflow(workflow: BackgroundSaveWorkflow) {
    let Ok(_permit) = workflow.concurrency.acquire_owned().await else {
        warn!(
            user_id = %workflow.user_id,
            workout_id = %workflow.storage_workout_id,
            "Background save workflow limiter closed before generation started"
        );
        return;
    };

    info!(
        user_id = %workflow.user_id,
        workout_id = %workflow.storage_workout_id,
        saved_at_epoch_seconds = workflow.saved_at_epoch_seconds,
        "Starting background recap and training plan generation"
    );

    if let Some(port) = &workflow.save_completion_port {
        port.bind_progress_alias(
            &workflow.user_id,
            &workflow.storage_workout_id,
            &workflow.completion_workout_id,
        );
    }

    let recap_ok = workflow
        .training_plan_service
        .generate_recap_for_saved_workout(
            &workflow.user_id,
            &workflow.storage_workout_id,
            workflow.saved_at_epoch_seconds,
        )
        .await;
    if let Err(ref error) = recap_ok {
        warn!(
            user_id = %workflow.user_id,
            workout_id = %workflow.storage_workout_id,
            saved_at_epoch_seconds = workflow.saved_at_epoch_seconds,
            error = %error,
            "Background recap generation failed"
        );
    }

    let plan_result = workflow
        .training_plan_service
        .generate_for_saved_workout(
            &workflow.user_id,
            &workflow.storage_workout_id,
            workflow.saved_at_epoch_seconds,
        )
        .await;
    if let Err(ref error) = plan_result {
        warn!(
            user_id = %workflow.user_id,
            workout_id = %workflow.storage_workout_id,
            saved_at_epoch_seconds = workflow.saved_at_epoch_seconds,
            error = %error,
            "Background training plan generation failed"
        );
    }

    if let Some(port) = workflow.save_completion_port {
        let (recap_status, plan_status, mut messages) =
            completion_workflow(recap_ok.is_ok(), plan_result.is_ok());
        if let Ok(generated) = &plan_result {
            messages.extend(generated.quality_progress_messages.iter().cloned());
        }
        port.on_completed(
            &workflow.user_id,
            &workflow.completion_workout_id,
            recap_status,
            plan_status,
            messages,
        );
        port.clear_progress_alias(&workflow.user_id, &workflow.storage_workout_id);
    }
}

fn retry_workflow_messages(
    recap_status: SaveWorkflowStatus,
    plan_status: SaveWorkflowStatus,
) -> Vec<String> {
    let mut messages = Vec::new();
    if recap_status == SaveWorkflowStatus::Generated {
        messages.push("Workout recap generated on retry.".to_string());
    }
    match plan_status {
        SaveWorkflowStatus::Generated => {
            messages.push("14-day schedule generated on retry.".to_string());
        }
        SaveWorkflowStatus::Failed => {
            messages.push("14-day schedule failed on retry.".to_string());
        }
        _ => {}
    }
    messages
}

fn unchanged_skipped_workflow() -> SaveWorkflowResult {
    SaveWorkflowResult {
        recap_status: SaveWorkflowStatus::Unchanged,
        plan_status: SaveWorkflowStatus::Skipped,
        messages: Vec::new(),
    }
}

impl<Repo, Ops, Time, Ids> WorkoutSummaryService<Repo, Ops, Time, Ids>
where
    Repo: WorkoutSummaryRepository + Clone,
    Ops: CoachReplyOperationRepository + Clone,
    Time: Clock + Clone,
    Ids: IdGenerator + Clone,
{
    fn present_unchanged_skipped(
        &self,
        summary: WorkoutSummary,
        requested_workout_id: &str,
    ) -> SaveSummaryResult {
        self.present_save_summary_result(
            SaveSummaryResult {
                summary,
                workflow: unchanged_skipped_workflow(),
            },
            requested_workout_id,
        )
    }

    pub(super) async fn mark_saved_impl(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> Result<SaveSummaryResult, WorkoutSummaryError> {
        let target = self
            .resolve_workout_summary_target(user_id, workout_id, None)
            .await?;
        let existing = target
            .existing_summary
            .clone()
            .ok_or(WorkoutSummaryError::NotFound)?;
        if existing.saved_at_epoch_seconds.is_some() {
            return self.retry_saved_workflow(user_id, &target, existing).await;
        }
        if existing.rpe.is_none() {
            return Err(WorkoutSummaryError::Validation(
                "rpe must be set before saving workout summary".to_string(),
            ));
        }

        let now = self.clock.now_epoch_seconds();
        self.repository
            .set_saved_state(user_id, &target.storage_workout_id, Some(now), now)
            .await?;

        if !has_finished_conversation(&existing) {
            let summary = self
                .get_existing_summary(user_id, &target.storage_workout_id)
                .await?;
            return Ok(self.present_save_summary_result(
                SaveSummaryResult {
                    summary,
                    workflow: SaveWorkflowResult {
                        recap_status: SaveWorkflowStatus::Skipped,
                        plan_status: SaveWorkflowStatus::Skipped,
                        messages: vec!["No finished coach conversation to process.".to_string()],
                    },
                },
                &target.requested_workout_id,
            ));
        }

        let workflow = if let Some(training_plan_service) = self.training_plan_service.clone() {
            tokio::spawn(run_background_save_workflow(BackgroundSaveWorkflow {
                training_plan_service,
                save_completion_port: self.save_completion_port.clone(),
                concurrency: background_save_workflow_semaphore(),
                user_id: user_id.to_string(),
                storage_workout_id: target.storage_workout_id.clone(),
                completion_workout_id: target.requested_workout_id.clone(),
                saved_at_epoch_seconds: now,
            }));
            processing_workflow_result()
        } else {
            skipped_generation_workflow_result()
        };

        let summary = self
            .get_existing_summary(user_id, &target.storage_workout_id)
            .await?;
        Ok(self.present_save_summary_result(
            SaveSummaryResult { summary, workflow },
            &target.requested_workout_id,
        ))
    }

    async fn retry_saved_workflow(
        &self,
        user_id: &str,
        target: &ResolvedWorkoutSummaryTarget,
        existing: WorkoutSummary,
    ) -> Result<SaveSummaryResult, WorkoutSummaryError> {
        if !has_finished_conversation(&existing) {
            return Ok(self.present_unchanged_skipped(existing, &target.requested_workout_id));
        }

        let recap_before_retry = RecapSnapshot::from_summary(&existing);

        let (Some(training_plan_service), Some(saved_at_epoch_seconds)) =
            (&self.training_plan_service, existing.saved_at_epoch_seconds)
        else {
            return Ok(self.present_unchanged_skipped(existing, &target.requested_workout_id));
        };

        let plan_result = training_plan_service
            .generate_for_saved_workout(user_id, &target.storage_workout_id, saved_at_epoch_seconds)
            .await;
        if let Err(ref error) = plan_result {
            warn!(
                user_id,
                workout_id = %target.storage_workout_id,
                saved_at_epoch_seconds,
                error = %error,
                "Saved workout summary remains persisted after training plan generation retry failure"
            );
        }

        let summary = self
            .get_existing_summary(user_id, &target.storage_workout_id)
            .await?;
        let recap_status = if RecapSnapshot::from_summary(&summary) != recap_before_retry {
            SaveWorkflowStatus::Generated
        } else {
            SaveWorkflowStatus::Unchanged
        };
        let plan_status = match &plan_result {
            Ok(generated_plan) if generated_plan.was_generated => SaveWorkflowStatus::Generated,
            Ok(_) => SaveWorkflowStatus::Unchanged,
            Err(_) => SaveWorkflowStatus::Failed,
        };

        let mut messages = retry_workflow_messages(recap_status, plan_status);
        if let Ok(generated_plan) = &plan_result {
            messages.extend(generated_plan.quality_progress_messages.iter().cloned());
        }

        Ok(self.present_save_summary_result(
            SaveSummaryResult {
                summary,
                workflow: SaveWorkflowResult {
                    messages,
                    recap_status,
                    plan_status,
                },
            },
            &target.requested_workout_id,
        ))
    }
}
