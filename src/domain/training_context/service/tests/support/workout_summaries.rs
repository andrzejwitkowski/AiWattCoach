use crate::domain::workout_summary::{
    BoxFuture as WorkoutSummaryBoxFuture, ConversationMessage, MessageRole, WorkoutRecap,
    WorkoutSummary, WorkoutSummaryError, WorkoutSummaryRepository,
};

#[derive(Clone)]
pub(crate) struct TestWorkoutSummaryRepository;

#[derive(Clone)]
pub(crate) struct EventIdOnlySummaryRepository;

#[derive(Clone)]
pub(crate) struct AliasSummaryRepository;

fn summary_for_workout_id(workout_id: &str) -> WorkoutSummary {
    WorkoutSummary {
        id: format!("summary-{workout_id}"),
        user_id: "user-1".to_string(),
        workout_id: workout_id.to_string(),
        rpe: Some(7),
        messages: vec![ConversationMessage {
            id: "message-1".to_string(),
            role: MessageRole::User,
            content: "felt controlled".to_string(),
            tool_call: None,
            created_at_epoch_seconds: 1,
        }],
        hidden_transcript: Vec::new(),
        saved_at_epoch_seconds: None,
        workout_recap_text: Some("Strong sweet spot execution with steady control".to_string()),
        workout_recap_provider: Some("openrouter".to_string()),
        workout_recap_model: Some("test-model".to_string()),
        workout_recap_generated_at_epoch_seconds: Some(1),
        created_at_epoch_seconds: 1,
        updated_at_epoch_seconds: 1,
    }
}

fn event_id_only_summary(workout_id: &str) -> WorkoutSummary {
    WorkoutSummary {
        id: format!("summary-{workout_id}"),
        user_id: "user-1".to_string(),
        workout_id: workout_id.to_string(),
        rpe: Some(8),
        messages: Vec::new(),
        hidden_transcript: Vec::new(),
        saved_at_epoch_seconds: None,
        workout_recap_text: Some("Matched legacy event summary".to_string()),
        workout_recap_provider: Some("openrouter".to_string()),
        workout_recap_model: Some("test-model".to_string()),
        workout_recap_generated_at_epoch_seconds: Some(1),
        created_at_epoch_seconds: 1,
        updated_at_epoch_seconds: 1,
    }
}

fn alias_backed_summary(requested_workout_id: &str) -> WorkoutSummary {
    WorkoutSummary {
        id: "summary-wahoo-alias".to_string(),
        user_id: "user-1".to_string(),
        workout_id: requested_workout_id.to_string(),
        rpe: Some(9),
        messages: Vec::new(),
        hidden_transcript: Vec::new(),
        saved_at_epoch_seconds: None,
        workout_recap_text: Some("Recovered alias-backed recap".to_string()),
        workout_recap_provider: Some("openrouter".to_string()),
        workout_recap_model: Some("test-model".to_string()),
        workout_recap_generated_at_epoch_seconds: Some(1),
        created_at_epoch_seconds: 1,
        updated_at_epoch_seconds: 1,
    }
}

impl WorkoutSummaryRepository for TestWorkoutSummaryRepository {
    fn find_by_user_id_and_workout_id(
        &self,
        _user_id: &str,
        _workout_id: &str,
    ) -> WorkoutSummaryBoxFuture<Result<Option<WorkoutSummary>, WorkoutSummaryError>> {
        Box::pin(async { Ok(None) })
    }

    fn find_by_user_id_and_workout_ids(
        &self,
        _user_id: &str,
        workout_ids: Vec<String>,
    ) -> WorkoutSummaryBoxFuture<Result<Vec<WorkoutSummary>, WorkoutSummaryError>> {
        Box::pin(async move {
            Ok(workout_ids
                .into_iter()
                .filter(|id| id == "ride-1" || id == "101")
                .map(|id| summary_for_workout_id(&id))
                .collect())
        })
    }

    fn create(
        &self,
        _summary: WorkoutSummary,
    ) -> WorkoutSummaryBoxFuture<Result<WorkoutSummary, WorkoutSummaryError>> {
        unreachable!()
    }

    fn update_rpe(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _rpe: u8,
        _updated_at_epoch_seconds: i64,
    ) -> WorkoutSummaryBoxFuture<Result<(), WorkoutSummaryError>> {
        unreachable!()
    }

    fn append_message(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _message: ConversationMessage,
        _updated_at_epoch_seconds: i64,
    ) -> WorkoutSummaryBoxFuture<Result<(), WorkoutSummaryError>> {
        unreachable!()
    }

    fn set_saved_state(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _saved_at_epoch_seconds: Option<i64>,
        _updated_at_epoch_seconds: i64,
    ) -> WorkoutSummaryBoxFuture<Result<(), WorkoutSummaryError>> {
        unreachable!()
    }

    fn replace_hidden_transcript(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _hidden_transcript: Vec<crate::domain::llm::LlmChatMessage>,
        _expected_updated_at_epoch_seconds: i64,
        _updated_at_epoch_seconds: i64,
    ) -> WorkoutSummaryBoxFuture<Result<(), WorkoutSummaryError>> {
        unreachable!()
    }

    fn persist_workout_recap(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _recap: WorkoutRecap,
        _updated_at_epoch_seconds: i64,
    ) -> WorkoutSummaryBoxFuture<Result<(), WorkoutSummaryError>> {
        unreachable!()
    }

