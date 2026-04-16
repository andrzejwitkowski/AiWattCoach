use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use aiwattcoach::domain::{
    llm::BoxFuture,
    workout_summary::{
        validate_message_content, CoachReply, MessageRole, PersistedUserMessage, SaveSummaryResult,
        SaveWorkflowResult, SaveWorkflowStatus, SendMessageResult, WorkoutRecap, WorkoutSummary,
        WorkoutSummaryError, WorkoutSummaryUseCases,
    },
};

use super::fixtures::sample_summary_for_user;

#[derive(Clone, Default)]
pub(crate) struct TestWorkoutSummaryService {
    summaries: Arc<Mutex<Vec<WorkoutSummary>>>,
    processed_user_messages: Arc<Mutex<Vec<String>>>,
    coach_reply_delay: Option<Duration>,
    availability_configured: bool,
    completed_workout_ids: Arc<Mutex<Option<Vec<String>>>>,
}

impl TestWorkoutSummaryService {
    pub(crate) fn with_summaries(summaries: Vec<WorkoutSummary>) -> Self {
        Self {
            summaries: Arc::new(Mutex::new(summaries)),
            processed_user_messages: Arc::new(Mutex::new(Vec::new())),
            coach_reply_delay: None,
            availability_configured: true,
            completed_workout_ids: Arc::new(Mutex::new(None)),
        }
    }

    pub(crate) fn with_coach_reply_delay(mut self, delay: Duration) -> Self {
        self.coach_reply_delay = Some(delay);
        self
    }

    pub(crate) fn with_availability_configured(mut self, configured: bool) -> Self {
        self.availability_configured = configured;
        self
    }

    pub(crate) fn with_completed_workout_ids(mut self, workout_ids: &[&str]) -> Self {
        self.completed_workout_ids = Arc::new(Mutex::new(Some(
            workout_ids
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
        )));
        self
    }

    pub(crate) fn processed_user_messages(&self) -> Vec<String> {
        self.processed_user_messages.lock().unwrap().clone()
    }

    pub(crate) fn summary(&self, user_id: &str, workout_id: &str) -> Option<WorkoutSummary> {
        self.summaries
            .lock()
            .unwrap()
            .iter()
            .find(|summary| summary.user_id == user_id && summary.workout_id == workout_id)
            .cloned()
    }

    fn validate_completed_target(&self, workout_id: &str) -> Result<(), WorkoutSummaryError> {
        let completed_workout_ids = self.completed_workout_ids.lock().unwrap().clone();
        match completed_workout_ids {
            Some(workout_ids) if !workout_ids.iter().any(|value| value == workout_id) => {
                Err(WorkoutSummaryError::Validation(
                    "workout summary is only available for completed workouts".to_string(),
                ))
            }
            _ => Ok(()),
        }
    }
}

impl WorkoutSummaryUseCases for TestWorkoutSummaryService {
    fn get_summary(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>> {
        if let Err(error) = self.validate_completed_target(workout_id) {
            return Box::pin(async move { Err(error) });
        }

        let summaries = self.summaries.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        Box::pin(async move {
            summaries
                .lock()
                .unwrap()
                .iter()
                .find(|summary| summary.user_id == user_id && summary.workout_id == workout_id)
                .cloned()
                .ok_or(WorkoutSummaryError::NotFound)
        })
    }

    fn create_summary(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>> {
        if let Err(error) = self.validate_completed_target(workout_id) {
            return Box::pin(async move { Err(error) });
        }

        let summaries = self.summaries.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        Box::pin(async move {
            let mut summaries = summaries.lock().unwrap();
            if let Some(existing) = summaries
                .iter()
                .find(|summary| summary.user_id == user_id && summary.workout_id == workout_id)
                .cloned()
            {
                return Ok(existing);
            }

            let summary = sample_summary_for_user(&user_id, &workout_id);
            summaries.push(summary.clone());
            Ok(summary)
        })
    }

    fn list_summaries(
        &self,
        user_id: &str,
        workout_ids: Vec<String>,
    ) -> BoxFuture<Result<Vec<WorkoutSummary>, WorkoutSummaryError>> {
        let completed_workout_ids = match self.completed_workout_ids.lock().unwrap().clone() {
            Some(allowed) => workout_ids
                .into_iter()
                .filter(|workout_id| allowed.iter().any(|value| value == workout_id))
                .collect::<Vec<_>>(),
            None => workout_ids,
        };

        let summaries = self.summaries.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let mut filtered: Vec<_> = summaries
                .lock()
                .unwrap()
                .iter()
                .filter(|summary| {
                    summary.user_id == user_id
                        && completed_workout_ids
                            .iter()
                            .any(|id| id == &summary.workout_id)
                })
                .cloned()
                .collect();
            filtered.sort_by(|left, right| {
                right
                    .updated_at_epoch_seconds
                    .cmp(&left.updated_at_epoch_seconds)
                    .then_with(|| {
                        right
                            .created_at_epoch_seconds
                            .cmp(&left.created_at_epoch_seconds)
                    })
            });
            Ok(filtered)
        })
    }

