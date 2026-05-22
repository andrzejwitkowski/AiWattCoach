use std::sync::Arc;

use super::context_prelude::PACKED_TRAINING_CONTEXT_LEGEND;

use crate::domain::{
    identity::Clock,
    llm::{
        build_chat_request, current_date_string, find_reusable_context_cache,
        persist_reusable_context_cache, rebuild_conversation_with_provider_transcript,
        reusable_context_cache_key, BoxFuture, LlmChatMessage, LlmChatPort, LlmChatRequestInput,
        LlmContextCacheRepository, LlmError, LlmMessageRole, LlmProvider,
        ReusableContextCacheLookup, ReusableContextCacheUpsert, UserLlmConfigProvider,
    },
    llm_tools::{
        run_tool_loop, with_tool_prompt_guidance, GetSelectedWorkoutDataPort, LlmToolLoopOutput,
        ToolExecutionContext, ToolScope,
    },
    training_context::TrainingContextBuilder,
    workout_summary::{workout_summary_coach_reply_json_schema, WorkoutCoach, WorkoutSummary},
};

#[derive(Clone)]
pub struct LlmWorkoutCoach<Time>
where
    Time: Clock,
{
    llm_chat_port: Arc<dyn LlmChatPort>,
    config_provider: Arc<dyn UserLlmConfigProvider>,
    training_context_builder: Arc<dyn TrainingContextBuilder>,
    context_cache_repository: Option<Arc<dyn LlmContextCacheRepository>>,
    data_port: Option<Arc<dyn GetSelectedWorkoutDataPort>>,
    clock: Time,
}

impl<Time> LlmWorkoutCoach<Time>
where
    Time: Clock,
{
    pub fn new(
        llm_chat_port: Arc<dyn LlmChatPort>,
        config_provider: Arc<dyn UserLlmConfigProvider>,
        training_context_builder: Arc<dyn TrainingContextBuilder>,
        clock: Time,
    ) -> Self {
        Self {
            llm_chat_port,
            config_provider,
            training_context_builder,
            context_cache_repository: None,
            data_port: None,
            clock,
        }
    }

    pub fn with_context_cache_repository(
        mut self,
        context_cache_repository: Arc<dyn LlmContextCacheRepository>,
    ) -> Self {
        self.context_cache_repository = Some(context_cache_repository);
        self
    }

    pub fn with_data_port(mut self, data_port: Arc<dyn GetSelectedWorkoutDataPort>) -> Self {
        self.data_port = Some(data_port);
        self
    }
}

