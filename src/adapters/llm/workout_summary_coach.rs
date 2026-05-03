use std::sync::Arc;

use super::context_prelude::PACKED_TRAINING_CONTEXT_LEGEND;

use crate::domain::{
    identity::Clock,
    llm::{
        approximate_token_budget_for_model, hash_text, BoxFuture, LlmChatMessage, LlmChatPort,
        LlmChatRequest, LlmChatResponse, LlmContextCache, LlmContextCacheRepository, LlmError,
        LlmMessageRole, LlmProvider, UserLlmConfigProvider,
    },
    training_context::TrainingContextBuilder,
    workout_summary::{WorkoutCoach, WorkoutSummary},
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
    ) -> BoxFuture<Result<LlmChatResponse, LlmError>> {
        let llm_chat_port = self.llm_chat_port.clone();
        let config_provider = self.config_provider.clone();
        let training_context_builder = self.training_context_builder.clone();
        let context_cache_repository = self.context_cache_repository.clone();
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
            let system_prompt = workout_coach_system_prompt();
            let conversation = build_conversation(
                summary.messages.as_slice(),
                &summary.hidden_transcript,
                &user_message,
            );
            let estimated_request_tokens = approximate_token_usage(&stable_context)
                + approximate_token_usage(&volatile_context)
                + approximate_token_usage(&system_prompt)
                + conversation
                    .iter()
                    .map(|message| {
                        approximate_token_usage(&message.content)
                            + message
                                .tool_calls
                                .iter()
                                .map(|tool| {
                                    approximate_token_usage(&tool.name)
                                        + approximate_token_usage(&tool.arguments_json)
                                })
                                .sum::<usize>()
                    })
                    .sum::<usize>();
            let token_budget = approximate_token_budget_for_model(&config.model);
            if estimated_request_tokens > token_budget {
                return Err(LlmError::ContextTooLarge(format!(
                    "packed training context exceeds model limits: estimated {estimated_request_tokens} tokens exceeds {token_budget} token budget"
                )));
            }
            let cache_scope_key = Some(format!("workout-summary:{user_id}:{}", summary.workout_id));
            let context_hash = hash_text(&format!("{system_prompt}\n{stable_context}"));
            let reusable_cache_id = if config.provider == LlmProvider::Gemini {
                match (&context_cache_repository, cache_scope_key.as_deref()) {
                    (Some(repository), Some(scope_key)) => {
                        let reusable = match repository
                            .find_reusable(
                                &user_id,
                                &config.provider,
                                &config.model,
                                scope_key,
                                &context_hash,
                                clock.now_epoch_seconds(),
                            )
                            .await
                        {
                            Ok(reusable) => reusable,
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
                        };
                        if let Some(cache) = reusable {
                            tracing::info!(
                                user_id = %user_id,
                                provider = %config.provider,
                                model = %config.model,
                                cache_scope_key = %scope_key,
                                "reusing persisted gemini context cache"
                            );
                            Some(cache.provider_cache_id)
                        } else {
                            tracing::info!(
                                user_id = %user_id,
                                provider = %config.provider,
                                model = %config.model,
                                cache_scope_key = %scope_key,
                                "no reusable gemini context cache found"
                            );
                            None
                        }
                    }
                    _ => None,
                }
            } else {
                None
            };
            let request = LlmChatRequest {
                user_id: user_id.clone(),
                system_prompt,
                stable_context,
                volatile_context,
                conversation,
                cache_scope_key: cache_scope_key.clone(),
                cache_key: Some(context_hash.clone()),
                reusable_cache_id,
                tools: Vec::new(),
                tool_choice: crate::domain::llm::LlmToolChoice::None,
            };

            tracing::info!(
                user_id = %user_id,
                workout_id = %summary.workout_id,
                provider = %config.provider,
                model = %config.model,
                estimated_request_tokens,
                system_prompt_chars = request.system_prompt.chars().count(),
                stable_context_chars = request.stable_context.chars().count(),
                volatile_context_chars = request.volatile_context.chars().count(),
                conversation_messages = request.conversation.len(),
                "prepared workout summary llm request"
            );

            let response = llm_chat_port.chat(config.clone(), request).await?;

            if config.provider == LlmProvider::Gemini {
                if let (Some(repository), Some(scope_key), Some(provider_cache_id)) = (
                    context_cache_repository,
                    cache_scope_key,
                    response.cache.provider_cache_id.clone(),
                ) {
                    if let Err(error) = repository
                        .upsert(LlmContextCache {
                            user_id: user_id.clone(),
                            provider: config.provider.clone(),
                            model: config.model.clone(),
                            scope_key,
                            context_hash,
                            provider_cache_id,
                            expires_at_epoch_seconds: response.cache.cache_expires_at_epoch_seconds,
                            created_at_epoch_seconds: clock.now_epoch_seconds(),
                            updated_at_epoch_seconds: clock.now_epoch_seconds(),
                        })
                        .await
                    {
                        tracing::warn!(error = %error, "failed to persist reusable gemini context cache");
                    } else {
                        tracing::info!(
                            user_id = %user_id,
                            provider = %config.provider,
                            model = %config.model,
                            "persisted reusable gemini context cache"
                        );
                    }
                }
            }

            Ok(response)
        })
    }
}

