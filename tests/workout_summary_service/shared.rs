use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicUsize, Ordering},
    sync::{Arc, Mutex},
};

use aiwattcoach::domain::{
    identity::{Clock, IdGenerator},
    llm::{LlmCacheUsage, LlmChatResponse, LlmProvider, LlmTokenUsage},
    workout_summary::{
        BoxFuture, CoachReplyClaimResult, CoachReplyOperation, CoachReplyOperationRepository,
        CoachReplyOperationStatus, ConversationMessage, MessageRole, WorkoutCoach, WorkoutSummary,
        WorkoutSummaryError, WorkoutSummaryRepository, WorkoutSummaryService,
    },
};

type ReplyOperationKey = (String, String, String);
type ReplyOperationStore = BTreeMap<ReplyOperationKey, CoachReplyOperation>;

#[derive(Clone)]
pub(crate) struct TestClock;

impl Clock for TestClock {
    fn now_epoch_seconds(&self) -> i64 {
        1_700_000_000
    }
}

#[derive(Clone, Default)]
pub(crate) struct TestIdGenerator {
    next_id: Arc<AtomicUsize>,
}

impl IdGenerator for TestIdGenerator {
    fn new_id(&self, prefix: &str) -> String {
        let next_id = self.next_id.fetch_add(1, Ordering::SeqCst) + 1;
        format!("{prefix}-{next_id}")
    }
}

#[derive(Clone, Default)]
pub(crate) struct InMemoryWorkoutSummaryRepository {
    summaries: Arc<Mutex<BTreeMap<(String, String), WorkoutSummary>>>,
    calls: Arc<Mutex<Vec<String>>>,
}

