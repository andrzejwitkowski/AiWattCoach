use std::sync::{Arc, Mutex};

use aiwattcoach::domain::{
    coach_conversation::{
        CoachConversation, CoachConversationError, CoachConversationMessage,
        CoachConversationMessageRepository, CoachConversationMessageRole,
        CoachConversationReplyClaimResult, CoachConversationReplyOperation,
        CoachConversationReplyOperationRepository, CoachConversationRepository,
        CoachConversationStatus, CoachConversationUseCases, CompletedCoachConversationReply,
        PendingCoachConversationReplyCheckpoint, SharedCoachConversationService,
    },
    identity::{Clock, IdGenerator},
    llm::{
        BoxFuture as LlmBoxFuture, LlmCacheUsage, LlmChatMessage, LlmChatPort, LlmChatRequest,
        LlmChatResponse, LlmContextCache, LlmContextCacheRepository, LlmError, LlmProvider,
        LlmProviderConfig, LlmTokenUsage, LlmToolCall, UserLlmConfigProvider,
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
struct StaticLlmChatPort {
    requests: Arc<Mutex<Vec<LlmChatRequest>>>,
    response: LlmChatResponse,
}

impl StaticLlmChatPort {
    fn new(response: LlmChatResponse) -> Self {
        Self {
            requests: Arc::new(Mutex::new(Vec::new())),
            response,
        }
    }

    fn requests(&self) -> Vec<LlmChatRequest> {
        self.requests.lock().unwrap().clone()
    }
}

impl LlmChatPort for StaticLlmChatPort {
    fn chat(
        &self,
        _config: LlmProviderConfig,
        request: LlmChatRequest,
    ) -> LlmBoxFuture<Result<LlmChatResponse, LlmError>> {
        let state = self.requests.clone();
        let response = self.response.clone();
        Box::pin(async move {
            state.lock().unwrap().push(request);
            Ok(response)
        })
    }
}

#[derive(Clone)]
struct TestIds {
    counter: Arc<Mutex<u64>>,
}

impl TestIds {
    fn new() -> Self {
        Self {
            counter: Arc::new(Mutex::new(0)),
        }
    }
}

impl Default for TestIds {
    fn default() -> Self {
        Self::new()
    }
}

impl IdGenerator for TestIds {
    fn new_id(&self, prefix: &str) -> String {
        let mut counter = self.counter.lock().unwrap();
        let id = format!("{prefix}-{counter}");
        *counter += 1;
        id
    }
}

#[derive(Clone, Default)]
struct InMemoryConversationRepository {
    conversation: Arc<Mutex<Option<CoachConversation>>>,
    hidden_transcript_conflicts_remaining: Arc<Mutex<usize>>,
}

impl InMemoryConversationRepository {
    fn conflict_next_hidden_transcript_write(&self) {
        *self.hidden_transcript_conflicts_remaining.lock().unwrap() = 1;
    }
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

    fn replace_provider_transcript(
        &self,
        user_id: &str,
        conversation_id: &str,
        provider_transcript: Vec<aiwattcoach::domain::llm::LlmChatMessage>,
        expected_updated_at_epoch_seconds: i64,
        updated_at_epoch_seconds: i64,
    ) -> aiwattcoach::domain::coach_conversation::BoxFuture<Result<(), CoachConversationError>>
    {
        let state = self.conversation.clone();
        let hidden_transcript_conflicts_remaining =
            self.hidden_transcript_conflicts_remaining.clone();
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
            let mut conflicts_remaining = hidden_transcript_conflicts_remaining.lock().unwrap();
            if *conflicts_remaining > 0 {
                *conflicts_remaining -= 1;
                existing
                    .provider_transcript
                    .push(LlmChatMessage::assistant("Concurrent calendar update"));
                existing.updated_at_epoch_seconds += 1;
            }
            if existing.updated_at_epoch_seconds != expected_updated_at_epoch_seconds {
                return Err(CoachConversationError::Repository(
                    "provider transcript update lost compare-and-set race".to_string(),
                ));
            }
            existing.provider_transcript = provider_transcript;
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
                    && operation.scope_id == conversation_id
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
                message: aiwattcoach::domain::llm::LlmChatMessage::assistant("Coach reply"),
                finish_reason: None,
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
        TestIds::new(),
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
        TestIds::new(),
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

#[tokio::test]
async fn calendar_coach_returns_dedicated_error_when_reply_is_already_pending() {
    let conversations = InMemoryConversationRepository::default();
    let messages = InMemoryMessageRepository::default();
    let reply_operations = InMemoryReplyOperationRepository::default();
    let service = SharedCoachConversationService::new(
        conversations.clone(),
        messages.clone(),
        reply_operations.clone(),
        Arc::new(RecordingLlmChatPort::default()),
        Arc::new(StaticLlmConfigProvider),
        Arc::new(RecordingTrainingContextBuilder::default()),
        FixedClock,
        TestIds::new(),
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
            "Need calendar advice".to_string(),
        )
        .await
        .expect("user message should persist");

    reply_operations
        .upsert(CoachConversationReplyOperation::pending(
            "user-1".to_string(),
            conversation.conversation_id.clone(),
            persisted.user_message.id.clone(),
            Some("calendar-coach:user-1:overview".to_string()),
            "message-pending".to_string(),
            1_700_000_000,
        ))
        .await
        .expect("pending operation should persist");

    let error = service
        .generate_calendar_reply(
            "user-1",
            &conversation.conversation_id,
            persisted.user_message.id.clone(),
        )
        .await
        .unwrap_err();

    assert_eq!(error, CoachConversationError::ReplyAlreadyPending);
}

#[tokio::test]
async fn calendar_coach_reuses_completed_operation_without_duplicate_llm_call() {
    let conversations = InMemoryConversationRepository::default();
    let messages = InMemoryMessageRepository::default();
    let reply_operations = InMemoryReplyOperationRepository::default();
    let llm_chat_port = RecordingLlmChatPort::default();
    let service = SharedCoachConversationService::new(
        conversations.clone(),
        messages.clone(),
        reply_operations.clone(),
        Arc::new(llm_chat_port.clone()),
        Arc::new(StaticLlmConfigProvider),
        Arc::new(RecordingTrainingContextBuilder::default()),
        FixedClock,
        TestIds::new(),
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
            "Need calendar advice".to_string(),
        )
        .await
        .expect("user message should persist");

    let coach_message = CoachConversationMessage {
        id: "message-completed".to_string(),
        conversation_id: conversation.conversation_id.clone(),
        user_id: "user-1".to_string(),
        role: CoachConversationMessageRole::Coach,
        content: "Recovered calendar reply".to_string(),
        tool_call: None,
        created_at_epoch_seconds: 1_700_000_001,
    };
    messages
        .append(coach_message.clone())
        .await
        .expect("coach message should persist");
    reply_operations
        .upsert(
            CoachConversationReplyOperation::pending(
                "user-1".to_string(),
                conversation.conversation_id.clone(),
                persisted.user_message.id.clone(),
                Some("calendar-coach:user-1:overview".to_string()),
                coach_message.id.clone(),
                1_700_000_000,
            )
            .mark_completed(CompletedCoachConversationReply {
                provider: LlmProvider::OpenAi,
                model: "gpt-5".to_string(),
                provider_request_id: Some("req-completed".to_string()),
                reply_message_id: coach_message.id.clone(),
                provider_cache_id: None,
                token_usage: LlmTokenUsage::default(),
                cache_usage: LlmCacheUsage::default(),
                updated_at_epoch_seconds: 1_700_000_002,
            }),
        )
        .await
        .expect("completed operation should persist");

    let reply = service
        .generate_calendar_reply(
            "user-1",
            &conversation.conversation_id,
            persisted.user_message.id.clone(),
        )
        .await
        .expect("reply should be reused from completed operation");

    assert_eq!(reply.coach_message.id, coach_message.id);
    assert_eq!(reply.coach_message.content, "Recovered calendar reply");
    assert!(llm_chat_port.requests().is_empty());
}

#[tokio::test]
async fn calendar_coach_follow_up_replays_last_hidden_assistant_tool_calls() {
    let llm_chat_port = StaticLlmChatPort::new(LlmChatResponse {
        provider: LlmProvider::OpenAi,
        model: "gpt-5".to_string(),
        message: LlmChatMessage::assistant("Coach follow-up"),
        finish_reason: None,
        provider_request_id: Some("req-2".to_string()),
        usage: LlmTokenUsage::default(),
        cache: LlmCacheUsage::default(),
    });
    let conversations = InMemoryConversationRepository {
        conversation: Arc::new(Mutex::new(Some(CoachConversation {
            conversation_id: "conversation-1".to_string(),
            user_id: "user-1".to_string(),
            surface: aiwattcoach::domain::coach_conversation::CoachConversationSurface::Calendar,
            status: CoachConversationStatus::Active,
            focus: aiwattcoach::domain::coach_conversation::CoachConversationFocus::Overview,
            provider_transcript: vec![
                LlmChatMessage::assistant_with_tool_calls(
                    "Coach reply",
                    vec![LlmToolCall {
                        id: "tool-1".to_string(),
                        name: "lookupCalendar".to_string(),
                        arguments_json: r#"{\"week\":\"2026-W18\"}"#.to_string(),
                    }],
                ),
                LlmChatMessage::tool("tool-1", "Calendar lookup result"),
            ],
            created_at_epoch_seconds: 1,
            updated_at_epoch_seconds: 2,
        }))),
        hidden_transcript_conflicts_remaining: Arc::new(Mutex::new(0)),
    };
    let messages = InMemoryMessageRepository {
        messages: Arc::new(Mutex::new(vec![
            CoachConversationMessage {
                id: "user-1".to_string(),
                conversation_id: "conversation-1".to_string(),
                user_id: "user-1".to_string(),
                role: CoachConversationMessageRole::User,
                content: "Need recovery advice".to_string(),
                tool_call: None,
                created_at_epoch_seconds: 1,
            },
            CoachConversationMessage {
                id: "tool-1".to_string(),
                conversation_id: "conversation-1".to_string(),
                user_id: "user-1".to_string(),
                role: CoachConversationMessageRole::Tool,
                content: "Tool call: lookupCalendar".to_string(),
                tool_call: Some(aiwattcoach::domain::workout_summary::PublicToolCall {
                    id: "tool-1".to_string(),
                    name: "lookupCalendar".to_string(),
                    arguments_json: r#"{\"week\":\"2026-W18\"}"#.to_string(),
                    arguments_preview: None,
                }),
                created_at_epoch_seconds: 2,
            },
            CoachConversationMessage {
                id: "coach-1".to_string(),
                conversation_id: "conversation-1".to_string(),
                user_id: "user-1".to_string(),
                role: CoachConversationMessageRole::Coach,
                content: "Coach reply".to_string(),
                tool_call: None,
                created_at_epoch_seconds: 3,
            },
        ])),
    };

    let service = SharedCoachConversationService::new(
        conversations,
        messages,
        InMemoryReplyOperationRepository::default(),
        Arc::new(llm_chat_port.clone()),
        Arc::new(StaticLlmConfigProvider),
        Arc::new(RecordingTrainingContextBuilder::default()),
        FixedClock,
        TestIds::new(),
    )
    .with_settings_service(Arc::new(ConfiguredSettingsService));

    let persisted = service
        .append_calendar_user_message(
            "user-1",
            "conversation-1",
            "What about tomorrow?".to_string(),
        )
        .await
        .expect("user message should persist");

    service
        .generate_calendar_reply("user-1", "conversation-1", persisted.user_message.id)
        .await
        .expect("reply should be generated");

    let requests = llm_chat_port.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].conversation.len(), 4);
    assert_eq!(
        requests[0].conversation[1].role,
        aiwattcoach::domain::llm::LlmMessageRole::Assistant
    );
    assert_eq!(requests[0].conversation[1].tool_calls.len(), 1);
    assert_eq!(requests[0].conversation[1].tool_calls[0].id, "tool-1");
    assert_eq!(
        requests[0].conversation[2].tool_call_id.as_deref(),
        Some("tool-1")
    );
    assert_eq!(requests[0].conversation[3].content, "What about tomorrow?");
}

#[tokio::test]
async fn calendar_coach_follow_up_replays_multiple_hidden_assistant_turns_with_trimmed_content() {
    let llm_chat_port = StaticLlmChatPort::new(LlmChatResponse {
        provider: LlmProvider::OpenAi,
        model: "gpt-5".to_string(),
        message: LlmChatMessage::assistant("Final answer"),
        finish_reason: None,
        provider_request_id: Some("req-3".to_string()),
        usage: LlmTokenUsage::default(),
        cache: LlmCacheUsage::default(),
    });
    let conversations = InMemoryConversationRepository {
        conversation: Arc::new(Mutex::new(Some(CoachConversation {
            conversation_id: "conversation-1".to_string(),
            user_id: "user-1".to_string(),
            surface: aiwattcoach::domain::coach_conversation::CoachConversationSurface::Calendar,
            status: CoachConversationStatus::Active,
            focus: aiwattcoach::domain::coach_conversation::CoachConversationFocus::Overview,
            provider_transcript: vec![
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
            created_at_epoch_seconds: 1,
            updated_at_epoch_seconds: 2,
        }))),
        hidden_transcript_conflicts_remaining: Arc::new(Mutex::new(0)),
    };
    let messages = InMemoryMessageRepository {
        messages: Arc::new(Mutex::new(vec![
            CoachConversationMessage {
                id: "user-1".to_string(),
                conversation_id: "conversation-1".to_string(),
                user_id: "user-1".to_string(),
                role: CoachConversationMessageRole::User,
                content: "First question".to_string(),
                tool_call: None,
                created_at_epoch_seconds: 1,
            },
            CoachConversationMessage {
                id: "coach-1".to_string(),
                conversation_id: "conversation-1".to_string(),
                user_id: "user-1".to_string(),
                role: CoachConversationMessageRole::Coach,
                content: "First answer".to_string(),
                tool_call: None,
                created_at_epoch_seconds: 2,
            },
            CoachConversationMessage {
                id: "user-2".to_string(),
                conversation_id: "conversation-1".to_string(),
                user_id: "user-1".to_string(),
                role: CoachConversationMessageRole::User,
                content: "Second question".to_string(),
                tool_call: None,
                created_at_epoch_seconds: 3,
            },
            CoachConversationMessage {
                id: "coach-2".to_string(),
                conversation_id: "conversation-1".to_string(),
                user_id: "user-1".to_string(),
                role: CoachConversationMessageRole::Coach,
                content: "Second answer".to_string(),
                tool_call: None,
                created_at_epoch_seconds: 4,
            },
        ])),
    };

    let service = SharedCoachConversationService::new(
        conversations,
        messages,
        InMemoryReplyOperationRepository::default(),
        Arc::new(llm_chat_port.clone()),
        Arc::new(StaticLlmConfigProvider),
        Arc::new(RecordingTrainingContextBuilder::default()),
        FixedClock,
        TestIds::new(),
    )
    .with_settings_service(Arc::new(ConfiguredSettingsService));

    let persisted = service
        .append_calendar_user_message("user-1", "conversation-1", "Third question".to_string())
        .await
        .expect("user message should persist");

    service
        .generate_calendar_reply("user-1", "conversation-1", persisted.user_message.id)
        .await
        .expect("reply should be generated");

    let requests = llm_chat_port.requests();
    assert_eq!(requests[0].conversation[1].tool_calls[0].id, "tool-1");
    assert_eq!(
        requests[0].conversation[2].tool_call_id.as_deref(),
        Some("tool-1")
    );
    assert_eq!(requests[0].conversation[4].tool_calls[0].id, "tool-2");
    assert_eq!(
        requests[0].conversation[5].tool_call_id.as_deref(),
        Some("tool-2")
    );
}

#[tokio::test]
async fn calendar_coach_marks_tool_only_recovery_as_failed() {
    let conversations = InMemoryConversationRepository::default();
    let messages = InMemoryMessageRepository::default();
    let reply_operations = InMemoryReplyOperationRepository::default();
    let service = SharedCoachConversationService::new(
        conversations.clone(),
        messages.clone(),
        reply_operations.clone(),
        Arc::new(RecordingLlmChatPort::default()),
        Arc::new(StaticLlmConfigProvider),
        Arc::new(RecordingTrainingContextBuilder::default()),
        FixedClock,
        TestIds::new(),
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
            "Need recovery advice".to_string(),
        )
        .await
        .expect("user message should persist");
    let user_message_id = persisted.user_message.id.clone();

    reply_operations
        .upsert(
            CoachConversationReplyOperation::pending(
                "user-1".to_string(),
                conversation.conversation_id.clone(),
                user_message_id.clone(),
                Some("calendar-coach:user-1:overview".to_string()),
                "message-tool-only".to_string(),
                1_699_999_000,
            )
            .record_provider_response(PendingCoachConversationReplyCheckpoint {
                provider: LlmProvider::OpenRouter,
                model: "openai/gpt-4o-mini".to_string(),
                provider_request_id: Some("req-tool-only".to_string()),
                provider_cache_id: None,
                token_usage: LlmTokenUsage::default(),
                cache_usage: LlmCacheUsage::default(),
                provider_transcript: vec![LlmChatMessage::assistant_with_tool_calls(
                    "",
                    vec![LlmToolCall {
                        id: "tool-1".to_string(),
                        name: "lookupCalendar".to_string(),
                        arguments_json: "{}".to_string(),
                    }],
                )],
                finish_reason: None,
                updated_at_epoch_seconds: 1_699_999_001,
            }),
        )
        .await
        .unwrap();

    let error = service
        .generate_calendar_reply(
            "user-1",
            &conversation.conversation_id,
            user_message_id.clone(),
        )
        .await
        .unwrap_err();

    assert_eq!(
        error,
        CoachConversationError::Llm(LlmError::InvalidResponse(
            "assistant reply missing final text message".to_string(),
        ))
    );
    let stored = reply_operations
        .find_by_user_message_id("user-1", &conversation.conversation_id, &user_message_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        stored.status,
        aiwattcoach::domain::coach_conversation::CoachConversationReplyOperationStatus::Failed
    );
}

#[tokio::test]
async fn calendar_coach_marks_fresh_tool_only_response_as_failed() {
    let llm_chat_port = StaticLlmChatPort::new(LlmChatResponse {
        provider: LlmProvider::OpenRouter,
        model: "openai/gpt-4o-mini".to_string(),
        message: LlmChatMessage::assistant_with_tool_calls(
            "",
            vec![LlmToolCall {
                id: "tool-1".to_string(),
                name: "lookupCalendar".to_string(),
                arguments_json: "{}".to_string(),
            }],
        ),
        finish_reason: None,
        provider_request_id: Some("req-tool-only-fresh".to_string()),
        usage: LlmTokenUsage::default(),
        cache: LlmCacheUsage::default(),
    });
    let conversations = InMemoryConversationRepository::default();
    let messages = InMemoryMessageRepository::default();
    let reply_operations = InMemoryReplyOperationRepository::default();
    let service = SharedCoachConversationService::new(
        conversations.clone(),
        messages.clone(),
        reply_operations.clone(),
        Arc::new(llm_chat_port),
        Arc::new(StaticLlmConfigProvider),
        Arc::new(RecordingTrainingContextBuilder::default()),
        FixedClock,
        TestIds::new(),
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
            "Need recovery advice".to_string(),
        )
        .await
        .expect("user message should persist");

    let error = service
        .generate_calendar_reply(
            "user-1",
            &conversation.conversation_id,
            persisted.user_message.id.clone(),
        )
        .await
        .unwrap_err();

    assert_eq!(
        error,
        CoachConversationError::Llm(LlmError::InvalidResponse(
            "assistant reply missing final text message".to_string(),
        ))
    );
    let stored = reply_operations
        .find_by_user_message_id(
            "user-1",
            &conversation.conversation_id,
            &persisted.user_message.id,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        stored.status,
        aiwattcoach::domain::coach_conversation::CoachConversationReplyOperationStatus::Failed
    );
}

#[tokio::test]
async fn calendar_coach_retries_provider_transcript_write_after_compare_and_set_conflict() {
    let llm_chat_port = RecordingLlmChatPort::default();
    let conversations = InMemoryConversationRepository::default();
    let messages = InMemoryMessageRepository::default();
    let reply_operations = InMemoryReplyOperationRepository::default();
    let service = SharedCoachConversationService::new(
        conversations.clone(),
        messages,
        reply_operations,
        Arc::new(llm_chat_port),
        Arc::new(StaticLlmConfigProvider),
        Arc::new(RecordingTrainingContextBuilder::default()),
        FixedClock,
        TestIds::new(),
    )
    .with_settings_service(Arc::new(ConfiguredSettingsService));

    let (conversation, _) = service
        .get_or_create_active_calendar_conversation("user-1")
        .await
        .expect("conversation should be created");

    {
        let mut guard = conversations.conversation.lock().unwrap();
        guard.as_mut().unwrap().provider_transcript = vec![LlmChatMessage::assistant("Turn 0")];
    }
    conversations.conflict_next_hidden_transcript_write();

    let persisted = service
        .append_calendar_user_message(
            "user-1",
            &conversation.conversation_id,
            "What should I do tomorrow?".to_string(),
        )
        .await
        .expect("user message should persist");

    service
        .generate_calendar_reply(
            "user-1",
            &conversation.conversation_id,
            persisted.user_message.id,
        )
        .await
        .expect("reply should be generated");

    let stored = conversations
        .find_by_user_id_and_conversation_id("user-1", &conversation.conversation_id)
        .await
        .unwrap()
        .unwrap();
    let provider_contents = stored
        .provider_transcript
        .iter()
        .map(|message| message.content.as_str())
        .collect::<Vec<_>>();

    assert!(provider_contents.contains(&"Turn 0"));
    assert!(provider_contents.contains(&"Concurrent calendar update"));
    assert!(provider_contents.contains(&"Coach reply"));
}
