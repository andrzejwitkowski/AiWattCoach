use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use tokio::sync::Notify;

use crate::domain::{
    athlete_summary::{
        AthleteSummary, AthleteSummaryError, AthleteSummaryState, AthleteSummaryUseCases,
        EnsuredAthleteSummary,
    },
    llm::{LlmCacheUsage, LlmChatMessage, LlmChatResponse, LlmError, LlmProvider, LlmTokenUsage},
    workout_summary::{
        CoachReplyClaimResult, CoachReplyOperation, CoachReplyOperationRepository, WorkoutCoach,
        WorkoutSummaryRepository, WorkoutSummaryService,
    },
};

use super::super::*;
use super::task_support::{TestClock, TestIdGenerator};

#[derive(Clone, Default)]
pub(super) struct InMemoryWorkoutSummaryRepository {
    summaries: Arc<Mutex<HashMap<(String, String), WorkoutSummary>>>,
}

impl InMemoryWorkoutSummaryRepository {
    pub(super) fn with_summary(summary: WorkoutSummary) -> Self {
        let mut summaries = HashMap::new();
        summaries.insert(
            (summary.user_id.clone(), summary.workout_id.clone()),
            summary,
        );
        Self {
            summaries: Arc::new(Mutex::new(summaries)),
        }
    }
}

