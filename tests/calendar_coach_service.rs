use std::sync::{Arc, Mutex};

use aiwattcoach::domain::{
    coach_conversation::{
        CoachConversation, CoachConversationError, CoachConversationMessage,
        CoachConversationMessageRepository, CoachConversationMessageRole,
        CoachConversationReplyClaimResult, CoachConversationReplyOperation,
        CoachConversationReplyOperationRepository, CoachConversationRepository,
        CoachConversationStatus, CoachConversationUseCases, SharedCoachConversationService,
    },
    identity::{Clock, IdGenerator},
    llm::{
        BoxFuture as LlmBoxFuture, LlmCacheUsage, LlmChatPort, LlmChatRequest, LlmChatResponse,
        LlmContextCache, LlmContextCacheRepository, LlmError, LlmProvider, LlmProviderConfig,
        LlmTokenUsage, UserLlmConfigProvider,
    },
    settings::{
        AiAgentsConfig, AnalysisOptions, AvailabilityDay, AvailabilitySettings, CyclingSettings,
        IntervalsConfig, SettingsError, UserSettings, UserSettingsUseCases, WahooConfig, Weekday,
    },
    training_context::{
        RenderedTrainingContext, TrainingContext, TrainingContextBuildResult,
        TrainingContextBuilder,
    },
};

#[derive(Clone)]
struct FixedClock;

impl Clock for FixedClock {
    fn now_epoch_seconds(&self) -> i64 {
        1_700_000_000
    }
}

#[derive(Clone)]
struct TestIds;

impl IdGenerator for TestIds {
    fn new_id(&self, prefix: &str) -> String {
        format!("{prefix}-id")
    }
}

#[derive(Clone, Default)]
struct InMemoryConversationRepository {
    conversation: Arc<Mutex<Option<CoachConversation>>>,
}

impl CoachConversationRepository for InMemoryConversationRepository {
    fn find_active_by_user_id_and_surface(
        &self,
        user_id: &str,
        surface: &aiwattcoach::domain::coach_conversation::CoachConversationSurface,
    ) -> aiwattcoach::domain::coach_conversation::BoxFuture<
        Result<Option<CoachConversation>, CoachConversationError>,
    > {
        let stored = self.conversation.lock().unwrap().clone();
        let user_id = user_id.to_string();
        let surface = surface.clone();
        Box::pin(async move {
            Ok(stored.filter(|conversation| {
                conversation.user_id == user_id
                    && conversation.surface == surface
                    && conversation.status == CoachConversationStatus::Active
            }))
        })
    }

    fn find_by_user_id_and_conversation_id(
        &self,
        user_id: &str,
        conversation_id: &str,
    ) -> aiwattcoach::domain::coach_conversation::BoxFuture<
        Result<Option<CoachConversation>, CoachConversationError>,
    > {
        let stored = self.conversation.lock().unwrap().clone();
        let user_id = user_id.to_string();
        let conversation_id = conversation_id.to_string();
        Box::pin(async move {
            Ok(stored.filter(|conversation| {
                conversation.user_id == user_id && conversation.conversation_id == conversation_id
            }))
        })
    }

    fn create(
        &self,
        conversation: CoachConversation,
    ) -> aiwattcoach::domain::coach_conversation::BoxFuture<
        Result<CoachConversation, CoachConversationError>,
    > {
        let state = self.conversation.clone();
        Box::pin(async move {
            *state.lock().unwrap() = Some(conversation.clone());
            Ok(conversation)
        })
    }

    fn update_status(
        &self,
        user_id: &str,
        conversation_id: &str,
        status: CoachConversationStatus,
        updated_at_epoch_seconds: i64,
    ) -> aiwattcoach::domain::coach_conversation::BoxFuture<Result<(), CoachConversationError>>
    {
        let state = self.conversation.clone();
        let user_id = user_id.to_string();
        let conversation_id = conversation_id.to_string();
        Box::pin(async move {
            let mut guard = state.lock().unwrap();
            let Some(existing) = guard.as_mut() else {
                return Err(CoachConversationError::NotFound);
            };
            if existing.user_id != user_id || existing.conversation_id != conversation_id {
                return Err(CoachConversationError::NotFound);
            }
            existing.status = status;
            existing.updated_at_epoch_seconds = updated_at_epoch_seconds;
            Ok(())
        })
    }