impl<Time> WorkoutCoach for LlmWorkoutCoach<Time>
where
    Time: Clock,
{
    fn reply(
        &self,
        user_id: &str,
        summary: &WorkoutSummary,
        user_message: &str,
        athlete_summary_text: Option<&str>,
    ) -> BoxFuture<Result<LlmToolLoopOutput, LlmError>> {
        let llm_chat_port = self.llm_chat_port.clone();
        let config_provider = self.config_provider.clone();
        let training_context_builder = self.training_context_builder.clone();
        let context_cache_repository = self.context_cache_repository.clone();
        let data_port = self.data_port.clone();
        let clock = self.clock.clone();
        let user_id = user_id.to_string();
        let summary = summary.clone();
        let user_message = user_message.to_string();
        let athlete_summary_text = athlete_summary_text.map(str::to_string);

        Box::pin(async move {
            let config = config_provider.get_config(&user_id).await?;
            tracing::info!(
                user_id = %user_id,
                provider = %config.provider,
                model = %config.model,
                "selected llm provider for workout summary coach"
            );
            let training_context = training_context_builder
                .build(&user_id, &summary.workout_id)
                .await?;
            let stable_context = build_stable_context(
                &summary,
                &training_context.rendered.stable_context,
                athlete_summary_text.as_deref(),
            );
            let volatile_context =
                build_volatile_context(&training_context.rendered.volatile_context);
            let tool_context = ToolExecutionContext {
                user_id: user_id.clone(),
                training_context: training_context.context.clone(),
                today: current_date_string(&clock),
                data_port,
                planned_workout_update_port: None,
            };
            let system_prompt = with_tool_prompt_guidance(
                &workout_coach_system_prompt(),
                ToolScope::WorkoutSummaryChat,
                &config.provider,
                &tool_context,
            );
            let conversation = build_conversation(
                summary.messages.as_slice(),
                &summary.provider_transcript,
                &user_message,
            );
            let cache_scope_key = Some(format!("workout-summary:{user_id}:{}", summary.workout_id));
            let context_hash = reusable_context_cache_key(&system_prompt, &stable_context);
            let reusable_cache_id = if config.provider == LlmProvider::Gemini {
                match (
                    context_cache_repository.as_deref(),
                    cache_scope_key.as_deref(),
                ) {
                    (Some(repository), Some(scope_key)) => {
                        match find_reusable_context_cache(ReusableContextCacheLookup {
                            repository: Some(repository),
                            user_id: &user_id,
                            provider: &config.provider,
                            model: &config.model,
                            scope_key: Some(scope_key),
                            context_hash: &context_hash,
                            now_epoch_seconds: clock.now_epoch_seconds(),
                        })
                        .await
                        {
                            Ok(Some(cache)) => {
                                tracing::info!(
                                    user_id = %user_id,
                                    provider = %config.provider,
                                    model = %config.model,
                                    cache_scope_key = %scope_key,
                                    "reusing persisted gemini context cache"
                                );
                                Some(cache.provider_cache_id)
                            }
                            Ok(None) => {
                                tracing::info!(
                                    user_id = %user_id,
                                    provider = %config.provider,
                                    model = %config.model,
                                    cache_scope_key = %scope_key,
                                    "no reusable gemini context cache found"
                                );
                                None
                            }
                            Err(error) => {
                                tracing::warn!(
                                    error = %error,
                                    user_id = %user_id,
                                    provider = %config.provider,
                                    model = %config.model,
                                    cache_scope_key = %scope_key,
                                    "failed to load reusable gemini context cache"
                                );
                                None
                            }
                        }
                    }
                    _ => None,
                }
            } else {
                None
            };
            let request = build_chat_request(LlmChatRequestInput {
                user_id: user_id.clone(),
                system_prompt,
                stable_context,
                volatile_context,
                conversation,
                cache_scope_key: cache_scope_key.clone(),
                cache_key: Some(context_hash.clone()),
                reusable_cache_id,
            });
            tracing::info!(
                user_id = %user_id,
                workout_id = %summary.workout_id,
                provider = %config.provider,
                model = %config.model,
                system_prompt_chars = request.system_prompt.chars().count(),
                stable_context_chars = request.stable_context.chars().count(),
                volatile_context_chars = request.volatile_context.chars().count(),
                conversation_messages = request.conversation.len(),
                "prepared workout summary llm request"
            );

            let response = run_tool_loop(
                llm_chat_port.clone(),
                config.clone(),
                request,
                ToolScope::WorkoutSummaryChat,
                tool_context,
                None,
            )
            .await?;

            match persist_reusable_context_cache(ReusableContextCacheUpsert {
                repository: context_cache_repository.as_deref(),
                user_id: &user_id,
                provider: &config.provider,
                model: &config.model,
                scope_key: cache_scope_key.as_deref(),
                context_hash: &context_hash,
                provider_cache_id: response.response.cache.provider_cache_id.as_deref(),
                expires_at_epoch_seconds: response.response.cache.cache_expires_at_epoch_seconds,
                now_epoch_seconds: clock.now_epoch_seconds(),
            })
            .await
            {
                Err(error) => {
                    tracing::warn!(error = %error, "failed to persist reusable gemini context cache");
                }
                Ok(Some(_)) => {
                    tracing::info!(
                        user_id = %user_id,
                        provider = %config.provider,
                        model = %config.model,
                        "persisted reusable gemini context cache"
                    );
                }
                Ok(None) => {}
            }

            Ok(response)
        })
    }
}

const WORKOUT_COACH_SYSTEM_PROMPT_BASE: &str = "You are an AI cycling coach helping an athlete reflect on one completed workout. Use the packed training context as factual background. Be direct, adult, and concise. Do not flatter, hedge, or act like a yes-man. Challenge weak reasoning when the context does not support it. Keep the conversation focused and practical rather than digressive. In your first reply after a workout, ask all follow-up questions you genuinely need at once instead of stretching them across many turns. The athlete should still feel coached, not interrogated. Ask concrete questions about the workout limiter, legs, breathing, fueling, sleep, stress, pain, readiness for the next days, and any plan constraints when relevant. Add other questions only when the workout characteristics clearly justify them. You may also ask about nutrition, race strategy, or the desired direction of the next 14 days when that would materially improve the next plan. For completed interval workouts, judge execution quality primarily from packed workout evidence: bl as intended block structure/targets, pc as executed power pattern, and c5 as supporting cadence evidence. Aggregate metrics like NP, average power, IF, VI, and TSS are secondary context only and are not sufficient proof that interval blocks were or were not executed correctly. Do not conclude poor interval execution just because whole-workout averages were lowered by recovery valleys, coasting, zeros, terrain, or wind. If the packed evidence is insufficient for a confident execution judgment, inspect higher-fidelity data before making a strong claim. When workout tools are available, use them for that fallback. If you already have enough information to generate the plan, say that clearly and tell the athlete to save the summary. Return your final answer as JSON only matching the workout summary coach reply schema. The summary may use markdown. Questions may be an empty array when you are ready. Do not output any text outside the JSON object. Do not invent details beyond the provided context.";

