use super::{
    TrainingPlanConversationMessage, TrainingPlanConversationRole, TrainingPlanPlanningContext,
};
use crate::domain::workout_summary::{MessageRole, WorkoutRecap, WorkoutSummary};

pub fn map_workout_summary_to_planning_context(
    summary: WorkoutSummary,
) -> Option<TrainingPlanPlanningContext> {
    let messages = summary
        .messages
        .into_iter()
        .filter(|message| message.role != MessageRole::Tool)
        .map(|message| TrainingPlanConversationMessage {
            role: match message.role {
                MessageRole::Coach => TrainingPlanConversationRole::Coach,
                MessageRole::User => TrainingPlanConversationRole::User,
                MessageRole::Tool => unreachable!(),
            },
            content: message.content,
            created_at_epoch_seconds: message.created_at_epoch_seconds,
        })
        .collect::<Vec<_>>();
    if summary.rpe.is_none() && messages.is_empty() {
        return None;
    }

    Some(TrainingPlanPlanningContext {
        rpe: summary.rpe,
        messages,
    })
}

pub fn workout_recap_from_summary(
    summary: &WorkoutSummary,
    fallback_epoch_seconds: i64,
) -> WorkoutRecap {
    WorkoutRecap::generated(
        summary.workout_recap_text.clone().unwrap_or_else(|| {
            "Preview: placeholder workout recap for training-plan prompt preview.".to_string()
        }),
        summary
            .workout_recap_provider
            .clone()
            .unwrap_or_else(|| "preview".to_string()),
        summary
            .workout_recap_model
            .clone()
            .unwrap_or_else(|| "preview".to_string()),
        summary
            .workout_recap_generated_at_epoch_seconds
            .unwrap_or(fallback_epoch_seconds),
    )
}

#[cfg(test)]
mod tests {
    use super::map_workout_summary_to_planning_context;
    use crate::domain::{
        training_plan::TrainingPlanConversationRole,
        workout_summary::{ConversationMessage, MessageRole, WorkoutSummary},
    };

    #[test]
    fn map_workout_summary_to_planning_context_skips_tool_messages() {
        let context = map_workout_summary_to_planning_context(WorkoutSummary {
            id: "summary-1".to_string(),
            user_id: "user-1".to_string(),
            workout_id: "workout-1".to_string(),
            rpe: Some(7),
            messages: vec![
                ConversationMessage {
                    id: "msg-1".to_string(),
                    role: MessageRole::User,
                    content: "How did I do?".to_string(),
                    tool_call: None,
                    questions: Vec::new(),
                    created_at_epoch_seconds: 1,
                },
                ConversationMessage {
                    id: "msg-2".to_string(),
                    role: MessageRole::Tool,
                    content: "tool output".to_string(),
                    tool_call: None,
                    questions: Vec::new(),
                    created_at_epoch_seconds: 2,
                },
            ],
            provider_transcript: Vec::new(),
            saved_at_epoch_seconds: None,
            workout_recap_text: None,
            workout_recap_provider: None,
            workout_recap_model: None,
            workout_recap_generated_at_epoch_seconds: None,
            created_at_epoch_seconds: 1,
            updated_at_epoch_seconds: 1,
        })
        .expect("planning context");

        assert_eq!(context.rpe, Some(7));
        assert_eq!(context.messages.len(), 1);
        assert!(matches!(
            context.messages[0].role,
            TrainingPlanConversationRole::User
        ));
    }
}