    fn touch_updated_at(
        &self,
        user_id: &str,
        conversation_id: &str,
        updated_at_epoch_seconds: i64,
    ) -> aiwattcoach::domain::coach_conversation::BoxFuture<Result<(), CoachConversationError>>
    {
        let state = self.conversation.clone();
        let user_id = user_id.to_string();
        let conversation_id = conversation_id.to_string();
        Box::pin(async move {
            let mut guard = state.lock().unwrap();
            let Some(existing) = guard.as_mut() else {
                return Err(CoachConversationError::NotFound);
            };
            if existing.user_id != user_id || existing.conversation_id != conversation_id {
                return Err(CoachConversationError::NotFound);
            }
            existing.updated_at_epoch_seconds = updated_at_epoch_seconds;
            Ok(())
        })
    }
}

#[derive(Clone, Default)]
struct InMemoryMessageRepository {
    messages: Arc<Mutex<Vec<CoachConversationMessage>>>,
}

impl CoachConversationMessageRepository for InMemoryMessageRepository {
    fn list_by_user_id_and_conversation_id(
        &self,
        user_id: &str,
        conversation_id: &str,
    ) -> aiwattcoach::domain::coach_conversation::BoxFuture<
        Result<Vec<CoachConversationMessage>, CoachConversationError>,
    > {
        let messages = self.messages.lock().unwrap().clone();
        let user_id = user_id.to_string();
        let conversation_id = conversation_id.to_string();
        Box::pin(async move {
            Ok(messages
                .into_iter()
                .filter(|message| {
                    message.user_id == user_id && message.conversation_id == conversation_id
                })
                .collect())
        })
    }

    fn append(
        &self,
        message: CoachConversationMessage,
    ) -> aiwattcoach::domain::coach_conversation::BoxFuture<
        Result<CoachConversationMessage, CoachConversationError>,
    > {
        let state = self.messages.clone();
        Box::pin(async move {
            state.lock().unwrap().push(message.clone());
            Ok(message)
        })
    }

    fn find_by_user_id_and_conversation_id_and_message_id(
        &self,
        user_id: &str,
        conversation_id: &str,
        message_id: &str,
    ) -> aiwattcoach::domain::coach_conversation::BoxFuture<
        Result<Option<CoachConversationMessage>, CoachConversationError>,
    > {
        let messages = self.messages.lock().unwrap().clone();
        let user_id = user_id.to_string();
        let conversation_id = conversation_id.to_string();
        let message_id = message_id.to_string();
        Box::pin(async move {
            Ok(messages.into_iter().find(|message| {
                message.user_id == user_id
                    && message.conversation_id == conversation_id
                    && message.id == message_id
            }))
        })
    }
}

#[derive(Clone, Default)]
struct InMemoryReplyOperationRepository {
    operation: Arc<Mutex<Option<CoachConversationReplyOperation>>>,
}

impl CoachConversationReplyOperationRepository for InMemoryReplyOperationRepository {
    fn find_by_user_message_id(
        &self,
        user_id: &str,
        conversation_id: &str,
        user_message_id: &str,
    ) -> aiwattcoach::domain::coach_conversation::BoxFuture<
        Result<Option<CoachConversationReplyOperation>, CoachConversationError>,
    > {
        let stored = self.operation.lock().unwrap().clone();
        let user_id = user_id.to_string();
        let conversation_id = conversation_id.to_string();
        let user_message_id = user_message_id.to_string();
        Box::pin(async move {
            Ok(stored.filter(|operation| {
                operation.user_id == user_id
                    && operation.conversation_id == conversation_id
                    && operation.user_message_id == user_message_id
            }))
        })
    }

