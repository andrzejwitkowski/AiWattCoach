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

fn status_message(
    status: &SaveWorkflowStatus,
    generated: &str,
    failed: &str,
    skipped: &str,
) -> String {
    match status {
        SaveWorkflowStatus::Generated => generated.to_string(),
        SaveWorkflowStatus::Failed => failed.to_string(),
        _ => skipped.to_string(),
    }
}

fn matches_latest_completed_activity_id(latest_activity_id: &str, workout_id: &str) -> bool {
    latest_activity_id == workout_id
        || latest_activity_id
            == crate::domain::completed_workouts::completed_workout_activity_id(workout_id)
}

struct BackgroundSaveWorkflow {
    training_plan_service: Arc<dyn TrainingPlanUseCases>,
    save_completion_port: Option<Arc<dyn SaveWorkflowCompletionPort>>,
    concurrency: Arc<Semaphore>,
    user_id: String,
    storage_workout_id: String,
    completion_workout_id: String,
    saved_at_epoch_seconds: i64,
    is_latest_completed_activity: bool,
}

fn processing_workflow_result(is_latest_completed_activity: bool) -> SaveWorkflowResult {
    SaveWorkflowResult {
        recap_status: SaveWorkflowStatus::Processing,
        plan_status: if is_latest_completed_activity {
            SaveWorkflowStatus::Processing
        } else {
            SaveWorkflowStatus::Skipped
        },
        messages: processing_messages(is_latest_completed_activity),
    }
}

fn skipped_generation_workflow_result(is_latest_completed_activity: bool) -> SaveWorkflowResult {
    SaveWorkflowResult {
        recap_status: SaveWorkflowStatus::Skipped,
        plan_status: SaveWorkflowStatus::Skipped,
        messages: skipped_generation_messages(is_latest_completed_activity),
    }
}

fn processing_messages(is_latest_completed_activity: bool) -> Vec<String> {
    if is_latest_completed_activity {
        vec![
            "Workout recap is being generated in the background.".to_string(),
            "14-day schedule is being generated in the background.".to_string(),
        ]
    } else {
        vec!["Workout recap is being generated in the background.".to_string()]
    }
}

fn skipped_generation_messages(is_latest_completed_activity: bool) -> Vec<String> {
    if is_latest_completed_activity {
        vec![
            status_message(
                &SaveWorkflowStatus::Skipped,
                "Workout recap generated.",
                "Workout recap failed.",
                "Workout recap skipped.",
            ),
            status_message(
                &SaveWorkflowStatus::Skipped,
                "14-day schedule generated.",
                "14-day schedule failed.",
                "14-day schedule skipped.",
            ),
        ]
    } else {
        vec![
            status_message(
                &SaveWorkflowStatus::Skipped,
                "Workout recap generated.",
                "Workout recap failed.",
                "Workout recap skipped.",
            ),
            "14-day schedule skipped because this is not the latest completed activity."
                .to_string(),
        ]
    }
}

fn completion_workflow(
    recap_ok: bool,
    plan_ok: Option<bool>,
) -> (SaveWorkflowStatus, SaveWorkflowStatus, Vec<String>) {
    let recap_status = if recap_ok {
        SaveWorkflowStatus::Generated
    } else {
        SaveWorkflowStatus::Failed
    };
    let plan_status = match plan_ok {
        Some(true) => SaveWorkflowStatus::Generated,
        Some(false) => SaveWorkflowStatus::Failed,
        None => SaveWorkflowStatus::Skipped,
    };
    let mut messages = vec![if recap_ok {
        "Workout recap generated.".to_string()
    } else {
        "Workout recap failed.".to_string()
    }];
    match plan_ok {
        Some(true) => messages.push("14-day schedule generated.".to_string()),
        Some(false) => messages.push("14-day schedule failed.".to_string()),
        None => {}
    }
    (recap_status, plan_status, messages)
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
        is_latest = workflow.is_latest_completed_activity,
        "Starting background recap and training plan generation"
    );

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

    let plan_ok = if workflow.is_latest_completed_activity {
        let result = workflow
            .training_plan_service
            .generate_for_saved_workout(
                &workflow.user_id,
                &workflow.storage_workout_id,
                workflow.saved_at_epoch_seconds,
            )
            .await;
        if let Err(ref error) = result {
            warn!(
                user_id = %workflow.user_id,
                workout_id = %workflow.storage_workout_id,
                saved_at_epoch_seconds = workflow.saved_at_epoch_seconds,
                error = %error,
                "Background training plan generation failed"
            );
        }
        Some(result.is_ok())
    } else {
        None
    };

    if let Some(port) = workflow.save_completion_port {
        let (recap_status, plan_status, messages) = completion_workflow(recap_ok.is_ok(), plan_ok);
        port.on_completed(
            &workflow.user_id,
            &workflow.completion_workout_id,
            recap_status,
            plan_status,
            messages,
        );
    }
}