impl WorkoutSummaryRepository for InMemoryWorkoutSummaryRepository {
    fn find_by_user_id_and_workout_id(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> crate::domain::workout_summary::BoxFuture<
        Result<Option<WorkoutSummary>, WorkoutSummaryError>,
    > {
        let summaries = self.summaries.clone();
        let key = (user_id.to_string(), workout_id.to_string());
        Box::pin(async move {
            Ok(summaries
                .lock()
                .expect("summary mutex poisoned")
                .get(&key)
                .cloned())
        })
    }

    fn find_by_user_id_and_workout_ids(
        &self,
        user_id: &str,
        workout_ids: Vec<String>,
    ) -> crate::domain::workout_summary::BoxFuture<Result<Vec<WorkoutSummary>, WorkoutSummaryError>>
    {
        let summaries = self.summaries.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let summaries = summaries.lock().expect("summary mutex poisoned");
            Ok(workout_ids
                .into_iter()
                .filter_map(|workout_id| summaries.get(&(user_id.clone(), workout_id)).cloned())
                .collect())
        })
    }

    fn create(
        &self,
        summary: WorkoutSummary,
    ) -> crate::domain::workout_summary::BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>>
    {
        let summaries = self.summaries.clone();
        Box::pin(async move {
            summaries.lock().expect("summary mutex poisoned").insert(
                (summary.user_id.clone(), summary.workout_id.clone()),
                summary.clone(),
            );
            Ok(summary)
        })
    }

    fn update_rpe(
        &self,
        user_id: &str,
        workout_id: &str,
        rpe: u8,
        updated_at_epoch_seconds: i64,
    ) -> crate::domain::workout_summary::BoxFuture<Result<(), WorkoutSummaryError>> {
        let summaries = self.summaries.clone();
        let key = (user_id.to_string(), workout_id.to_string());
        Box::pin(async move {
            let mut summaries = summaries.lock().expect("summary mutex poisoned");
            let summary = summaries
                .get_mut(&key)
                .ok_or(WorkoutSummaryError::NotFound)?;
            summary.rpe = Some(rpe);
            summary.updated_at_epoch_seconds = updated_at_epoch_seconds;
            Ok(())
        })
    }

    fn append_message(
        &self,
        user_id: &str,
        workout_id: &str,
        message: ConversationMessage,
        updated_at_epoch_seconds: i64,
    ) -> crate::domain::workout_summary::BoxFuture<Result<(), WorkoutSummaryError>> {
        let summaries = self.summaries.clone();
        let key = (user_id.to_string(), workout_id.to_string());
        Box::pin(async move {
            let mut summaries = summaries.lock().expect("summary mutex poisoned");
            let summary = summaries
                .get_mut(&key)
                .ok_or(WorkoutSummaryError::NotFound)?;
            if summary
                .messages
                .iter()
                .any(|existing| existing.id == message.id)
            {
                return Ok(());
            }
            summary.messages.push(message);
            summary.updated_at_epoch_seconds = updated_at_epoch_seconds;
            Ok(())
        })
    }

    fn set_saved_state(
        &self,
        user_id: &str,
        workout_id: &str,
        saved_at_epoch_seconds: Option<i64>,
        updated_at_epoch_seconds: i64,
    ) -> crate::domain::workout_summary::BoxFuture<Result<(), WorkoutSummaryError>> {
        let summaries = self.summaries.clone();
        let key = (user_id.to_string(), workout_id.to_string());
        Box::pin(async move {
            let mut summaries = summaries.lock().expect("summary mutex poisoned");
            let summary = summaries
                .get_mut(&key)
                .ok_or(WorkoutSummaryError::NotFound)?;
            summary.saved_at_epoch_seconds = saved_at_epoch_seconds;
            summary.updated_at_epoch_seconds = updated_at_epoch_seconds;
            Ok(())
        })
    }

    fn replace_provider_transcript(
        &self,
        user_id: &str,
        workout_id: &str,
        provider_transcript: Vec<LlmChatMessage>,
        expected_updated_at_epoch_seconds: i64,
        updated_at_epoch_seconds: i64,
    ) -> crate::domain::workout_summary::BoxFuture<Result<(), WorkoutSummaryError>> {
        let summaries = self.summaries.clone();
        let key = (user_id.to_string(), workout_id.to_string());
        Box::pin(async move {
            let mut summaries = summaries.lock().expect("summary mutex poisoned");
            let summary = summaries
                .get_mut(&key)
                .ok_or(WorkoutSummaryError::NotFound)?;
            if summary.updated_at_epoch_seconds != expected_updated_at_epoch_seconds {
                return Err(WorkoutSummaryError::Repository(
                    "provider transcript update lost compare-and-set race".to_string(),
                ));
            }
            summary.provider_transcript = provider_transcript;
            summary.updated_at_epoch_seconds = updated_at_epoch_seconds;
            Ok(())
        })
    }

    fn persist_workout_recap(
        &self,
        user_id: &str,
        workout_id: &str,
        recap: crate::domain::workout_summary::WorkoutRecap,
        updated_at_epoch_seconds: i64,
    ) -> crate::domain::workout_summary::BoxFuture<Result<(), WorkoutSummaryError>> {
        let summaries = self.summaries.clone();
        let key = (user_id.to_string(), workout_id.to_string());
        Box::pin(async move {
            let mut summaries = summaries.lock().expect("summary mutex poisoned");
            let summary = summaries
                .get_mut(&key)
                .ok_or(WorkoutSummaryError::NotFound)?;
            summary.workout_recap_text = Some(recap.text);
            summary.workout_recap_provider = Some(recap.provider);
            summary.workout_recap_model = Some(recap.model);
            summary.workout_recap_generated_at_epoch_seconds =
                Some(recap.generated_at_epoch_seconds);
            summary.updated_at_epoch_seconds = updated_at_epoch_seconds;
            Ok(())
        })
    }

    fn find_message_by_id(
        &self,
        user_id: &str,
        workout_id: &str,
        message_id: &str,
    ) -> crate::domain::workout_summary::BoxFuture<
        Result<Option<ConversationMessage>, WorkoutSummaryError>,
    > {
        let summaries = self.summaries.clone();
        let key = (user_id.to_string(), workout_id.to_string());
        let message_id = message_id.to_string();
        Box::pin(async move {
            let message = summaries
                .lock()
                .expect("summary mutex poisoned")
                .get(&key)
                .and_then(|summary| {
                    summary
                        .messages
                        .iter()
                        .find(|message| message.id == message_id)
                        .cloned()
                });
            Ok(message)
        })
    }
}

