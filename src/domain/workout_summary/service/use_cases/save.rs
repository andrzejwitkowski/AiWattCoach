use tracing::{info, warn};

use super::*;

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

        let (recap_status, plan_status, messages) = if let Some(training_plan_service) =
            self.training_plan_service.clone()
        {
            let bg_user_id = user_id.to_string();
            let bg_workout_id = target.storage_workout_id.clone();
            let is_latest = is_latest_completed_activity;
            let save_completion_port = self.save_completion_port.clone();
            tokio::spawn(async move {
                info!(
                    user_id = %bg_user_id,
                    workout_id = %bg_workout_id,
                    saved_at_epoch_seconds = now,
                    is_latest,
                    "Starting background recap and training plan generation"
                );
                let recap_ok = training_plan_service
                    .generate_recap_for_saved_workout(&bg_user_id, &bg_workout_id, now)
                    .await;
                if let Err(ref error) = recap_ok {
                    warn!(
                        bg_user_id,
                        bg_workout_id,
                        saved_at_epoch_seconds = now,
                        error = %error,
                        "Background recap generation failed"
                    );
                }
                let plan_ok = if is_latest {
                    let result = training_plan_service
                        .generate_for_saved_workout(&bg_user_id, &bg_workout_id, now)
                        .await;
                    if let Err(ref error) = result {
                        warn!(
                            bg_user_id,
                            bg_workout_id,
                            saved_at_epoch_seconds = now,
                            error = %error,
                            "Background training plan generation failed"
                        );
                    }
                    Some(result)
                } else {
                    None
                };

                if let Some(port) = &save_completion_port {
                    let recap_status = if recap_ok.is_ok() {
                        SaveWorkflowStatus::Generated
                    } else {
                        SaveWorkflowStatus::Failed
                    };
                    let (plan_status, plan_msgs) = match plan_ok {
                        Some(Ok(_)) => (
                            SaveWorkflowStatus::Generated,
                            vec!["14-day schedule generated.".to_string()],
                        ),
                        Some(Err(_)) => (
                            SaveWorkflowStatus::Failed,
                            vec!["14-day schedule failed.".to_string()],
                        ),
                        None => (SaveWorkflowStatus::Skipped, Vec::new()),
                    };
                    let mut msgs = vec![if recap_ok.is_ok() {
                        "Workout recap generated.".to_string()
                    } else {
                        "Workout recap failed.".to_string()
                    }];
                    msgs.extend(plan_msgs);
                    port.on_completed(&bg_user_id, &bg_workout_id, recap_status, plan_status, msgs);
                }
            });
            let msgs = if is_latest_completed_activity {
                vec![
                    "Workout recap is being generated in the background.".to_string(),
                    "14-day schedule is being generated in the background.".to_string(),
                ]
            } else {
                vec!["Workout recap is being generated in the background.".to_string()]
            };
            (
                SaveWorkflowStatus::Processing,
                if is_latest_completed_activity {
                    SaveWorkflowStatus::Processing
                } else {
                    SaveWorkflowStatus::Skipped
                },
                msgs,
            )
        } else {
            let msgs = if is_latest_completed_activity {
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
            };
            (
                SaveWorkflowStatus::Skipped,
                SaveWorkflowStatus::Skipped,
                msgs,
            )
        };

        let summary = self
            .get_existing_summary(user_id, &target.storage_workout_id)
            .await?;
        Ok(self.present_save_summary_result(
            SaveSummaryResult {
                summary,
                workflow: SaveWorkflowResult {
                    recap_status,
                    plan_status,
                    messages,
                },
            },
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
