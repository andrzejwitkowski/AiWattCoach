use std::sync::{Arc, Mutex};

use aiwattcoach::domain::workout_summary::{
    validate_message_content, CoachReply, ConversationMessage, MessageRole, PersistedUserMessage,
    SendMessageResult, WorkoutSummary, WorkoutSummaryError, WorkoutSummaryUseCases,
};

type BoxFuture<T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + 'static>>;

#[derive(Clone, Default)]
pub(crate) struct TestWorkoutSummaryService {
    summaries: Arc<Mutex<Vec<WorkoutSummary>>>,
    processed_user_messages: Arc<Mutex<Vec<String>>>,
}

impl TestWorkoutSummaryService {
    pub(crate) fn with_summaries(summaries: Vec<WorkoutSummary>) -> Self {
        Self {
            summaries: Arc::new(Mutex::new(summaries)),
            processed_user_messages: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub(crate) fn summary(&self, user_id: &str, workout_id: &str) -> Option<WorkoutSummary> {
        self.summaries
            .lock()
            .unwrap()
            .iter()
            .find(|summary| summary.user_id == user_id && summary.workout_id == workout_id)
            .cloned()
    }

    fn find_summary(&self, user_id: &str, workout_id: &str) -> Option<WorkoutSummary> {
        self.summary(user_id, workout_id)
    }

    pub(crate) fn processed_user_messages(&self) -> Vec<String> {
        self.processed_user_messages.lock().unwrap().clone()
    }
}

impl WorkoutSummaryUseCases for TestWorkoutSummaryService {
    fn get_summary(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>> {
        let summary = self.find_summary(user_id, workout_id);
        Box::pin(async move { summary.ok_or(WorkoutSummaryError::NotFound) })
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

            let summary = WorkoutSummary {
                id: format!("summary-{workout_id}"),
                user_id,
                workout_id,
                rpe: None,
                messages: Vec::new(),
                saved_at_epoch_seconds: None,
                created_at_epoch_seconds: 1_700_000_000,
                updated_at_epoch_seconds: 1_700_000_000,
            };
            summaries.push(summary.clone());
            Ok(summary)
        })
    }

    fn list_summaries(
        &self,
        user_id: &str,
        workout_ids: Vec<String>,
    ) -> BoxFuture<Result<Vec<WorkoutSummary>, WorkoutSummaryError>> {
        let summaries = self.summaries.lock().unwrap().clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let mut summaries = summaries
                .into_iter()
                .filter(|summary| {
                    summary.user_id == user_id && workout_ids.contains(&summary.workout_id)
                })
                .collect::<Vec<_>>();
            summaries.sort_by(|left, right| {
                right
                    .updated_at_epoch_seconds
                    .cmp(&left.updated_at_epoch_seconds)
                    .then_with(|| {
                        right
                            .created_at_epoch_seconds
                            .cmp(&left.created_at_epoch_seconds)
                    })
            });
            Ok(summaries)
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
            if !(1..=10).contains(&rpe) {
                return Err(WorkoutSummaryError::Validation(
                    "rpe must be between 1 and 10".to_string(),
                ));
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
            if summary.saved_at_epoch_seconds.is_none() {
                summary.saved_at_epoch_seconds = Some(1_700_000_100);
                summary.updated_at_epoch_seconds = 1_700_000_100;
            }
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

            let user_message = ConversationMessage {
                id: "message-user-1".to_string(),
                role: MessageRole::User,
                content,
                created_at_epoch_seconds: 1_700_000_000,
            };
            let coach_message = ConversationMessage {
                id: "message-coach-1".to_string(),
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

            let user_message = ConversationMessage {
                id: "message-user-1".to_string(),
                role: MessageRole::User,
                content,
                created_at_epoch_seconds: 1_700_000_000,
            };

            summary.messages.push(user_message.clone());
            summary.updated_at_epoch_seconds = 1_700_000_100;

            Ok(PersistedUserMessage {
                summary: summary.clone(),
                user_message,
            })
        })
    }

    fn generate_coach_reply(
        &self,
        user_id: &str,
        workout_id: &str,
        user_message_content: String,
    ) -> BoxFuture<Result<CoachReply, WorkoutSummaryError>> {
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
            if summary.rpe.is_none() {
                return Err(WorkoutSummaryError::Validation(
                    "rpe must be set before chatting with coach".to_string(),
                ));
            }

            let coach_message = ConversationMessage {
                id: "message-coach-1".to_string(),
                role: MessageRole::Coach,
                content: format!("Coach reply to: {user_message_content}"),
                created_at_epoch_seconds: 1_700_000_000,
            };

            summary.messages.push(coach_message.clone());
            summary.updated_at_epoch_seconds = 1_700_000_100;

            Ok(CoachReply {
                summary: summary.clone(),
                coach_message,
            })
        })
    }
}

pub(crate) fn sample_summary(workout_id: &str) -> WorkoutSummary {
    WorkoutSummary {
        id: format!("summary-{workout_id}"),
        user_id: "user-1".to_string(),
        workout_id: workout_id.to_string(),
        rpe: Some(6),
        messages: Vec::new(),
        saved_at_epoch_seconds: None,
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