type ReplyOperationKey = (String, String, String);

#[derive(Clone, Default)]
pub(super) struct InMemoryCoachReplyOperationRepository {
    operations: Arc<Mutex<HashMap<ReplyOperationKey, CoachReplyOperation>>>,
}

impl InMemoryCoachReplyOperationRepository {
    pub(super) fn seed(&self, operation: CoachReplyOperation) {
        self.operations
            .lock()
            .expect("reply op mutex poisoned")
            .insert(
                (
                    operation.user_id.clone(),
                    operation.scope_id.clone(),
                    operation.user_message_id.clone(),
                ),
                operation,
            );
    }

    pub(super) fn get(
        &self,
        user_id: &str,
        workout_id: &str,
        user_message_id: &str,
    ) -> Option<CoachReplyOperation> {
        self.operations
            .lock()
            .expect("reply op mutex poisoned")
            .get(&(
                user_id.to_string(),
                workout_id.to_string(),
                user_message_id.to_string(),
            ))
            .cloned()
    }
}

impl CoachReplyOperationRepository for InMemoryCoachReplyOperationRepository {
    fn find_by_user_message_id(
        &self,
        user_id: &str,
        workout_id: &str,
        user_message_id: &str,
    ) -> crate::domain::workout_summary::BoxFuture<
        Result<Option<CoachReplyOperation>, WorkoutSummaryError>,
    > {
        let operations = self.operations.clone();
        let key = (
            user_id.to_string(),
            workout_id.to_string(),
            user_message_id.to_string(),
        );
        Box::pin(async move {
            Ok(operations
                .lock()
                .expect("reply op mutex poisoned")
                .get(&key)
                .cloned())
        })
    }

    fn claim_pending(
        &self,
        operation: CoachReplyOperation,
        stale_before_epoch_seconds: i64,
    ) -> crate::domain::workout_summary::BoxFuture<Result<CoachReplyClaimResult, WorkoutSummaryError>>
    {
        let operations = self.operations.clone();
        Box::pin(async move {
            let key = (
                operation.user_id.clone(),
                operation.scope_id.clone(),
                operation.user_message_id.clone(),
            );
            let mut operations = operations.lock().expect("reply op mutex poisoned");
            if let Some(existing) = operations.get(&key).cloned() {
                let reclaimable = match existing.status {
                    crate::domain::workout_summary::CoachReplyOperationStatus::Pending => {
                        existing.is_stale(stale_before_epoch_seconds)
                    }
                    crate::domain::workout_summary::CoachReplyOperationStatus::Failed => true,
                    crate::domain::workout_summary::CoachReplyOperationStatus::Completed => false,
                };

                if reclaimable {
                    let fallback_coach_message_id = operation
                        .reply_message_id
                        .clone()
                        .unwrap_or_else(|| "coach-message-fallback".to_string());
                    let reclaimed = existing.reclaim(
                        fallback_coach_message_id,
                        operation.last_attempt_at_epoch_seconds,
                    );
                    operations.insert(key, reclaimed.clone());
                    return Ok(CoachReplyClaimResult::Claimed(reclaimed));
                }
                return Ok(CoachReplyClaimResult::Existing(existing));
            }

            operations.insert(key, operation.clone());
            Ok(CoachReplyClaimResult::Claimed(operation))
        })
    }

    fn upsert(
        &self,
        operation: CoachReplyOperation,
    ) -> crate::domain::workout_summary::BoxFuture<Result<CoachReplyOperation, WorkoutSummaryError>>
    {
        let operations = self.operations.clone();
        Box::pin(async move {
            operations.lock().expect("reply op mutex poisoned").insert(
                (
                    operation.user_id.clone(),
                    operation.scope_id.clone(),
                    operation.user_message_id.clone(),
                ),
                operation.clone(),
            );
            Ok(operation)
        })
    }
}

