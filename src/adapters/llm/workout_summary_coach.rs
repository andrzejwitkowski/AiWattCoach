use std::sync::Arc;

use crate::domain::{
    identity::Clock,
    llm::{
        current_date_string, find_reusable_context_cache, persist_reusable_context_cache,
        reusable_context_cache_key, BoxFuture, LlmChatPort, LlmContextCacheRepository, LlmError,
        LlmProvider, ReusableContextCacheLookup, ReusableContextCacheUpsert, UserLlmConfigProvider,
    },
    llm_tools::{run_tool_loop, GetSelectedWorkoutDataPort, LlmToolLoopOutput, ToolScope},
    meso_cycle::MesoCycleProjectionRepository,
    training_context::TrainingContextBuilder,
    workout_summary::{
        assemble_workout_summary_coach_request, try_load_meso_roadmap_stable_context, WorkoutCoach,
        WorkoutSummary, WorkoutSummaryCoachPromptInput,
    },
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
    meso_projection_repository: Option<Arc<dyn MesoCycleProjectionRepository>>,
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
            meso_projection_repository: None,
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

    pub fn with_meso_projection_repository(
        mut self,
        meso_projection_repository: Arc<dyn MesoCycleProjectionRepository>,
    ) -> Self {
        self.meso_projection_repository = Some(meso_projection_repository);
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
        let meso_projection_repository = self.meso_projection_repository.clone();
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
            let meso_roadmap_stable_context =
                if let Some(repository) = meso_projection_repository.as_deref() {
                    try_load_meso_roadmap_stable_context(repository, &user_id).await
                } else {
                    None
                };
            let cache_scope_key = Some(format!("workout-summary:{user_id}:{}", summary.workout_id));
            let context_hash = reusable_context_cache_key(
                &crate::domain::workout_summary::workout_coach_system_prompt(),
                &crate::domain::workout_summary::build_stable_context(
                    &summary,
                    &training_context.focus_date,
                    &training_context.rendered.stable_context,
                    athlete_summary_text.as_deref(),
                    meso_roadmap_stable_context.as_deref(),
                ),
            );
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
            let today = current_date_string(&clock);
            let tool_context = crate::domain::llm_tools::ToolExecutionContext {
                user_id: user_id.clone(),
                training_context: training_context.context.clone(),
                today: today.clone(),
                data_port: data_port.clone(),
                planned_workout_update_port: None,
            };
            let mut request =
                assemble_workout_summary_coach_request(WorkoutSummaryCoachPromptInput {
                    user_id: user_id.clone(),
                    config: config.clone(),
                    summary: summary.clone(),
                    training_context,
                    user_message,
                    athlete_summary_text,
                    conversation_epoch_seconds: clock.now_epoch_seconds(),
                    today,
                    data_port,
                    reusable_cache_id,
                    meso_roadmap_stable_context,
                });
            request.cache_key = Some(context_hash.clone());
            request.cache_scope_key = cache_scope_key.clone();
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

#[cfg(test)]
mod tests {
    use crate::domain::workout_summary::{
        build_conversation, build_stable_context, build_volatile_context,
        workout_coach_system_prompt,
    };
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
            "For completed interval workouts, judge interval execution primarily from bl"
        ));
        assert!(prompt.contains("ps (executed power segments)"));
        assert!(prompt.contains("ps=executed power segments"));
        assert!(
            prompt.contains("cs ([minRPM,maxRPM,durationSec]) as supporting cadence evidence only")
        );
        assert!(prompt.contains("Never tell the athlete they have free time, vacation, or a rest block unless prd confirms it"));
        assert!(!prompt.contains("p3 as executed power"));
        assert!(!prompt.contains("p3=power watts"));
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
        assert!(prompt.contains("Seiler polarized training model"));
        assert!(prompt.contains("Do not use this block to justify extra follow-up questions"));
        assert!(
            prompt.contains("Use the provided selected workout date as the active workout context")
        );
        assert!(!prompt.contains(
            "If the packed evidence is insufficient for a confident execution judgment, use the available workout tools to inspect higher-fidelity data before making a strong claim"
        ));
    }

    #[test]
    fn build_stable_context_includes_selected_workout_date() {
        let summary = crate::domain::workout_summary::WorkoutSummary::new(
            "summary-1".to_string(),
            "user-1".to_string(),
            "workout-1".to_string(),
            1,
        );

        let context = build_stable_context(&summary, "2026-05-29", "{}", None, None);

        assert!(
            context.contains(r#"selected_workout={"workoutId":"workout-1","date":"2026-05-29"}"#)
        );
        assert!(context.contains(
            "current_workout_context=You are discussing the completed workout from 2026-05-29."
        ));
    }

    #[test]
    fn build_stable_context_wraps_athlete_summary_with_guidance() {
        let summary = crate::domain::workout_summary::WorkoutSummary::new(
            "summary-1".to_string(),
            "user-1".to_string(),
            "workout-1".to_string(),
            1,
        );

        let context = build_stable_context(
            &summary,
            "2026-05-29",
            "{}",
            Some("Durable athlete with strong threshold repeatability."),
            None,
        );

        assert!(context.contains("athlete_summary_guidance="));
        assert!(context.contains("NOT calendar truth"));
        assert!(context
            .contains("athlete_summary_text=Durable athlete with strong threshold repeatability."));
    }

    #[test]
    fn build_volatile_context_includes_conversation_timing() {
        let context = build_volatile_context("{}", 1_746_489_600, Some(1_746_490_200));

        assert!(context.contains("currentConversationDatetime"));
        assert!(context.contains("2025-05-06T00:00:00+00:00"));
        assert!(context.contains("latest_user_message_datetime=2025-05-06T00:10:00+00:00"));
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
            4,
        );

        assert_eq!(conversation.len(), 4);
        assert_eq!(conversation[1].role, LlmMessageRole::Assistant);
        assert_eq!(conversation[1].tool_calls.len(), 1);
        assert_eq!(conversation[1].tool_calls[0].id, "tool-1");
        assert_eq!(conversation[2].role, LlmMessageRole::Tool);
        assert_eq!(conversation[2].tool_call_id.as_deref(), Some("tool-1"));
        assert_eq!(conversation[3].role, LlmMessageRole::User);
        assert_eq!(
            conversation[3].content,
            "[sent_at=1970-01-01T00:00:01+00:00]\nWhat about tomorrow?"
        );
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
            5,
        );

        assert_eq!(conversation[1].tool_calls[0].id, "tool-1");
        assert_eq!(conversation[2].tool_call_id.as_deref(), Some("tool-1"));
        assert_eq!(conversation[4].tool_calls[0].id, "tool-2");
        assert_eq!(conversation[5].tool_call_id.as_deref(), Some("tool-2"));
        assert_eq!(
            conversation[6].content,
            "[sent_at=1970-01-01T00:00:03+00:00]\nThird question"
        );
    }

    #[test]
    fn build_conversation_uses_fallback_timestamp_when_history_has_no_user_message() {
        let conversation = build_conversation(&[], &[], "Fresh question", 1_746_489_600);

        assert_eq!(conversation.len(), 1);
        assert_eq!(
            conversation[0].content,
            "[sent_at=2025-05-06T00:00:00+00:00]\nFresh question"
        );
    }
}