const WORKOUT_COACH_SYSTEM_PROMPT_BASE: &str = "You are an AI cycling coach helping an athlete reflect on one completed workout. Use the packed training context as factual background. Be direct, adult, and concise. Do not flatter, hedge, or act like a yes-man. Challenge weak reasoning when the context does not support it. Ask only one focused follow-up question when genuinely needed, gather the minimum information required to adjust the next plan, and move the conversation toward being ready to regenerate workouts. Do not invent details beyond the provided context.";

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

fn approximate_token_usage(value: &str) -> usize {
    value.chars().count().div_ceil(3)
}

fn workout_coach_system_prompt() -> String {
    format!("{WORKOUT_COACH_SYSTEM_PROMPT_BASE} {PACKED_TRAINING_CONTEXT_LEGEND}")
}

fn build_conversation(
    messages: &[crate::domain::workout_summary::ConversationMessage],
    hidden_transcript: &[LlmChatMessage],
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
            }),
            crate::domain::workout_summary::MessageRole::Coach => Some(LlmChatMessage {
                role: LlmMessageRole::Assistant,
                content: message.content.clone(),
                tool_calls: Vec::new(),
                tool_call_id: None,
            }),
            crate::domain::workout_summary::MessageRole::Tool => None,
        })
        .collect::<Vec<_>>();

    let mut rebuilt = rebuild_conversation_with_hidden_transcript(conversation, hidden_transcript);

    if let Some(last) = rebuilt.last_mut() {
        if last.role == LlmMessageRole::User {
            last.content = user_message.to_string();
            return rebuilt;
        }
    }

    rebuilt.push(LlmChatMessage::user(user_message));

    rebuilt
}

fn rebuild_conversation_with_hidden_transcript(
    conversation: Vec<LlmChatMessage>,
    hidden_transcript: &[LlmChatMessage],
) -> Vec<LlmChatMessage> {
    let hidden_assistants = hidden_transcript
        .iter()
        .filter(|message| message.role == LlmMessageRole::Assistant)
        .cloned()
        .collect::<Vec<_>>();
    let mut hidden_assistant_index = 0;
    let mut rebuilt = Vec::with_capacity(conversation.len() + hidden_transcript.len());

    for message in conversation {
        if message.role != LlmMessageRole::Assistant {
            rebuilt.push(message);
            continue;
        }

        let assistant = hidden_assistants
            .get(hidden_assistant_index)
            .cloned()
            .unwrap_or(message);
        hidden_assistant_index += 1;
        rebuilt.push(assistant.clone());
        rebuilt.extend(hidden_tool_messages_for_assistant(
            hidden_transcript,
            &assistant,
        ));
    }

    rebuilt
}

fn hidden_tool_messages_for_assistant(
    hidden_transcript: &[LlmChatMessage],
    assistant: &LlmChatMessage,
) -> Vec<LlmChatMessage> {
    assistant
        .tool_calls
        .iter()
        .filter_map(|tool_call| {
            hidden_transcript
                .iter()
                .find(|message| {
                    message.role == LlmMessageRole::Tool
                        && message.tool_call_id.as_deref() == Some(tool_call.id.as_str())
                })
                .cloned()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::build_conversation;
    use crate::domain::{
        llm::{LlmChatMessage, LlmMessageRole, LlmToolCall},
        workout_summary::{ConversationMessage, MessageRole, PublicToolCall},
    };

    #[test]
    fn build_conversation_replays_last_hidden_assistant_tool_calls() {
        let conversation = build_conversation(
            &[
                ConversationMessage {
                    id: "user-1".to_string(),
                    role: MessageRole::User,
                    content: "Need feedback".to_string(),
                    tool_call: None,
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
                    }),
                    created_at_epoch_seconds: 2,
                },
                ConversationMessage {
                    id: "coach-1".to_string(),
                    role: MessageRole::Coach,
                    content: "Coach reply".to_string(),
                    tool_call: None,
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
                    created_at_epoch_seconds: 1,
                },
                ConversationMessage {
                    id: "coach-1".to_string(),
                    role: MessageRole::Coach,
                    content: "First answer".to_string(),
                    tool_call: None,
                    created_at_epoch_seconds: 2,
                },
                ConversationMessage {
                    id: "user-2".to_string(),
                    role: MessageRole::User,
                    content: "Second question".to_string(),
                    tool_call: None,
                    created_at_epoch_seconds: 3,
                },
                ConversationMessage {
                    id: "coach-2".to_string(),
                    role: MessageRole::Coach,
                    content: "Second answer".to_string(),
                    tool_call: None,
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