    fn claim_pending(
        &self,
        operation: CoachConversationReplyOperation,
        _stale_before_epoch_seconds: i64,
    ) -> aiwattcoach::domain::coach_conversation::BoxFuture<
        Result<CoachConversationReplyClaimResult, CoachConversationError>,
    > {
        let state = self.operation.clone();
        Box::pin(async move {
            let mut guard = state.lock().unwrap();
            if let Some(existing) = guard.clone() {
                return Ok(CoachConversationReplyClaimResult::Existing(existing));
            }
            *guard = Some(operation.clone());
            Ok(CoachConversationReplyClaimResult::Claimed(operation))
        })
    }

    fn upsert(
        &self,
        operation: CoachConversationReplyOperation,
    ) -> aiwattcoach::domain::coach_conversation::BoxFuture<
        Result<CoachConversationReplyOperation, CoachConversationError>,
    > {
        let state = self.operation.clone();
        Box::pin(async move {
            *state.lock().unwrap() = Some(operation.clone());
            Ok(operation)
        })
    }
}

#[derive(Clone, Default)]
struct RecordingLlmChatPort {
    requests: Arc<Mutex<Vec<LlmChatRequest>>>,
}

impl RecordingLlmChatPort {
    fn requests(&self) -> Vec<LlmChatRequest> {
        self.requests.lock().unwrap().clone()
    }
}

impl LlmChatPort for RecordingLlmChatPort {
    fn chat(
        &self,
        _config: LlmProviderConfig,
        request: LlmChatRequest,
    ) -> LlmBoxFuture<Result<LlmChatResponse, LlmError>> {
        let state = self.requests.clone();
        Box::pin(async move {
            state.lock().unwrap().push(request);
            Ok(LlmChatResponse {
                provider: LlmProvider::OpenAi,
                model: "gpt-5".to_string(),
                message: "Coach reply".to_string(),
                provider_request_id: Some("req-1".to_string()),
                usage: LlmTokenUsage::default(),
                cache: LlmCacheUsage::default(),
            })
        })
    }
}

#[derive(Clone)]
struct StaticLlmConfigProvider;

impl UserLlmConfigProvider for StaticLlmConfigProvider {
    fn get_config(&self, _user_id: &str) -> LlmBoxFuture<Result<LlmProviderConfig, LlmError>> {
        Box::pin(async {
            Ok(LlmProviderConfig {
                provider: LlmProvider::OpenAi,
                model: "gpt-5".to_string(),
                api_key: "test-key".to_string(),
            })
        })
    }
}

#[derive(Clone, Default)]
struct RecordingTrainingContextBuilder {
    build_calls: Arc<Mutex<Vec<String>>>,
    calendar_overview_calls: Arc<Mutex<Vec<String>>>,
    athlete_summary_calls: Arc<Mutex<Vec<String>>>,
}

impl RecordingTrainingContextBuilder {
    fn build_calls(&self) -> Vec<String> {
        self.build_calls.lock().unwrap().clone()
    }

    fn calendar_overview_calls(&self) -> Vec<String> {
        self.calendar_overview_calls.lock().unwrap().clone()
    }

    fn athlete_summary_calls(&self) -> Vec<String> {
        self.athlete_summary_calls.lock().unwrap().clone()
    }

    fn result_for(focus_id: Option<String>, focus_kind: &str) -> TrainingContextBuildResult {
        TrainingContextBuildResult {
            context: TrainingContext {
                generated_at_epoch_seconds: 1_700_000_000,
                focus_workout_id: focus_id,
                focus_kind: focus_kind.to_string(),
                intervals_status: Default::default(),
                profile: Default::default(),
                races: Vec::new(),
                future_events: Vec::new(),
                history: Default::default(),
                recent_days: Vec::new(),
                upcoming_days: Vec::new(),
                projected_days: Vec::new(),
            },
            rendered: RenderedTrainingContext {
                stable_context: format!(r#"{{"source":"{focus_kind}"}}"#),
                volatile_context: format!(r#"{{"focus":"{focus_kind}"}}"#),
                approximate_tokens: 10,
            },
        }
    }
}

impl TrainingContextBuilder for RecordingTrainingContextBuilder {
    fn build(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> LlmBoxFuture<Result<TrainingContextBuildResult, LlmError>> {
        let build_calls = self.build_calls.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        Box::pin(async move {
            build_calls
                .lock()
                .unwrap()
                .push(format!("{user_id}:{workout_id}"));
            Ok(Self::result_for(Some(workout_id), "activity"))
        })
    }

    fn build_calendar_overview_context(
        &self,
        user_id: &str,
    ) -> LlmBoxFuture<Result<TrainingContextBuildResult, LlmError>> {
        let calls = self.calendar_overview_calls.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            calls.lock().unwrap().push(user_id);
            Ok(Self::result_for(None, "summary"))
        })
    }

    fn build_athlete_summary_context(
        &self,
        user_id: &str,
    ) -> LlmBoxFuture<Result<TrainingContextBuildResult, LlmError>> {
        let calls = self.athlete_summary_calls.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            calls.lock().unwrap().push(user_id);
            Ok(Self::result_for(None, "summary"))
        })
    }
}

