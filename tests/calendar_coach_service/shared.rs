use std::sync::{Arc, Mutex};

use aiwattcoach::domain::{
    coach_conversation::{
        CoachConversation, CoachConversationError, CoachConversationMessage,
        CoachConversationMessageRepository, CoachConversationReplyClaimResult,
        CoachConversationReplyOperation, CoachConversationReplyOperationRepository,
        CoachConversationRepository, CoachConversationStatus,
    },
    identity::{Clock, IdGenerator},
    llm::{
        BoxFuture as LlmBoxFuture, LlmCacheUsage, LlmChatMessage, LlmChatPort, LlmChatRequest,
        LlmChatResponse, LlmContextCache, LlmContextCacheRepository, LlmError, LlmProvider,
        LlmProviderConfig, LlmTokenUsage, UserLlmConfigProvider,
    },
    settings::{
        AiAgentsConfig, AnalysisOptions, AvailabilityDay, AvailabilitySettings, CyclingSettings,
        IntervalsConfig, SettingsError, UserSettings, UserSettingsUseCases, WahooConfig, Weekday,
    },
    training_context::{
        RenderedTrainingContext, TrainingContext, TrainingContextBuildResult,
        TrainingContextBuilder, MESO_CYCLE_FOCUS_ID,
    },
};

#[derive(Clone)]
pub(super) struct FixedClock;

impl Clock for FixedClock {
    fn now_epoch_seconds(&self) -> i64 {
        1_700_000_000
    }
}

#[derive(Clone)]
pub(super) struct StaticLlmChatPort {
    requests: Arc<Mutex<Vec<LlmChatRequest>>>,
    response: LlmChatResponse,
}

impl StaticLlmChatPort {
    pub(super) fn new(response: LlmChatResponse) -> Self {
        Self {
            requests: Arc::new(Mutex::new(Vec::new())),
            response,
        }
    }

    pub(super) fn requests(&self) -> Vec<LlmChatRequest> {
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
pub(super) struct TestIds {
    counter: Arc<Mutex<u64>>,
}

impl TestIds {
    pub(super) fn new() -> Self {
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
pub(super) struct InMemoryConversationRepository {
    pub(super) conversation: Arc<Mutex<Option<CoachConversation>>>,
    hidden_transcript_conflicts_remaining: Arc<Mutex<usize>>,
}

impl InMemoryConversationRepository {
    pub(super) fn with_conversation(conversation: CoachConversation) -> Self {
        Self {
            conversation: Arc::new(Mutex::new(Some(conversation))),
            ..Default::default()
        }
    }

    pub(super) fn conflict_next_hidden_transcript_write(&self) {
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
pub(super) struct InMemoryMessageRepository {
    pub(super) messages: Arc<Mutex<Vec<CoachConversationMessage>>>,
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
pub(super) struct InMemoryReplyOperationRepository {
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
pub(super) struct RecordingLlmChatPort {
    requests: Arc<Mutex<Vec<LlmChatRequest>>>,
}

impl RecordingLlmChatPort {
    pub(super) fn requests(&self) -> Vec<LlmChatRequest> {
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
pub(super) struct StaticLlmConfigProvider;

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
pub(super) struct RecordingTrainingContextBuilder {
    build_calls: Arc<Mutex<Vec<String>>>,
    calendar_overview_calls: Arc<Mutex<Vec<String>>>,
    athlete_summary_calls: Arc<Mutex<Vec<String>>>,
}

impl RecordingTrainingContextBuilder {
    pub(super) fn build_calls(&self) -> Vec<String> {
        self.build_calls.lock().unwrap().clone()
    }

    pub(super) fn calendar_overview_calls(&self) -> Vec<String> {
        self.calendar_overview_calls.lock().unwrap().clone()
    }

    pub(super) fn athlete_summary_calls(&self) -> Vec<String> {
        self.athlete_summary_calls.lock().unwrap().clone()
    }

    fn result_for(focus_id: Option<String>, focus_kind: &str) -> TrainingContextBuildResult {
        TrainingContextBuildResult {
            focus_date: "2026-05-29".to_string(),
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
                recent_workout_recaps: Vec::new(),
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

    fn build_as_of(
        &self,
        user_id: &str,
        workout_id: &str,
        _focus_date: chrono::NaiveDate,
    ) -> LlmBoxFuture<Result<TrainingContextBuildResult, LlmError>> {
        self.build(user_id, workout_id)
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

    fn build_calendar_overview_context_as_of(
        &self,
        user_id: &str,
        _focus_date: chrono::NaiveDate,
    ) -> LlmBoxFuture<Result<TrainingContextBuildResult, LlmError>> {
        self.build_calendar_overview_context(user_id)
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

    fn build_meso_cycle_context(
        &self,
        _user_id: &str,
        _meso_end: chrono::NaiveDate,
    ) -> LlmBoxFuture<Result<TrainingContextBuildResult, LlmError>> {
        Box::pin(async move { Ok(Self::result_for(None, MESO_CYCLE_FOCUS_ID)) })
    }
}

#[derive(Clone)]
pub(super) struct ConfiguredSettingsService;

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
pub(super) struct NoopContextCacheRepository;

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

pub(super) fn configured_settings(user_id: &str) -> UserSettings {
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
