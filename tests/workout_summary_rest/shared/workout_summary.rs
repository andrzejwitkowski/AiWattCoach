use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use aiwattcoach::domain::{
    llm::BoxFuture,
    workout_summary::{
        validate_message_content, CoachReply, MessageRole, PersistedUserMessage, SendMessageResult,
        WorkoutRecap, WorkoutSummary, WorkoutSummaryError, WorkoutSummaryUseCases,
    },
};

#[derive(Clone, Default)]
pub(crate) struct TestWorkoutSummaryService {
    summaries: Arc<Mutex<Vec<WorkoutSummary>>>,
    processed_user_messages: Arc<Mutex<Vec<String>>>,
    coach_reply_delay: Option<Duration>,
}

impl TestWorkoutSummaryService {
    pub(crate) fn with_summaries(summaries: Vec<WorkoutSummary>) -> Self {
        Self {
            summaries: Arc::new(Mutex::new(summaries)),
            processed_user_messages: Arc::new(Mutex::new(Vec::new())),
            coach_reply_delay: None,
        }
    }

    pub(crate) fn with_coach_reply_delay(mut self, delay: Duration) -> Self {
        self.coach_reply_delay = Some(delay);
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
}

impl WorkoutSummaryUseCases for TestWorkoutSummaryService {
    fn get_summary(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>> {
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
        let summaries = self.summaries.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let mut filtered: Vec<_> = summaries
                .lock()
                .unwrap()
                .iter()
                .filter(|summary| {
                    summary.user_id == user_id
                        && workout_ids.iter().any(|id| id == &summary.workout_id)
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
    ) -> BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>> {
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
            Ok(summary.clone())
        })
    }

    fn reopen_summary(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>> {
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
        let summaries = self.summaries.clone();
        let processed_user_messages = self.processed_user_messages.clone();
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
        let summaries = self.summaries.clone();
        let processed_user_messages = self.processed_user_messages.clone();
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
        let summaries = self.summaries.clone();
        let coach_reply_delay = self.coach_reply_delay;
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

pub(crate) fn sample_summary(workout_id: &str) -> WorkoutSummary {
    sample_summary_for_user("user-1", workout_id)
}

fn sample_summary_for_user(user_id: &str, workout_id: &str) -> WorkoutSummary {
    WorkoutSummary {
        id: format!("summary-{workout_id}"),
        user_id: user_id.to_string(),
        workout_id: workout_id.to_string(),
        rpe: Some(6),
        messages: Vec::new(),
        saved_at_epoch_seconds: None,
        workout_recap_text: None,
        workout_recap_provider: None,
        workout_recap_model: None,
        workout_recap_generated_at_epoch_seconds: None,
        created_at_epoch_seconds: 1_700_000_000,
        updated_at_epoch_seconds: 1_700_000_000,
    }
}

pub(crate) fn sample_summary_with_updated_at(
    workout_id: &str,
    updated_at_epoch_seconds: i64,
) -> WorkoutSummary {
    let mut summary = sample_summary(workout_id);
    summary.updated_at_epoch_seconds = updated_at_epoch_seconds;
    summary
}