#[derive(Clone)]
struct ConfiguredSettingsService;

impl UserSettingsUseCases for ConfiguredSettingsService {
    fn find_settings(
        &self,
        user_id: &str,
    ) -> aiwattcoach::domain::settings::BoxFuture<Result<Option<UserSettings>, SettingsError>> {
        let user_id = user_id.to_string();
        Box::pin(async move { Ok(Some(configured_settings(&user_id))) })
    }

    fn get_settings(
        &self,
        user_id: &str,
    ) -> aiwattcoach::domain::settings::BoxFuture<Result<UserSettings, SettingsError>> {
        let user_id = user_id.to_string();
        Box::pin(async move { Ok(configured_settings(&user_id)) })
    }

    fn update_ai_agents(
        &self,
        _user_id: &str,
        _ai_agents: AiAgentsConfig,
    ) -> aiwattcoach::domain::settings::BoxFuture<Result<UserSettings, SettingsError>> {
        Box::pin(async { panic!("not used in test") })
    }

    fn update_intervals(
        &self,
        _user_id: &str,
        _intervals: IntervalsConfig,
    ) -> aiwattcoach::domain::settings::BoxFuture<Result<UserSettings, SettingsError>> {
        Box::pin(async { panic!("not used in test") })
    }

    fn update_options(
        &self,
        _user_id: &str,
        _options: AnalysisOptions,
    ) -> aiwattcoach::domain::settings::BoxFuture<Result<UserSettings, SettingsError>> {
        Box::pin(async { panic!("not used in test") })
    }

    fn update_availability(
        &self,
        _user_id: &str,
        _availability: AvailabilitySettings,
    ) -> aiwattcoach::domain::settings::BoxFuture<Result<UserSettings, SettingsError>> {
        Box::pin(async { panic!("not used in test") })
    }

    fn update_cycling(
        &self,
        _user_id: &str,
        _cycling: CyclingSettings,
    ) -> aiwattcoach::domain::settings::BoxFuture<Result<UserSettings, SettingsError>> {
        Box::pin(async { panic!("not used in test") })
    }
}

#[derive(Clone, Default)]
struct NoopContextCacheRepository;

impl LlmContextCacheRepository for NoopContextCacheRepository {
    fn find_reusable(
        &self,
        _user_id: &str,
        _provider: &LlmProvider,
        _model: &str,
        _scope_key: &str,
        _context_hash: &str,
        _now_epoch_seconds: i64,
    ) -> LlmBoxFuture<Result<Option<LlmContextCache>, LlmError>> {
        Box::pin(async { Ok(None) })
    }

    fn upsert(&self, cache: LlmContextCache) -> LlmBoxFuture<Result<LlmContextCache, LlmError>> {
        Box::pin(async move { Ok(cache) })
    }

    fn delete_by_user_id(&self, _user_id: &str) -> LlmBoxFuture<Result<(), LlmError>> {
        Box::pin(async { Ok(()) })
    }
}