#[derive(Clone, Default)]
pub(super) struct TestCoach {
    pub(super) fail_with: Arc<std::sync::Mutex<Option<LlmError>>>,
}

impl TestCoach {
    pub(super) fn failing(error: LlmError) -> Arc<Self> {
        Arc::new(Self {
            fail_with: Arc::new(std::sync::Mutex::new(Some(error))),
        })
    }

    pub(super) fn successful() -> Arc<Self> {
        Arc::new(Self::default())
    }
}

impl WorkoutCoach for TestCoach {
    fn reply(
        &self,
        _user_id: &str,
        _summary: &WorkoutSummary,
        user_message: &str,
        _athlete_summary_text: Option<&str>,
    ) -> crate::domain::llm::BoxFuture<Result<crate::domain::llm_tools::LlmToolLoopOutput, LlmError>>
    {
        let error = self.fail_with.lock().expect("coach mutex poisoned").take();
        let user_message = user_message.to_string();
        Box::pin(async move {
            if let Some(error) = error {
                return Err(error);
            }
            Ok(crate::domain::llm_tools::LlmToolLoopOutput::from_response(
                LlmChatResponse {
                    provider: LlmProvider::OpenAi,
                    model: "test-coach".to_string(),
                    message: LlmChatMessage::assistant(
                        crate::domain::workout_summary::coach_reply_json(format!(
                            "Coach reply to: {user_message}"
                        )),
                    ),
                    finish_reason: None,
                    provider_request_id: Some("req-1".to_string()),
                    usage: LlmTokenUsage::default(),
                    cache: LlmCacheUsage::default(),
                },
            ))
        })
    }
}

#[derive(Clone)]
pub(super) struct BlockingCoach {
    pub(super) started: Arc<Notify>,
    pub(super) release: Arc<Notify>,
}

impl BlockingCoach {
    pub(super) fn new() -> Arc<Self> {
        Arc::new(Self {
            started: Arc::new(Notify::new()),
            release: Arc::new(Notify::new()),
        })
    }
}

impl WorkoutCoach for BlockingCoach {
    fn reply(
        &self,
        _user_id: &str,
        _summary: &WorkoutSummary,
        user_message: &str,
        _athlete_summary_text: Option<&str>,
    ) -> crate::domain::llm::BoxFuture<Result<crate::domain::llm_tools::LlmToolLoopOutput, LlmError>>
    {
        let started = self.started.clone();
        let release = self.release.clone();
        let user_message = user_message.to_string();
        Box::pin(async move {
            started.notify_one();
            release.notified().await;
            Ok(crate::domain::llm_tools::LlmToolLoopOutput::from_response(
                LlmChatResponse {
                    provider: LlmProvider::OpenAi,
                    model: "test-coach".to_string(),
                    message: LlmChatMessage::assistant(
                        crate::domain::workout_summary::coach_reply_json(format!(
                            "Coach reply to: {user_message}"
                        )),
                    ),
                    finish_reason: None,
                    provider_request_id: Some("req-1".to_string()),
                    usage: LlmTokenUsage::default(),
                    cache: LlmCacheUsage::default(),
                },
            ))
        })
    }
}

#[derive(Clone)]
pub(super) struct StaticAthleteSummaryService {
    pub(super) was_regenerated: bool,
}

impl AthleteSummaryUseCases for StaticAthleteSummaryService {
    fn get_summary_state(
        &self,
        _user_id: &str,
    ) -> crate::domain::athlete_summary::BoxFuture<Result<AthleteSummaryState, AthleteSummaryError>>
    {
        Box::pin(async {
            Ok(AthleteSummaryState {
                summary: None,
                stale: false,
            })
        })
    }

    fn generate_summary(
        &self,
        _user_id: &str,
        _force: bool,
    ) -> crate::domain::athlete_summary::BoxFuture<Result<AthleteSummary, AthleteSummaryError>>
    {
        Box::pin(async { Ok(sample_athlete_summary()) })
    }