fn build_stable_context(
    summary: &WorkoutSummary,
    packed_training_context: &str,
    athlete_summary_text: Option<&str>,
) -> String {
    let mut context = format!(
        "workout_summary={{\"workoutId\":\"{}\",\"rpe\":{}}}",
        summary.workout_id,
        summary
            .rpe
            .map(|value| value.to_string())
            .unwrap_or_else(|| "null".to_string()),
    );

    if let Some(summary_text) = athlete_summary_text.filter(|value| !value.trim().is_empty()) {
        context.push_str(&format!("\nathlete_summary_text={summary_text}"));
    }

    context.push_str(&format!(
        "\ntraining_context_stable={packed_training_context}"
    ));
    context
}

fn build_volatile_context(packed_training_context: &str) -> String {
    format!("training_context_volatile={packed_training_context}")
}

fn workout_coach_system_prompt() -> String {
    format!(
        "{WORKOUT_COACH_SYSTEM_PROMPT_BASE}\nworkout_summary_coach_reply_schema={}\n{PACKED_TRAINING_CONTEXT_LEGEND}",
        workout_summary_coach_reply_json_schema()
    )
}

fn build_conversation(
    messages: &[crate::domain::workout_summary::ConversationMessage],
    provider_transcript: &[LlmChatMessage],
    user_message: &str,
) -> Vec<LlmChatMessage> {
    let conversation = messages
        .iter()
        .filter_map(|message| match message.role {
            crate::domain::workout_summary::MessageRole::User => Some(LlmChatMessage {
                role: LlmMessageRole::User,
                content: message.content.clone(),
                tool_calls: Vec::new(),
                tool_call_id: None,
                reasoning_content: None,
            }),
            crate::domain::workout_summary::MessageRole::Coach => Some(LlmChatMessage {
                role: LlmMessageRole::Assistant,
                content: message.content.clone(),
                tool_calls: Vec::new(),
                tool_call_id: None,
                reasoning_content: None,
            }),
            crate::domain::workout_summary::MessageRole::Tool => None,
        })
        .collect::<Vec<_>>();

    let mut rebuilt =
        rebuild_conversation_with_provider_transcript(conversation, provider_transcript);

    if let Some(last) = rebuilt.last_mut() {
        if last.role == LlmMessageRole::User {
            last.content = user_message.to_string();
            return rebuilt;
        }
    }

    rebuilt.push(LlmChatMessage::user(user_message));

    rebuilt
}

#[cfg(test)]
mod tests {
    use super::{build_conversation, workout_coach_system_prompt};
    use crate::domain::{
        llm::{LlmChatMessage, LlmMessageRole, LlmToolCall},
        workout_summary::{ConversationMessage, MessageRole, PublicToolCall},
    };

