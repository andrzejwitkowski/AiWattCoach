use tracing::warn;

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
            == Some(workout_id))
    }

    pub(super) async fn mark_saved_impl(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> Result<SaveSummaryResult, WorkoutSummaryError> {
        self.validate_completed_workout_target(user_id, workout_id)
            .await?;

        let existing = self.get_existing_summary(user_id, workout_id).await?;
        if existing.saved_at_epoch_seconds.is_some() {
            return self
                .retry_saved_workflow(user_id, workout_id, existing)
                .await;
        }
        if existing.rpe.is_none() {
            return Err(WorkoutSummaryError::Validation(
                "rpe must be set before saving workout summary".to_string(),
            ));
        }

        let now = self.clock.now_epoch_seconds();
        self.repository
            .set_saved_state(user_id, workout_id, Some(now), now)
            .await?;

        if !has_finished_conversation(&existing) {
            let summary = self.get_existing_summary(user_id, workout_id).await?;
            return Ok(SaveSummaryResult {
                summary,
                workflow: SaveWorkflowResult {
                    recap_status: SaveWorkflowStatus::Skipped,
                    plan_status: SaveWorkflowStatus::Skipped,
                    messages: vec!["No finished coach conversation to process.".to_string()],
                },
            });
        }

        let is_latest_completed_activity = self
            .is_latest_completed_activity(user_id, workout_id)
            .await?;

        let recap_status = if let Some(training_plan_service) = &self.training_plan_service {
            match training_plan_service
                .generate_recap_for_saved_workout(user_id, workout_id, now)
                .await
            {
                Ok(_) => SaveWorkflowStatus::Generated,
                Err(error) => {
                    warn!(
                        user_id,
                        workout_id,
                        saved_at_epoch_seconds = now,
                        error = %error,
                        "Saved workout summary remains persisted after recap generation failure"
                    );
                    SaveWorkflowStatus::Failed
                }
            }
        } else {
            SaveWorkflowStatus::Skipped
        };

        let plan_status = if is_latest_completed_activity {
            if let Some(training_plan_service) = &self.training_plan_service {
                match training_plan_service
                    .generate_for_saved_workout(user_id, workout_id, now)
                    .await
                {
                    Ok(_) => SaveWorkflowStatus::Generated,
                    Err(error) => {
                        warn!(
                            user_id,
                            workout_id,
                            saved_at_epoch_seconds = now,
                            error = %error,
                            "Saved workout summary remains persisted after training plan generation failure"
                        );
                        SaveWorkflowStatus::Failed
                    }
                }
            } else {
                SaveWorkflowStatus::Skipped
            }
        } else {
            SaveWorkflowStatus::Skipped
        };

        let summary = self.get_existing_summary(user_id, workout_id).await?;
        Ok(SaveSummaryResult {
            summary,
            workflow: SaveWorkflowResult {
                recap_status: recap_status.clone(),
                plan_status: plan_status.clone(),
                messages: if is_latest_completed_activity {
                    vec![
                        status_message(
                            &recap_status,
                            "Workout recap generated.",
                            "Workout recap failed.",
                            "Workout recap skipped.",
                        ),
                        status_message(
                            &plan_status,
                            "14-day schedule generated.",
                            "14-day schedule failed.",
                            "14-day schedule skipped.",
                        ),
                    ]
                } else {
                    vec![
                        status_message(
                            &recap_status,
                            "Workout recap generated.",
                            "Workout recap failed.",
                            "Workout recap skipped.",
                        ),
                        "14-day schedule skipped because this is not the latest completed activity."
                            .to_string(),
                    ]
                },
            },
        })
    }

    async fn retry_saved_workflow(
        &self,
        user_id: &str,
        workout_id: &str,
        existing: WorkoutSummary,
    ) -> Result<SaveSummaryResult, WorkoutSummaryError> {
        if !has_finished_conversation(&existing) {
            return Ok(SaveSummaryResult {
                summary: existing,
                workflow: SaveWorkflowResult {
                    recap_status: SaveWorkflowStatus::Unchanged,
                    plan_status: SaveWorkflowStatus::Skipped,
                    messages: Vec::new(),
                },
            });
        }

        let is_latest_completed_activity = self
            .is_latest_completed_activity(user_id, workout_id)
            .await?;
        if !is_latest_completed_activity {
            return Ok(SaveSummaryResult {
                summary: existing,
                workflow: SaveWorkflowResult {
                    recap_status: SaveWorkflowStatus::Unchanged,
                    plan_status: SaveWorkflowStatus::Skipped,
                    messages: Vec::new(),
                },
            });
        }

        let recap_before_retry = RecapSnapshot::from_summary(&existing);

        if let (Some(training_plan_service), Some(saved_at_epoch_seconds)) =
            (&self.training_plan_service, existing.saved_at_epoch_seconds)
        {
            match training_plan_service
                .generate_for_saved_workout(user_id, workout_id, saved_at_epoch_seconds)
                .await
            {
                Ok(generated_plan) => {
                    let summary = self.get_existing_summary(user_id, workout_id).await?;
                    let recap_status =
                        if RecapSnapshot::from_summary(&summary) != recap_before_retry {
                            SaveWorkflowStatus::Generated
                        } else {
                            SaveWorkflowStatus::Unchanged
                        };
                    return Ok(SaveSummaryResult {
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
                    });
                }
                Err(error) => {
                    warn!(
                        user_id,
                        workout_id,
                        saved_at_epoch_seconds,
                        error = %error,
                        "Saved workout summary remains persisted after training plan generation retry failure"
                    );

                    let summary = self.get_existing_summary(user_id, workout_id).await?;
                    let recap_status =
                        if RecapSnapshot::from_summary(&summary) != recap_before_retry {
                            SaveWorkflowStatus::Generated
                        } else {
                            SaveWorkflowStatus::Unchanged
                        };
                    return Ok(SaveSummaryResult {
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
                    });
                }
            }
        }
        Ok(SaveSummaryResult {
            summary: existing,
            workflow: SaveWorkflowResult {
                recap_status: SaveWorkflowStatus::Unchanged,
                plan_status: SaveWorkflowStatus::Skipped,
                messages: Vec::new(),
            },
        })
    }
}