impl<Repo, Ops, Time, Ids> WorkoutSummaryService<Repo, Ops, Time, Ids>
where
    Repo: WorkoutSummaryRepository + Clone,
    Ops: CoachReplyOperationRepository + Clone,
    Time: Clock + Clone,
    Ids: IdGenerator + Clone,
{
    async fn is_latest_completed_activity(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> Result<bool, WorkoutSummaryError> {
        let Some(latest_completed_activity_service) = &self.latest_completed_activity_service
        else {
            return Ok(false);
        };

        Ok(latest_completed_activity_service
            .latest_completed_activity_id(user_id)
            .await?
            .as_deref()
            .is_some_and(|latest_activity_id| {
                matches_latest_completed_activity_id(latest_activity_id, workout_id)
            }))
    }

    pub(super) async fn mark_saved_impl(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> Result<SaveSummaryResult, WorkoutSummaryError> {
        let target = self
            .resolve_workout_summary_target(user_id, workout_id)
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

        let is_latest_completed_activity = self
            .is_latest_completed_activity(user_id, &target.preferred_workout_id)
            .await?;

        let workflow = if let Some(training_plan_service) = self.training_plan_service.clone() {
            tokio::spawn(run_background_save_workflow(BackgroundSaveWorkflow {
                training_plan_service,
                save_completion_port: self.save_completion_port.clone(),
                concurrency: background_save_workflow_semaphore(),
                user_id: user_id.to_string(),
                storage_workout_id: target.storage_workout_id.clone(),
                completion_workout_id: target.requested_workout_id.clone(),
                saved_at_epoch_seconds: now,
                is_latest_completed_activity,
            }));
            processing_workflow_result(is_latest_completed_activity)
        } else {
            skipped_generation_workflow_result(is_latest_completed_activity)
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
            return Ok(self.present_save_summary_result(
                SaveSummaryResult {
                    summary: existing,
                    workflow: SaveWorkflowResult {
                        recap_status: SaveWorkflowStatus::Unchanged,
                        plan_status: SaveWorkflowStatus::Skipped,
                        messages: Vec::new(),
                    },
                },
                &target.requested_workout_id,
            ));
        }

        let is_latest_completed_activity = self
            .is_latest_completed_activity(user_id, &target.preferred_workout_id)
            .await?;
        if !is_latest_completed_activity {
            return Ok(self.present_save_summary_result(
                SaveSummaryResult {
                    summary: existing,
                    workflow: SaveWorkflowResult {
                        recap_status: SaveWorkflowStatus::Unchanged,
                        plan_status: SaveWorkflowStatus::Skipped,
                        messages: Vec::new(),
                    },
                },
                &target.requested_workout_id,
            ));
        }

        let recap_before_retry = RecapSnapshot::from_summary(&existing);

        if let (Some(training_plan_service), Some(saved_at_epoch_seconds)) =
            (&self.training_plan_service, existing.saved_at_epoch_seconds)
        {
            match training_plan_service
                .generate_for_saved_workout(
                    user_id,
                    &target.storage_workout_id,
                    saved_at_epoch_seconds,
                )
                .await
            {
                Ok(generated_plan) => {
                    let summary = self
                        .get_existing_summary(user_id, &target.storage_workout_id)
                        .await?;
                    let recap_status =
                        if RecapSnapshot::from_summary(&summary) != recap_before_retry {
                            SaveWorkflowStatus::Generated
                        } else {
                            SaveWorkflowStatus::Unchanged
                        };
                    return Ok(self.present_save_summary_result(
                        SaveSummaryResult {
                            summary,
                            workflow: SaveWorkflowResult {
                                recap_status: recap_status.clone(),
                                plan_status: if generated_plan.was_generated {
                                    SaveWorkflowStatus::Generated
                                } else {
                                    SaveWorkflowStatus::Unchanged
                                },
                                messages: match (recap_status, generated_plan.was_generated) {
                                    (SaveWorkflowStatus::Generated, true) => vec![
                                        "Workout recap generated on retry.".to_string(),
                                        "14-day schedule generated on retry.".to_string(),
                                    ],
                                    (SaveWorkflowStatus::Generated, false) => {
                                        vec!["Workout recap generated on retry.".to_string()]
                                    }
                                    (_, true) => {
                                        vec!["14-day schedule generated on retry.".to_string()]
                                    }
                                    _ => Vec::new(),
                                },
                            },
                        },
                        &target.requested_workout_id,
                    ));
                }
                Err(error) => {
                    warn!(
                        user_id,
                        workout_id = %target.storage_workout_id,
                        saved_at_epoch_seconds,
                        error = %error,
                        "Saved workout summary remains persisted after training plan generation retry failure"
                    );

                    let summary = self
                        .get_existing_summary(user_id, &target.storage_workout_id)
                        .await?;
                    let recap_status =
                        if RecapSnapshot::from_summary(&summary) != recap_before_retry {
                            SaveWorkflowStatus::Generated
                        } else {
                            SaveWorkflowStatus::Unchanged
                        };
                    return Ok(self.present_save_summary_result(
                        SaveSummaryResult {
                            summary,
                            workflow: SaveWorkflowResult {
                                recap_status: recap_status.clone(),
                                plan_status: SaveWorkflowStatus::Failed,
                                messages: if recap_status == SaveWorkflowStatus::Generated {
                                    vec![
                                        "Workout recap generated on retry.".to_string(),
                                        "14-day schedule failed on retry.".to_string(),
                                    ]
                                } else {
                                    vec!["14-day schedule failed on retry.".to_string()]
                                },
                            },
                        },
                        &target.requested_workout_id,
                    ));
                }
            }
        }
        Ok(self.present_save_summary_result(
            SaveSummaryResult {
                summary: existing,
                workflow: SaveWorkflowResult {
                    recap_status: SaveWorkflowStatus::Unchanged,
                    plan_status: SaveWorkflowStatus::Skipped,
                    messages: Vec::new(),
                },
            },
            &target.requested_workout_id,
        ))
    }
}