impl InMemoryWorkoutSummaryRepository {
    pub(crate) fn with_summary(summary: WorkoutSummary) -> Self {
        let mut summaries = BTreeMap::new();
        summaries.insert(
            (summary.user_id.clone(), summary.workout_id.clone()),
            summary,
        );

        Self {
            summaries: Arc::new(Mutex::new(summaries)),
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub(crate) fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

impl WorkoutSummaryRepository for InMemoryWorkoutSummaryRepository {
    fn find_by_user_id_and_workout_id(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> BoxFuture<Result<Option<WorkoutSummary>, WorkoutSummaryError>> {
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        let summaries = self.summaries.clone();
        Box::pin(async move {
            Ok(summaries
                .lock()
                .unwrap()
                .get(&(user_id, workout_id))
                .cloned())
        })
    }

    fn find_by_user_id_and_workout_ids(
        &self,
        user_id: &str,
        workout_ids: Vec<String>,
    ) -> BoxFuture<Result<Vec<WorkoutSummary>, WorkoutSummaryError>> {
        let user_id = user_id.to_string();
        let summaries = self.summaries.clone();
        Box::pin(async move {
            let summaries = summaries.lock().unwrap();
            Ok(workout_ids
                .into_iter()
                .filter_map(|workout_id| summaries.get(&(user_id.clone(), workout_id)).cloned())
                .collect())
        })
    }

    fn create(
        &self,
        summary: WorkoutSummary,
    ) -> BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>> {
        let summaries = self.summaries.clone();
        let calls = self.calls.clone();
        Box::pin(async move {
            calls
                .lock()
                .unwrap()
                .push(format!("create:{}", summary.workout_id));
            let key = (summary.user_id.clone(), summary.workout_id.clone());
            let mut summaries = summaries.lock().unwrap();
            if summaries.contains_key(&key) {
                return Err(WorkoutSummaryError::AlreadyExists);
            }
            summaries.insert(key, summary.clone());
            Ok(summary)
        })
    }

    fn update_rpe(
        &self,
        user_id: &str,
        workout_id: &str,
        rpe: u8,
        updated_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<(), WorkoutSummaryError>> {
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        let summaries = self.summaries.clone();
        let calls = self.calls.clone();
        Box::pin(async move {
            calls
                .lock()
                .unwrap()
                .push(format!("update_rpe:{workout_id}"));
            let mut summaries = summaries.lock().unwrap();
            let Some(summary) = summaries.get_mut(&(user_id, workout_id)) else {
                return Err(WorkoutSummaryError::NotFound);
            };
            summary.rpe = Some(rpe);
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
    ) -> BoxFuture<Result<(), WorkoutSummaryError>> {
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        let summaries = self.summaries.clone();
        let calls = self.calls.clone();
        Box::pin(async move {
            calls.lock().unwrap().push(format!(
                "set_saved_state:{workout_id}:{saved_at_epoch_seconds:?}"
            ));
            let mut summaries = summaries.lock().unwrap();
            let Some(summary) = summaries.get_mut(&(user_id, workout_id)) else {
                return Err(WorkoutSummaryError::NotFound);
            };
            summary.saved_at_epoch_seconds = saved_at_epoch_seconds;
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
    ) -> BoxFuture<Result<(), WorkoutSummaryError>> {
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        let summaries = self.summaries.clone();
        let calls = self.calls.clone();
        Box::pin(async move {
            calls.lock().unwrap().push(format!(
                "append_message:{}:{}",
                workout_id,
                match message.role {
                    MessageRole::User => "user",
                    MessageRole::Coach => "coach",
                }
            ));
            let mut summaries = summaries.lock().unwrap();
            let Some(summary) = summaries.get_mut(&(user_id, workout_id)) else {
                return Err(WorkoutSummaryError::NotFound);
            };
            summary.messages.push(message);
            summary.updated_at_epoch_seconds = updated_at_epoch_seconds;
            Ok(())
        })
    }

    fn find_message_by_id(
        &self,
        user_id: &str,
        workout_id: &str,
        message_id: &str,
    ) -> BoxFuture<Result<Option<ConversationMessage>, WorkoutSummaryError>> {
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        let message_id = message_id.to_string();
        let summaries = self.summaries.clone();
        Box::pin(async move {
            Ok(summaries
                .lock()
                .unwrap()
                .get(&(user_id, workout_id))
                .and_then(|summary| {
                    summary
                        .messages
                        .iter()
                        .rev()
                        .find(|message| message.id == message_id)
                        .cloned()
                }))
        })
    }
}

#[derive(Clone, Default)]
pub(crate) struct InMemoryCoachReplyOperationRepository {
    operations: Arc<Mutex<ReplyOperationStore>>,
    calls: Arc<Mutex<Vec<String>>>,
}

impl InMemoryCoachReplyOperationRepository {
    pub(crate) fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }

    pub(crate) fn seed(&self, operation: CoachReplyOperation) {
        self.calls.lock().unwrap().push(format!(
            "seed:{}:{}:{:?}",
            operation.workout_id, operation.user_message_id, operation.status
        ));
        self.operations.lock().unwrap().insert(
            (
                operation.user_id.clone(),
                operation.workout_id.clone(),
                operation.user_message_id.clone(),
            ),
            operation,
        );
    }
}

impl CoachReplyOperationRepository for InMemoryCoachReplyOperationRepository {
    fn find_by_user_message_id(
        &self,
        user_id: &str,
        workout_id: &str,
        user_message_id: &str,
    ) -> BoxFuture<Result<Option<CoachReplyOperation>, WorkoutSummaryError>> {
        let key = (
            user_id.to_string(),
            workout_id.to_string(),
            user_message_id.to_string(),
        );
        let operations = self.operations.clone();
        Box::pin(async move { Ok(operations.lock().unwrap().get(&key).cloned()) })
    }

    fn claim_pending(
        &self,
        operation: CoachReplyOperation,
    ) -> BoxFuture<Result<CoachReplyClaimResult, WorkoutSummaryError>> {
        let operations = self.operations.clone();
        let calls = self.calls.clone();
        Box::pin(async move {
            calls.lock().unwrap().push(format!(
                "claim_pending:{}:{}",
                operation.workout_id, operation.user_message_id
            ));
            let key = (
                operation.user_id.clone(),
                operation.workout_id.clone(),
                operation.user_message_id.clone(),
            );
            let mut operations = operations.lock().unwrap();
            if let Some(existing) = operations.get(&key).cloned() {
                if existing.status == CoachReplyOperationStatus::Failed {
                    operations.insert(key, operation.clone());
                    return Ok(CoachReplyClaimResult::Claimed(operation));
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
    ) -> BoxFuture<Result<CoachReplyOperation, WorkoutSummaryError>> {
        let operations = self.operations.clone();
        let calls = self.calls.clone();
        Box::pin(async move {
            calls.lock().unwrap().push(format!(
                "upsert:{}:{}:{:?}",
                operation.workout_id, operation.user_message_id, operation.status
            ));
            operations.lock().unwrap().insert(
                (
                    operation.user_id.clone(),
                    operation.workout_id.clone(),
                    operation.user_message_id.clone(),
                ),
                operation.clone(),
            );
            Ok(operation)
        })
    }
}

pub(crate) fn test_service(
    repository: InMemoryWorkoutSummaryRepository,
) -> WorkoutSummaryService<
    InMemoryWorkoutSummaryRepository,
    InMemoryCoachReplyOperationRepository,
    TestClock,
    TestIdGenerator,
> {
    WorkoutSummaryService::new(
        repository,
        InMemoryCoachReplyOperationRepository::default(),
        TestClock,
        TestIdGenerator::default(),
    )
}

pub(crate) fn test_service_with_coach(
    repository: InMemoryWorkoutSummaryRepository,
    reply_operations: InMemoryCoachReplyOperationRepository,
    coach: Arc<dyn WorkoutCoach>,
) -> WorkoutSummaryService<
    InMemoryWorkoutSummaryRepository,
    InMemoryCoachReplyOperationRepository,
    TestClock,
    TestIdGenerator,
> {
    WorkoutSummaryService::with_coach(
        repository,
        reply_operations,
        TestClock,
        TestIdGenerator::default(),
        coach,
    )
}

pub(crate) fn default_dev_coach() -> Arc<dyn WorkoutCoach> {
    Arc::new(DevWorkoutCoach)
}

pub(crate) fn existing_summary() -> WorkoutSummary {
    WorkoutSummary {
        id: "summary-1".to_string(),
        user_id: "user-1".to_string(),
        workout_id: "workout-1".to_string(),
        rpe: Some(6),
        messages: Vec::new(),
        saved_at_epoch_seconds: None,
        created_at_epoch_seconds: 1_700_000_000,
        updated_at_epoch_seconds: 1_700_000_000,
    }
}

#[derive(Clone, Default)]
struct DevWorkoutCoach;

impl WorkoutCoach for DevWorkoutCoach {
    fn reply(
        &self,
        _user_id: &str,
        _summary: &WorkoutSummary,
        user_message: &str,
    ) -> aiwattcoach::domain::llm::BoxFuture<
        Result<aiwattcoach::domain::llm::LlmChatResponse, aiwattcoach::domain::llm::LlmError>,
    > {
        let message = format!(
            "Thanks, that helps. What stood out most about \"{user_message}\" during the workout?"
        );
        Box::pin(async move {
            Ok(LlmChatResponse {
                provider: LlmProvider::OpenAi,
                model: "dev-llm-coach".to_string(),
                message,
                provider_request_id: Some("dev-request-1".to_string()),
                usage: LlmTokenUsage::default(),
                cache: LlmCacheUsage::default(),
            })
        })
    }
}