fn configured_settings(user_id: &str) -> UserSettings {
    UserSettings {
        user_id: user_id.to_string(),
        ai_agents: AiAgentsConfig::default(),
        intervals: IntervalsConfig::default(),
        wahoo: WahooConfig::default(),
        options: AnalysisOptions::default(),
        availability: AvailabilitySettings {
            configured: true,
            days: vec![
                Weekday::Mon,
                Weekday::Tue,
                Weekday::Wed,
                Weekday::Thu,
                Weekday::Fri,
                Weekday::Sat,
                Weekday::Sun,
            ]
            .into_iter()
            .map(|weekday| AvailabilityDay {
                weekday,
                available: weekday != Weekday::Sun,
                max_duration_minutes: Some(90),
            })
            .collect(),
        },
        cycling: CyclingSettings::default(),
        created_at_epoch_seconds: 1_700_000_000,
        updated_at_epoch_seconds: 1_700_000_000,
    }
}

#[tokio::test]
async fn calendar_coach_generate_reply_uses_calendar_overview_context_and_no_summary_text() {
    let conversations = InMemoryConversationRepository::default();
    let messages = InMemoryMessageRepository::default();
    let reply_operations = InMemoryReplyOperationRepository::default();
    let llm_chat_port = RecordingLlmChatPort::default();
    let training_context_builder = RecordingTrainingContextBuilder::default();

    let service = SharedCoachConversationService::new(
        conversations.clone(),
        messages.clone(),
        reply_operations,
        Arc::new(llm_chat_port.clone()),
        Arc::new(StaticLlmConfigProvider),
        Arc::new(training_context_builder.clone()),
        FixedClock,
        TestIds,
    )
    .with_settings_service(Arc::new(ConfiguredSettingsService))
    .with_context_cache_repository(Arc::new(NoopContextCacheRepository));

    let (conversation, _) = service
        .get_or_create_active_calendar_conversation("user-1")
        .await
        .expect("conversation should be created");
    let persisted = service
        .append_calendar_user_message(
            "user-1",
            &conversation.conversation_id,
            "What should I do today?".to_string(),
        )
        .await
        .expect("user message should persist");

    assert!(!persisted.athlete_summary_may_regenerate_before_reply);

    let reply = service
        .generate_calendar_reply(
            "user-1",
            &conversation.conversation_id,
            persisted.user_message.id.clone(),
        )
        .await
        .expect("reply should be generated");

    assert!(!reply.athlete_summary_was_regenerated);
    assert_eq!(training_context_builder.build_calls(), Vec::<String>::new());
    assert_eq!(
        training_context_builder.calendar_overview_calls(),
        vec!["user-1".to_string()]
    );
    assert_eq!(
        training_context_builder.athlete_summary_calls(),
        Vec::<String>::new()
    );

    let requests = llm_chat_port.requests();
    assert_eq!(requests.len(), 1);
    let request = &requests[0];
    assert!(request
        .stable_context
        .contains("training_context_stable={\"source\":\"summary\"}"));
    assert!(!request.stable_context.contains("athlete_summary_text="));
    assert!(request
        .volatile_context
        .contains("training_context_volatile={\"focus\":\"summary\"}"));
    assert_eq!(request.conversation.len(), 1);
    assert_eq!(request.conversation[0].content, "What should I do today?");
    assert_eq!(
        request.cache_scope_key.as_deref(),
        Some("calendar-coach:user-1:overview")
    );
    assert_eq!(
        reply.coach_message.role,
        CoachConversationMessageRole::Coach
    );
    assert_eq!(reply.coach_message.content, "Coach reply");
}

#[tokio::test]
async fn calendar_coach_send_message_result_keeps_summary_regeneration_hint_false() {
    let service = SharedCoachConversationService::new(
        InMemoryConversationRepository::default(),
        InMemoryMessageRepository::default(),
        InMemoryReplyOperationRepository::default(),
        Arc::new(RecordingLlmChatPort::default()),
        Arc::new(StaticLlmConfigProvider),
        Arc::new(RecordingTrainingContextBuilder::default()),
        FixedClock,
        TestIds,
    )
    .with_settings_service(Arc::new(ConfiguredSettingsService));

    let (conversation, _) = service
        .get_or_create_active_calendar_conversation("user-1")
        .await
        .expect("conversation should be created");
    let persisted = service
        .append_calendar_user_message(
            "user-1",
            &conversation.conversation_id,
            "Check tomorrow".to_string(),
        )
        .await
        .expect("user message should persist");

    assert!(!persisted.athlete_summary_may_regenerate_before_reply);
}
