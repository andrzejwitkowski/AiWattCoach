use std::sync::{Arc, Mutex};

use aiwattcoach::domain::workout_summary::{
    CoachReply, ConversationMessage, MessageRole, PersistedUserMessage, SendMessageResult,
    WorkoutSummary, WorkoutSummaryError, WorkoutSummaryUseCases,
};

type BoxFuture<T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + 'static>>;

#[derive(Clone, Default)]
pub(crate) struct TestWorkoutSummaryService {
    summaries: Arc<Mutex<Vec<WorkoutSummary>>>,
}

impl TestWorkoutSummaryService {
    pub(crate) fn with_summaries(summaries: Vec<WorkoutSummary>) -> Self {
        Self {
            summaries: Arc::new(Mutex::new(summaries)),
        }
    }

    pub(crate) fn summary(&self, user_id: &str, event_id: &str) -> Option<WorkoutSummary> {
        self.summaries
            .lock()
            .unwrap()
            .iter()
            .find(|summary| summary.user_id == user_id && summary.event_id == event_id)
            .cloned()
    }

    fn find_summary(&self, user_id: &str, event_id: &str) -> Option<WorkoutSummary> {
        self.summary(user_id, event_id)
    }
}

impl WorkoutSummaryUseCases for TestWorkoutSummaryService {
    fn get_summary(
        &self,
        user_id: &str,
        event_id: &str,
    ) -> BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>> {
        let summary = self.find_summary(user_id, event_id);
        Box::pin(async move { summary.ok_or(WorkoutSummaryError::NotFound) })
    }

    fn create_summary(
        &self,
        user_id: &str,
        event_id: &str,
    ) -> BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>> {
        let existing = self.find_summary(user_id, event_id);
        let summaries = self.summaries.clone();
        let user_id = user_id.to_string();
        let event_id = event_id.to_string();
        Box::pin(async move {
            if let Some(existing) = existing {
                return Ok(existing);
            }

            let summary = WorkoutSummary {
                id: format!("summary-{event_id}"),
                user_id,
                event_id,
                rpe: None,
                messages: Vec::new(),
                created_at_epoch_seconds: 1_700_000_000,
                updated_at_epoch_seconds: 1_700_000_000,
            };
            summaries.lock().unwrap().push(summary.clone());
            Ok(summary)
        })
    }

    fn list_summaries(
        &self,
        user_id: &str,
        event_ids: Vec<String>,
    ) -> BoxFuture<Result<Vec<WorkoutSummary>, WorkoutSummaryError>> {
        let summaries = self.summaries.lock().unwrap().clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let mut summaries = summaries
                .into_iter()
                .filter(|summary| {
                    summary.user_id == user_id && event_ids.contains(&summary.event_id)
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
        event_id: &str,
        rpe: u8,
    ) -> BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>> {
        let summaries = self.summaries.clone();
        let user_id = user_id.to_string();
        let event_id = event_id.to_string();
        Box::pin(async move {
            if !(1..=10).contains(&rpe) {
                return Err(WorkoutSummaryError::Validation(
                    "rpe must be between 1 and 10".to_string(),
                ));
            }

            let mut summaries = summaries.lock().unwrap();
            let Some(summary) = summaries
                .iter_mut()
                .find(|summary| summary.user_id == user_id && summary.event_id == event_id)
            else {
                return Err(WorkoutSummaryError::NotFound);
            };
            summary.rpe = Some(rpe);
            summary.updated_at_epoch_seconds = 1_700_000_100;
            Ok(summary.clone())
        })
    }

    fn send_message(
        &self,
        user_id: &str,
        event_id: &str,
        content: String,
    ) -> BoxFuture<Result<SendMessageResult, WorkoutSummaryError>> {
        let summaries = self.summaries.clone();
        let user_id = user_id.to_string();
        let event_id = event_id.to_string();
        Box::pin(async move {
            let mut summaries = summaries.lock().unwrap();
            let Some(summary) = summaries
                .iter_mut()
                .find(|summary| summary.user_id == user_id && summary.event_id == event_id)
            else {
                return Err(WorkoutSummaryError::NotFound);
            };

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
        event_id: &str,
        content: String,
    ) -> BoxFuture<Result<PersistedUserMessage, WorkoutSummaryError>> {
        let summaries = self.summaries.clone();
        let user_id = user_id.to_string();
        let event_id = event_id.to_string();
        Box::pin(async move {
            let mut summaries = summaries.lock().unwrap();
            let Some(summary) = summaries
                .iter_mut()
                .find(|summary| summary.user_id == user_id && summary.event_id == event_id)
            else {
                return Err(WorkoutSummaryError::NotFound);
            };

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
        event_id: &str,
        _user_message_content: String,
    ) -> BoxFuture<Result<CoachReply, WorkoutSummaryError>> {
        let summaries = self.summaries.clone();
        let user_id = user_id.to_string();
        let event_id = event_id.to_string();
        Box::pin(async move {
            let mut summaries = summaries.lock().unwrap();
            let Some(summary) = summaries
                .iter_mut()
                .find(|summary| summary.user_id == user_id && summary.event_id == event_id)
            else {
                return Err(WorkoutSummaryError::NotFound);
            };

            let coach_message = ConversationMessage {
                id: "message-coach-1".to_string(),
                role: MessageRole::Coach,
                content: "Thanks, that helps. What stood out most about how the workout felt compared with the plan?".to_string(),
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

pub(crate) fn sample_summary(event_id: &str) -> WorkoutSummary {
    WorkoutSummary {
        id: format!("summary-{event_id}"),
        user_id: "user-1".to_string(),
        event_id: event_id.to_string(),
        rpe: Some(6),
        messages: Vec::new(),
        created_at_epoch_seconds: 1_700_000_000,
        updated_at_epoch_seconds: 1_700_000_000,
    }
}

pub(crate) fn sample_summary_with_updated_at(
    event_id: &str,
    updated_at_epoch_seconds: i64,
) -> WorkoutSummary {
    let mut summary = sample_summary(event_id);
    summary.updated_at_epoch_seconds = updated_at_epoch_seconds;
    summary
}