    fn ensure_fresh_summary(
        &self,
        _user_id: &str,
    ) -> crate::domain::athlete_summary::BoxFuture<Result<AthleteSummary, AthleteSummaryError>>
    {
        Box::pin(async { Ok(sample_athlete_summary()) })
    }

    fn ensure_fresh_summary_state(
        &self,
        _user_id: &str,
    ) -> crate::domain::athlete_summary::BoxFuture<Result<EnsuredAthleteSummary, AthleteSummaryError>>
    {
        let was_regenerated = self.was_regenerated;
        Box::pin(async move {
            Ok(EnsuredAthleteSummary {
                summary: sample_athlete_summary(),
                was_regenerated,
            })
        })
    }
}

fn sample_athlete_summary() -> AthleteSummary {
    AthleteSummary {
        user_id: "user-1".to_string(),
        summary_text: "fresh athlete summary".to_string(),
        generated_at_epoch_seconds: 1_700_000_000,
        created_at_epoch_seconds: 1_700_000_000,
        updated_at_epoch_seconds: 1_700_000_000,
        provider: Some("openai".to_string()),
        model: Some("test-model".to_string()),
    }
}

pub(super) fn existing_summary() -> WorkoutSummary {
    WorkoutSummary {
        id: "summary-1".to_string(),
        user_id: "user-1".to_string(),
        workout_id: "workout-1".to_string(),
        rpe: Some(6),
        messages: Vec::new(),
        provider_transcript: Vec::new(),
        saved_at_epoch_seconds: None,
        workout_recap_text: None,
        workout_recap_provider: None,
        workout_recap_model: None,
        workout_recap_generated_at_epoch_seconds: None,
        created_at_epoch_seconds: 1_700_000_000,
        updated_at_epoch_seconds: 1_700_000_000,
    }
}

pub(super) fn direct_service(
    repository: InMemoryWorkoutSummaryRepository,
    coach: Arc<dyn WorkoutCoach>,
) -> Arc<
    WorkoutSummaryService<
        InMemoryWorkoutSummaryRepository,
        InMemoryCoachReplyOperationRepository,
        TestClock,
        TestIdGenerator,
    >,
> {
    Arc::new(WorkoutSummaryService::with_coach(
        repository,
        InMemoryCoachReplyOperationRepository::default(),
        TestClock::default(),
        TestIdGenerator::default(),
        coach,
    ))
}

pub(super) fn direct_service_with_operation_repository(
    repository: InMemoryWorkoutSummaryRepository,
    reply_operations: InMemoryCoachReplyOperationRepository,
    clock: TestClock,
    coach: Arc<dyn WorkoutCoach>,
) -> Arc<
    WorkoutSummaryService<
        InMemoryWorkoutSummaryRepository,
        InMemoryCoachReplyOperationRepository,
        TestClock,
        TestIdGenerator,
    >,
> {
    Arc::new(WorkoutSummaryService::with_coach(
        repository,
        reply_operations,
        clock,
        TestIdGenerator::default(),
        coach,
    ))
}

pub(super) fn direct_service_with_athlete_summary(
    repository: InMemoryWorkoutSummaryRepository,
    coach: Arc<dyn WorkoutCoach>,
    was_regenerated: bool,
) -> Arc<
    WorkoutSummaryService<
        InMemoryWorkoutSummaryRepository,
        InMemoryCoachReplyOperationRepository,
        TestClock,
        TestIdGenerator,
    >,
> {
    Arc::new(
        WorkoutSummaryService::with_coach(
            repository,
            InMemoryCoachReplyOperationRepository::default(),
            TestClock::default(),
            TestIdGenerator::default(),
            coach,
        )
        .with_athlete_summary_service(Arc::new(StaticAthleteSummaryService { was_regenerated })),
    )
}