    fn find_message_by_id(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _message_id: &str,
    ) -> WorkoutSummaryBoxFuture<Result<Option<ConversationMessage>, WorkoutSummaryError>> {
        unreachable!()
    }
}

impl WorkoutSummaryRepository for EventIdOnlySummaryRepository {
    fn find_by_user_id_and_workout_id(
        &self,
        _user_id: &str,
        _workout_id: &str,
    ) -> WorkoutSummaryBoxFuture<Result<Option<WorkoutSummary>, WorkoutSummaryError>> {
        Box::pin(async { Ok(None) })
    }

    fn find_by_user_id_and_workout_ids(
        &self,
        _user_id: &str,
        workout_ids: Vec<String>,
    ) -> WorkoutSummaryBoxFuture<Result<Vec<WorkoutSummary>, WorkoutSummaryError>> {
        Box::pin(async move {
            Ok(workout_ids
                .into_iter()
                .filter(|id| id == "101")
                .map(|id| event_id_only_summary(&id))
                .collect())
        })
    }

    fn create(
        &self,
        _summary: WorkoutSummary,
    ) -> WorkoutSummaryBoxFuture<Result<WorkoutSummary, WorkoutSummaryError>> {
        unreachable!()
    }

    fn update_rpe(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _rpe: u8,
        _updated_at_epoch_seconds: i64,
    ) -> WorkoutSummaryBoxFuture<Result<(), WorkoutSummaryError>> {
        unreachable!()
    }

    fn append_message(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _message: ConversationMessage,
        _updated_at_epoch_seconds: i64,
    ) -> WorkoutSummaryBoxFuture<Result<(), WorkoutSummaryError>> {
        unreachable!()
    }

    fn set_saved_state(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _saved_at_epoch_seconds: Option<i64>,
        _updated_at_epoch_seconds: i64,
    ) -> WorkoutSummaryBoxFuture<Result<(), WorkoutSummaryError>> {
        unreachable!()
    }

    fn replace_hidden_transcript(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _hidden_transcript: Vec<crate::domain::llm::LlmChatMessage>,
        _expected_updated_at_epoch_seconds: i64,
        _updated_at_epoch_seconds: i64,
    ) -> WorkoutSummaryBoxFuture<Result<(), WorkoutSummaryError>> {
        unreachable!()
    }

    fn persist_workout_recap(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _recap: WorkoutRecap,
        _updated_at_epoch_seconds: i64,
    ) -> WorkoutSummaryBoxFuture<Result<(), WorkoutSummaryError>> {
        unreachable!()
    }

    fn find_message_by_id(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _message_id: &str,
    ) -> WorkoutSummaryBoxFuture<Result<Option<ConversationMessage>, WorkoutSummaryError>> {
        unreachable!()
    }
}

impl WorkoutSummaryRepository for AliasSummaryRepository {
    fn find_by_user_id_and_workout_id(
        &self,
        _user_id: &str,
        _workout_id: &str,
    ) -> WorkoutSummaryBoxFuture<Result<Option<WorkoutSummary>, WorkoutSummaryError>> {
        Box::pin(async { Ok(None) })
    }

    fn find_by_user_id_and_workout_ids(
        &self,
        _user_id: &str,
        workout_ids: Vec<String>,
    ) -> WorkoutSummaryBoxFuture<Result<Vec<WorkoutSummary>, WorkoutSummaryError>> {
        Box::pin(async move {
            Ok(workout_ids
                .into_iter()
                .filter(|id| id == "ride-1")
                .map(|id| alias_backed_summary(&id))
                .collect())
        })
    }

    fn create(
        &self,
        _summary: WorkoutSummary,
    ) -> WorkoutSummaryBoxFuture<Result<WorkoutSummary, WorkoutSummaryError>> {
        unreachable!()
    }

    fn update_rpe(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _rpe: u8,
        _updated_at_epoch_seconds: i64,
    ) -> WorkoutSummaryBoxFuture<Result<(), WorkoutSummaryError>> {
        unreachable!()
    }

    fn append_message(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _message: ConversationMessage,
        _updated_at_epoch_seconds: i64,
    ) -> WorkoutSummaryBoxFuture<Result<(), WorkoutSummaryError>> {
        unreachable!()
    }

    fn set_saved_state(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _saved_at_epoch_seconds: Option<i64>,
        _updated_at_epoch_seconds: i64,
    ) -> WorkoutSummaryBoxFuture<Result<(), WorkoutSummaryError>> {
        unreachable!()
    }

    fn replace_hidden_transcript(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _hidden_transcript: Vec<crate::domain::llm::LlmChatMessage>,
        _expected_updated_at_epoch_seconds: i64,
        _updated_at_epoch_seconds: i64,
    ) -> WorkoutSummaryBoxFuture<Result<(), WorkoutSummaryError>> {
        unreachable!()
    }

    fn persist_workout_recap(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _recap: WorkoutRecap,
        _updated_at_epoch_seconds: i64,
    ) -> WorkoutSummaryBoxFuture<Result<(), WorkoutSummaryError>> {
        unreachable!()
    }

    fn find_message_by_id(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _message_id: &str,
    ) -> WorkoutSummaryBoxFuture<Result<Option<ConversationMessage>, WorkoutSummaryError>> {
        unreachable!()
    }
}