    #[test]
    fn workout_coach_system_prompt_includes_schema_from_domain_contract() {
        let prompt = workout_coach_system_prompt();

        assert!(prompt.contains("workout_summary_coach_reply_schema="));
        assert!(prompt.contains(r#""summary""#));
        assert!(prompt.contains(r#""questions""#));
        assert!(prompt.contains(r#""freeTextLabel""#));
        assert!(prompt.contains(r#""additionalProperties": false"#));
        assert!(prompt.contains(
            "For completed interval workouts, judge execution quality primarily from packed workout evidence"
        ));
        assert!(prompt.contains("bl as intended block structure/targets"));
        assert!(prompt.contains("pc as executed power pattern"));
        assert!(prompt.contains("c5 as supporting cadence evidence"));
        assert!(prompt.contains(
            "Aggregate metrics like NP, average power, IF, VI, and TSS are secondary context only"
        ));
        assert!(prompt.contains(
            "are not sufficient proof that interval blocks were or were not executed correctly"
        ));
        assert!(prompt.contains(
            "Do not conclude poor interval execution just because whole-workout averages were lowered by recovery valleys, coasting, zeros, terrain, or wind"
        ));
        assert!(prompt.contains("When workout tools are available, use them for that fallback"));
        assert!(!prompt.contains(
            "If the packed evidence is insufficient for a confident execution judgment, use the available workout tools to inspect higher-fidelity data before making a strong claim"
        ));
    }

    #[test]
    fn build_conversation_replays_last_hidden_assistant_tool_calls() {
        let conversation = build_conversation(
            &[
                ConversationMessage {
                    id: "user-1".to_string(),
                    role: MessageRole::User,
                    content: "Need feedback".to_string(),
                    tool_call: None,
                    questions: Vec::new(),
                    created_at_epoch_seconds: 1,
                },
                ConversationMessage {
                    id: "tool-1".to_string(),
                    role: MessageRole::Tool,
                    content: "Tool call: lookupWorkout".to_string(),
                    tool_call: Some(PublicToolCall {
                        id: "tool-1".to_string(),
                        name: "lookupWorkout".to_string(),
                        arguments_json: r#"{\"workoutId\":\"workout-1\"}"#.to_string(),
                        arguments_preview: None,
                    }),
                    questions: Vec::new(),
                    created_at_epoch_seconds: 2,
                },
                ConversationMessage {
                    id: "coach-1".to_string(),
                    role: MessageRole::Coach,
                    content: "Coach reply".to_string(),
                    tool_call: None,
                    questions: Vec::new(),
                    created_at_epoch_seconds: 3,
                },
            ],
            &[
                LlmChatMessage::assistant_with_tool_calls(
                    "Coach reply",
                    vec![LlmToolCall {
                        id: "tool-1".to_string(),
                        name: "lookupWorkout".to_string(),
                        arguments_json: r#"{\"workoutId\":\"workout-1\"}"#.to_string(),
                    }],
                ),
                LlmChatMessage::tool("tool-1", "Workout lookup result"),
            ],
            "What about tomorrow?",
        );

        assert_eq!(conversation.len(), 4);
        assert_eq!(conversation[1].role, LlmMessageRole::Assistant);
        assert_eq!(conversation[1].tool_calls.len(), 1);
        assert_eq!(conversation[1].tool_calls[0].id, "tool-1");
        assert_eq!(conversation[2].role, LlmMessageRole::Tool);
        assert_eq!(conversation[2].tool_call_id.as_deref(), Some("tool-1"));
        assert_eq!(conversation[3].role, LlmMessageRole::User);
        assert_eq!(conversation[3].content, "What about tomorrow?");
    }

    #[test]
    fn build_conversation_replays_hidden_assistant_turns_by_position() {
        let conversation = build_conversation(
            &[
                ConversationMessage {
                    id: "user-1".to_string(),
                    role: MessageRole::User,
                    content: "First question".to_string(),
                    tool_call: None,
                    questions: Vec::new(),
                    created_at_epoch_seconds: 1,
                },
                ConversationMessage {
                    id: "coach-1".to_string(),
                    role: MessageRole::Coach,
                    content: "First answer".to_string(),
                    tool_call: None,
                    questions: Vec::new(),
                    created_at_epoch_seconds: 2,
                },
                ConversationMessage {
                    id: "user-2".to_string(),
                    role: MessageRole::User,
                    content: "Second question".to_string(),
                    tool_call: None,
                    questions: Vec::new(),
                    created_at_epoch_seconds: 3,
                },
                ConversationMessage {
                    id: "coach-2".to_string(),
                    role: MessageRole::Coach,
                    content: "Second answer".to_string(),
                    tool_call: None,
                    questions: Vec::new(),
                    created_at_epoch_seconds: 4,
                },
            ],
            &[
                LlmChatMessage::assistant_with_tool_calls(
                    "First answer\n",
                    vec![LlmToolCall {
                        id: "tool-1".to_string(),
                        name: "lookupOne".to_string(),
                        arguments_json: "{}".to_string(),
                    }],
                ),
                LlmChatMessage::tool("tool-1", "first result"),
                LlmChatMessage::assistant_with_tool_calls(
                    "Second answer\n",
                    vec![LlmToolCall {
                        id: "tool-2".to_string(),
                        name: "lookupTwo".to_string(),
                        arguments_json: "{}".to_string(),
                    }],
                ),
                LlmChatMessage::tool("tool-2", "second result"),
            ],
            "Third question",
        );

        assert_eq!(conversation[1].tool_calls[0].id, "tool-1");
        assert_eq!(conversation[2].tool_call_id.as_deref(), Some("tool-1"));
        assert_eq!(conversation[4].tool_calls[0].id, "tool-2");
        assert_eq!(conversation[5].tool_call_id.as_deref(), Some("tool-2"));
        assert_eq!(conversation[6].content, "Third question");
    }
}