    fn update_rpe(
        &self,
        user_id: &str,
        workout_id: &str,
        rpe: u8,
    ) -> BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>> {
        if let Err(error) = self.validate_completed_target(workout_id) {
            return Box::pin(async move { Err(error) });
        }

        let summaries = self.summaries.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        Box::pin(async move {
            let mut summaries = summaries.lock().unwrap();
            let Some(summary) = summaries
                .iter_mut()
                .find(|summary| summary.user_id == user_id && summary.workout_id == workout_id)
            else {
                return Err(WorkoutSummaryError::NotFound);
            };

            if summary.saved_at_epoch_seconds.is_some() {
                return Err(WorkoutSummaryError::Locked);
            }

            summary.rpe = Some(rpe);
            summary.updated_at_epoch_seconds = 1_700_000_100;
            Ok(summary.clone())
        })
    }

    fn mark_saved(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> BoxFuture<Result<SaveSummaryResult, WorkoutSummaryError>> {
        if let Err(error) = self.validate_completed_target(workout_id) {
            return Box::pin(async move { Err(error) });
        }

        let summaries = self.summaries.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        Box::pin(async move {
            let mut summaries = summaries.lock().unwrap();
            let Some(summary) = summaries
                .iter_mut()
                .find(|summary| summary.user_id == user_id && summary.workout_id == workout_id)
            else {
                return Err(WorkoutSummaryError::NotFound);
            };

            if summary.rpe.is_none() {
                return Err(WorkoutSummaryError::Validation(
                    "rpe must be set before saving workout summary".to_string(),
                ));
            }

            summary.saved_at_epoch_seconds = Some(1_700_000_100);
            summary.updated_at_epoch_seconds = 1_700_000_100;
            Ok(SaveSummaryResult {
                summary: summary.clone(),
                workflow: SaveWorkflowResult {
                    recap_status: SaveWorkflowStatus::Skipped,
                    plan_status: SaveWorkflowStatus::Skipped,
                    messages: Vec::new(),
                },
            })
        })
    }

    fn reopen_summary(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>> {
        if let Err(error) = self.validate_completed_target(workout_id) {
            return Box::pin(async move { Err(error) });
        }

        let summaries = self.summaries.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        Box::pin(async move {
            let mut summaries = summaries.lock().unwrap();
            let Some(summary) = summaries
                .iter_mut()
                .find(|summary| summary.user_id == user_id && summary.workout_id == workout_id)
            else {
                return Err(WorkoutSummaryError::NotFound);
            };

            summary.saved_at_epoch_seconds = None;
            summary.updated_at_epoch_seconds = 1_700_000_300;
            Ok(summary.clone())
        })
    }

    fn persist_workout_recap(
        &self,
        user_id: &str,
        workout_id: &str,
        recap: WorkoutRecap,
    ) -> BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>> {
        if let Err(error) = self.validate_completed_target(workout_id) {
            return Box::pin(async move { Err(error) });
        }

        let summaries = self.summaries.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        Box::pin(async move {
            let mut summaries = summaries.lock().unwrap();
            let Some(summary) = summaries
                .iter_mut()
                .find(|summary| summary.user_id == user_id && summary.workout_id == workout_id)
            else {
                return Err(WorkoutSummaryError::NotFound);
            };

            summary.workout_recap_text = Some(recap.text);
            summary.workout_recap_provider = Some(recap.provider);
            summary.workout_recap_model = Some(recap.model);
            summary.workout_recap_generated_at_epoch_seconds =
                Some(recap.generated_at_epoch_seconds);
            summary.updated_at_epoch_seconds = 1_700_000_100;
            Ok(summary.clone())
        })
    }

    fn send_message(
        &self,
        user_id: &str,
        workout_id: &str,
        content: String,
    ) -> BoxFuture<Result<SendMessageResult, WorkoutSummaryError>> {
        if let Err(error) = self.validate_completed_target(workout_id) {
            return Box::pin(async move { Err(error) });
        }

        let summaries = self.summaries.clone();
        let processed_user_messages = self.processed_user_messages.clone();
        let availability_configured = self.availability_configured;
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        Box::pin(async move {
            let content = validate_message_content(&content)?;
            let mut summaries = summaries.lock().unwrap();
            let Some(summary) = summaries
                .iter_mut()
                .find(|summary| summary.user_id == user_id && summary.workout_id == workout_id)
            else {
                return Err(WorkoutSummaryError::NotFound);
            };

            if summary.saved_at_epoch_seconds.is_some() {
                return Err(WorkoutSummaryError::Locked);
            }
            if summary.rpe.is_none() {
                return Err(WorkoutSummaryError::Validation(
                    "rpe must be set before chatting with coach".to_string(),
                ));
            }
            if !availability_configured {
                return Err(WorkoutSummaryError::Validation(
                    "availability must be configured before chatting with coach".to_string(),
                ));
            }

            processed_user_messages
                .lock()
                .unwrap()
                .push(content.clone());

            let next_user_suffix = summary.messages.len() + 1;
            let user_message = aiwattcoach::domain::workout_summary::ConversationMessage {
                id: format!("message-user-{next_user_suffix}"),
                role: MessageRole::User,
                content,
                created_at_epoch_seconds: 1_700_000_000,
            };
            let coach_message = aiwattcoach::domain::workout_summary::ConversationMessage {
                id: format!("message-coach-{}", next_user_suffix + 1),
                role: MessageRole::Coach,
                content: "Thanks, that helps. What stood out most about how the workout felt compared with the plan?".to_string(),
                created_at_epoch_seconds: 1_700_000_000,
            };

            summary.messages.push(user_message.clone());
            summary.messages.push(coach_message.clone());
            summary.updated_at_epoch_seconds = 1_700_000_100;

            Ok(SendMessageResult {
                summary: summary.clone(),
                user_message,
                coach_message,
            })
        })
    }

    fn append_user_message(
        &self,
        user_id: &str,
        workout_id: &str,
        content: String,
    ) -> BoxFuture<Result<PersistedUserMessage, WorkoutSummaryError>> {
        if let Err(error) = self.validate_completed_target(workout_id) {
            return Box::pin(async move { Err(error) });
        }

        let summaries = self.summaries.clone();
        let processed_user_messages = self.processed_user_messages.clone();
        let availability_configured = self.availability_configured;
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        Box::pin(async move {
            let content = validate_message_content(&content)?;
            let mut summaries = summaries.lock().unwrap();
            let Some(summary) = summaries
                .iter_mut()
                .find(|summary| summary.user_id == user_id && summary.workout_id == workout_id)
            else {
                return Err(WorkoutSummaryError::NotFound);
            };

            if summary.saved_at_epoch_seconds.is_some() {
                return Err(WorkoutSummaryError::Locked);
            }
            if summary.rpe.is_none() {
                return Err(WorkoutSummaryError::Validation(
                    "rpe must be set before chatting with coach".to_string(),
                ));
            }
            if !availability_configured {
                return Err(WorkoutSummaryError::Validation(
                    "availability must be configured before chatting with coach".to_string(),
                ));
            }

            processed_user_messages
                .lock()
                .unwrap()
                .push(content.clone());

            let next_user_suffix = summary.messages.len() + 1;
            let user_message = aiwattcoach::domain::workout_summary::ConversationMessage {
                id: format!("message-user-{next_user_suffix}"),
                role: MessageRole::User,
                content,
                created_at_epoch_seconds: 1_700_000_000,
            };

            summary.messages.push(user_message.clone());
            summary.updated_at_epoch_seconds = 1_700_000_100;

            Ok(PersistedUserMessage {
                summary: summary.clone(),
                user_message,
                athlete_summary_may_regenerate_before_reply: false,
            })
        })
    }

    fn generate_coach_reply(
        &self,
        user_id: &str,
        workout_id: &str,
        user_message_id: String,
    ) -> BoxFuture<Result<CoachReply, WorkoutSummaryError>> {
        if let Err(error) = self.validate_completed_target(workout_id) {
            return Box::pin(async move { Err(error) });
        }

        let summaries = self.summaries.clone();
        let coach_reply_delay = self.coach_reply_delay;
        let availability_configured = self.availability_configured;
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        Box::pin(async move {
            if let Some(delay) = coach_reply_delay {
                tokio::time::sleep(delay).await;
            }

            let mut summaries = summaries.lock().unwrap();
            let Some(summary) = summaries
                .iter_mut()
                .find(|summary| summary.user_id == user_id && summary.workout_id == workout_id)
            else {
                return Err(WorkoutSummaryError::NotFound);
            };

            if summary.saved_at_epoch_seconds.is_some() {
                return Err(WorkoutSummaryError::Locked);
            }
            if summary.rpe.is_none() {
                return Err(WorkoutSummaryError::Validation(
                    "rpe must be set before chatting with coach".to_string(),
                ));
            }
            if !availability_configured {
                return Err(WorkoutSummaryError::Validation(
                    "availability must be configured before chatting with coach".to_string(),
                ));
            }

            let user_message_content = summary
                .messages
                .iter()
                .find(|message| message.id == user_message_id && message.role == MessageRole::User)
                .map(|message| message.content.clone())
                .ok_or_else(|| {
                    WorkoutSummaryError::Validation(
                        "user message must be persisted before generating coach reply".to_string(),
                    )
                })?;

            let next_coach_suffix = summary.messages.len() + 1;
            let coach_message = aiwattcoach::domain::workout_summary::ConversationMessage {
                id: format!("message-coach-{next_coach_suffix}"),
                role: MessageRole::Coach,
                content: format!("Coach reply to: {user_message_content}"),
                created_at_epoch_seconds: 1_700_000_000,
            };

            summary.messages.push(coach_message.clone());
            summary.updated_at_epoch_seconds = 1_700_000_100;

            Ok(CoachReply {
                summary: summary.clone(),
                coach_message,
                athlete_summary_was_regenerated: false,
            })
        })
    }
}
