use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use aiwattcoach::domain::{
    athlete_summary::{
        AthleteSummary, AthleteSummaryError, AthleteSummaryGenerationClaimResult,
        AthleteSummaryGenerationOperation, AthleteSummaryGenerationOperationRepository,
        AthleteSummaryGenerationOperationStatus, AthleteSummaryGenerator, AthleteSummaryRepository,
        AthleteSummaryService, AthleteSummaryUseCases,
    },
    identity::Clock,
    llm::{LlmCacheUsage, LlmChatResponse, LlmError, LlmProvider, LlmTokenUsage},
};

const USER_ID: &str = "user-1";
const NOW_EPOCH_SECONDS: i64 = 1_775_564_800;
const THIS_WEEK_EPOCH_SECONDS: i64 = 1_775_520_000;
const LAST_WEEK_EPOCH_SECONDS: i64 = 1_775_347_200;
const MODEL: &str = "google/gemini-3-flash-preview";

type CallLog = Arc<Mutex<Vec<String>>>;

#[derive(Clone)]
struct FixedClock {
    now_epoch_seconds: i64,
}

impl Clock for FixedClock {
    fn now_epoch_seconds(&self) -> i64 {
        self.now_epoch_seconds
    }
}

#[derive(Clone)]
struct InMemoryAthleteSummaryRepository {
    summary: Arc<Mutex<Option<AthleteSummary>>>,
    find_calls: Arc<Mutex<u32>>,
    upsert_calls: Arc<Mutex<u32>>,
    call_log: CallLog,
}

impl Default for InMemoryAthleteSummaryRepository {
    fn default() -> Self {
        Self::new(new_call_log())
    }
}

impl InMemoryAthleteSummaryRepository {
    fn new(call_log: CallLog) -> Self {
        Self {
            summary: Arc::new(Mutex::new(None)),
            find_calls: Arc::new(Mutex::new(0)),
            upsert_calls: Arc::new(Mutex::new(0)),
            call_log,
        }
    }

    fn with_summary(call_log: CallLog, summary: AthleteSummary) -> Self {
        Self {
            summary: Arc::new(Mutex::new(Some(summary))),
            find_calls: Arc::new(Mutex::new(0)),
            upsert_calls: Arc::new(Mutex::new(0)),
            call_log,
        }
    }

    fn find_call_count(&self) -> u32 {
        *self.find_calls.lock().unwrap()
    }

    fn upsert_call_count(&self) -> u32 {
        *self.upsert_calls.lock().unwrap()
    }
}

impl AthleteSummaryRepository for InMemoryAthleteSummaryRepository {
    fn find_by_user_id(
        &self,
        _user_id: &str,
    ) -> aiwattcoach::domain::athlete_summary::BoxFuture<
        Result<Option<AthleteSummary>, AthleteSummaryError>,
    > {
        *self.find_calls.lock().unwrap() += 1;
        push_call(&self.call_log, "summary.find_by_user_id");
        let summary = self.summary.lock().unwrap().clone();
        Box::pin(async move { Ok(summary) })
    }

    fn upsert(
        &self,
        summary: AthleteSummary,
    ) -> aiwattcoach::domain::athlete_summary::BoxFuture<Result<AthleteSummary, AthleteSummaryError>>
    {
        *self.upsert_calls.lock().unwrap() += 1;
        push_call(&self.call_log, "summary.upsert");
        let store = self.summary.clone();
        Box::pin(async move {
            *store.lock().unwrap() = Some(summary.clone());
            Ok(summary)
        })
    }
}

#[derive(Clone)]
struct InMemoryAthleteSummaryOperationRepository {
    operation: Arc<Mutex<Option<AthleteSummaryGenerationOperation>>>,
    find_calls: Arc<Mutex<u32>>,
    claim_pending_calls: Arc<Mutex<u32>>,
    upsert_calls: Arc<Mutex<u32>>,
    call_log: CallLog,
}

impl Default for InMemoryAthleteSummaryOperationRepository {
    fn default() -> Self {
        Self::new(new_call_log())
    }
}

impl InMemoryAthleteSummaryOperationRepository {
    fn new(call_log: CallLog) -> Self {
        Self {
            operation: Arc::new(Mutex::new(None)),
            find_calls: Arc::new(Mutex::new(0)),
            claim_pending_calls: Arc::new(Mutex::new(0)),
            upsert_calls: Arc::new(Mutex::new(0)),
            call_log,
        }
    }

    fn with_operation(call_log: CallLog, operation: AthleteSummaryGenerationOperation) -> Self {
        Self {
            operation: Arc::new(Mutex::new(Some(operation))),
            find_calls: Arc::new(Mutex::new(0)),
            claim_pending_calls: Arc::new(Mutex::new(0)),
            upsert_calls: Arc::new(Mutex::new(0)),
            call_log,
        }
    }

    fn claim_pending_call_count(&self) -> u32 {
        *self.claim_pending_calls.lock().unwrap()
    }

    fn upsert_call_count(&self) -> u32 {
        *self.upsert_calls.lock().unwrap()
    }

    fn stored_operation(&self) -> Option<AthleteSummaryGenerationOperation> {
        self.operation.lock().unwrap().clone()
    }
}

impl AthleteSummaryGenerationOperationRepository for InMemoryAthleteSummaryOperationRepository {
    fn find_by_user_id(
        &self,
        _user_id: &str,
    ) -> aiwattcoach::domain::athlete_summary::BoxFuture<
        Result<Option<AthleteSummaryGenerationOperation>, AthleteSummaryError>,
    > {
        *self.find_calls.lock().unwrap() += 1;
        push_call(&self.call_log, "operation.find_by_user_id");
        let operation = self.operation.lock().unwrap().clone();
        Box::pin(async move { Ok(operation) })
    }

    fn claim_pending(
        &self,
        operation: AthleteSummaryGenerationOperation,
        stale_before_epoch_seconds: i64,
    ) -> aiwattcoach::domain::athlete_summary::BoxFuture<
        Result<AthleteSummaryGenerationClaimResult, AthleteSummaryError>,
    > {
        *self.claim_pending_calls.lock().unwrap() += 1;
        push_call(&self.call_log, "operation.claim_pending");

        let mut stored_operation = self.operation.lock().unwrap();
        let claim_result = match stored_operation.clone() {
            None => {
                *stored_operation = Some(operation.clone());
                AthleteSummaryGenerationClaimResult::Claimed(operation)
            }
            Some(existing)
                if existing.status == AthleteSummaryGenerationOperationStatus::Failed
                    || (existing.status == AthleteSummaryGenerationOperationStatus::Pending
                        && existing.last_attempt_at_epoch_seconds
                            <= stale_before_epoch_seconds) =>
            {
                let reclaimed = reclaim_operation(&existing, &operation);
                *stored_operation = Some(reclaimed.clone());
                AthleteSummaryGenerationClaimResult::Claimed(reclaimed)
            }
            Some(existing) => AthleteSummaryGenerationClaimResult::Existing(existing),
        };

        Box::pin(async move { Ok(claim_result) })
    }

    fn upsert(
        &self,
        operation: AthleteSummaryGenerationOperation,
    ) -> aiwattcoach::domain::athlete_summary::BoxFuture<
        Result<AthleteSummaryGenerationOperation, AthleteSummaryError>,
    > {
        *self.upsert_calls.lock().unwrap() += 1;
        push_call(&self.call_log, "operation.upsert");
        let store = self.operation.clone();
        Box::pin(async move {
            *store.lock().unwrap() = Some(operation.clone());
            Ok(operation)
        })
    }
}

#[derive(Clone)]
struct StubGenerator {
    calls: Arc<Mutex<u32>>,
    responses: Arc<Mutex<VecDeque<Result<LlmChatResponse, LlmError>>>>,
    call_log: CallLog,
}

impl StubGenerator {
    fn succeeds_with(call_log: CallLog, message: &str) -> Self {
        Self {
            calls: Arc::new(Mutex::new(0)),
            responses: Arc::new(Mutex::new(VecDeque::from([Ok(llm_response(message))]))),
            call_log,
        }
    }

    fn call_count(&self) -> u32 {
        *self.calls.lock().unwrap()
    }
}

#[derive(Clone)]
struct FailingUpsertAthleteSummaryRepository {
    error_message: String,
}

impl FailingUpsertAthleteSummaryRepository {
    fn new(error_message: &str) -> Self {
        Self {
            error_message: error_message.to_string(),
        }
    }
}

impl AthleteSummaryRepository for FailingUpsertAthleteSummaryRepository {
    fn find_by_user_id(
        &self,
        _user_id: &str,
    ) -> aiwattcoach::domain::athlete_summary::BoxFuture<
        Result<Option<AthleteSummary>, AthleteSummaryError>,
    > {
        Box::pin(async { Ok(None) })
    }

    fn upsert(
        &self,
        _summary: AthleteSummary,
    ) -> aiwattcoach::domain::athlete_summary::BoxFuture<Result<AthleteSummary, AthleteSummaryError>>
    {
        let error_message = self.error_message.clone();
        Box::pin(async move { Err(AthleteSummaryError::Repository(error_message)) })
    }
}

#[derive(Clone)]
struct FailingUpsertAthleteSummaryOperationRepository {
    operation: Arc<Mutex<Option<AthleteSummaryGenerationOperation>>>,
    error_message: String,
}

impl FailingUpsertAthleteSummaryOperationRepository {
    fn new(error_message: &str) -> Self {
        Self {
            operation: Arc::new(Mutex::new(None)),
            error_message: error_message.to_string(),
        }
    }
}

impl AthleteSummaryGenerationOperationRepository
    for FailingUpsertAthleteSummaryOperationRepository
{
    fn find_by_user_id(
        &self,
        _user_id: &str,
    ) -> aiwattcoach::domain::athlete_summary::BoxFuture<
        Result<Option<AthleteSummaryGenerationOperation>, AthleteSummaryError>,
    > {
        let operation = self.operation.lock().unwrap().clone();
        Box::pin(async move { Ok(operation) })
    }

    fn claim_pending(
        &self,
        operation: AthleteSummaryGenerationOperation,
        _stale_before_epoch_seconds: i64,
    ) -> aiwattcoach::domain::athlete_summary::BoxFuture<
        Result<AthleteSummaryGenerationClaimResult, AthleteSummaryError>,
    > {
        let store = self.operation.clone();
        Box::pin(async move {
            *store.lock().unwrap() = Some(operation.clone());
            Ok(AthleteSummaryGenerationClaimResult::Claimed(operation))
        })
    }

    fn upsert(
        &self,
        _operation: AthleteSummaryGenerationOperation,
    ) -> aiwattcoach::domain::athlete_summary::BoxFuture<
        Result<AthleteSummaryGenerationOperation, AthleteSummaryError>,
    > {
        let error_message = self.error_message.clone();
        Box::pin(async move { Err(AthleteSummaryError::Repository(error_message)) })
    }
}

impl AthleteSummaryGenerator for StubGenerator {
    fn generate(
        &self,
        _user_id: &str,
    ) -> aiwattcoach::domain::athlete_summary::BoxFuture<Result<LlmChatResponse, LlmError>> {
        *self.calls.lock().unwrap() += 1;
        push_call(&self.call_log, "generator.generate");
        let response = self
            .responses
            .lock()
            .unwrap()
            .pop_front()
            .expect("expected queued generator response");
        Box::pin(async move { response })
    }
}

#[tokio::test]
async fn ensure_fresh_summary_generates_when_missing() {
    let call_log = new_call_log();
    let repository = InMemoryAthleteSummaryRepository::new(call_log.clone());
    let operations = InMemoryAthleteSummaryOperationRepository::new(call_log.clone());
    let generator = StubGenerator::succeeds_with(call_log, "fresh summary");
    let service = AthleteSummaryService::new(
        repository.clone(),
        operations,
        generator.clone(),
        FixedClock {
            now_epoch_seconds: NOW_EPOCH_SECONDS,
        },
    );

    let summary = service.ensure_fresh_summary(USER_ID).await.unwrap();

    assert_eq!(summary.summary_text, "fresh summary");
    assert_eq!(generator.call_count(), 1);
    assert_eq!(repository.upsert_call_count(), 1);
}

#[tokio::test]
async fn ensure_fresh_summary_reuses_summary_generated_this_week() {
    let call_log = new_call_log();
    let repository = InMemoryAthleteSummaryRepository::with_summary(
        call_log.clone(),
        summary("existing", THIS_WEEK_EPOCH_SECONDS),
    );
    let operations = InMemoryAthleteSummaryOperationRepository::new(call_log.clone());
    let generator = StubGenerator::succeeds_with(call_log, "fresh summary");
    let service = AthleteSummaryService::new(
        repository,
        operations,
        generator.clone(),
        FixedClock {
            now_epoch_seconds: NOW_EPOCH_SECONDS,
        },
    );

    let summary = service.ensure_fresh_summary(USER_ID).await.unwrap();

    assert_eq!(summary.summary_text, "existing");
    assert_eq!(generator.call_count(), 0);
}

#[tokio::test]
async fn ensure_fresh_summary_regenerates_when_older_than_monday() {
    let call_log = new_call_log();
    let repository = InMemoryAthleteSummaryRepository::with_summary(
        call_log.clone(),
        summary("old", LAST_WEEK_EPOCH_SECONDS),
    );
    let operations = InMemoryAthleteSummaryOperationRepository::new(call_log.clone());
    let generator = StubGenerator::succeeds_with(call_log, "fresh summary");
    let service = AthleteSummaryService::new(
        repository.clone(),
        operations,
        generator.clone(),
        FixedClock {
            now_epoch_seconds: NOW_EPOCH_SECONDS,
        },
    );

    let summary = service.ensure_fresh_summary(USER_ID).await.unwrap();

    assert_eq!(summary.summary_text, "fresh summary");
    assert_eq!(generator.call_count(), 1);
    assert_eq!(repository.upsert_call_count(), 1);
}

#[tokio::test]
async fn generate_summary_force_true_regenerates_even_when_fresh() {
    let call_log = new_call_log();
    let repository = InMemoryAthleteSummaryRepository::with_summary(
        call_log.clone(),
        summary("existing", THIS_WEEK_EPOCH_SECONDS),
    );
    let operations = InMemoryAthleteSummaryOperationRepository::new(call_log.clone());
    let generator = StubGenerator::succeeds_with(call_log, "forced summary");
    let service = AthleteSummaryService::new(
        repository.clone(),
        operations,
        generator.clone(),
        FixedClock {
            now_epoch_seconds: NOW_EPOCH_SECONDS,
        },
    );

    let summary = service.generate_summary(USER_ID, true).await.unwrap();

    assert_eq!(summary.summary_text, "forced summary");
    assert_eq!(generator.call_count(), 1);
    assert_eq!(repository.upsert_call_count(), 1);
}

#[tokio::test]
async fn ensure_fresh_summary_reads_repository_once_when_summary_is_fresh() {
    let call_log = new_call_log();
    let repository = InMemoryAthleteSummaryRepository::with_summary(
        call_log.clone(),
        summary("existing", THIS_WEEK_EPOCH_SECONDS),
    );
    let operations = InMemoryAthleteSummaryOperationRepository::new(call_log.clone());
    let generator = StubGenerator::succeeds_with(call_log, "fresh summary");
    let service = AthleteSummaryService::new(
        repository.clone(),
        operations,
        generator.clone(),
        FixedClock {
            now_epoch_seconds: NOW_EPOCH_SECONDS,
        },
    );

    let summary = service.ensure_fresh_summary(USER_ID).await.unwrap();

    assert_eq!(summary.summary_text, "existing");
    assert_eq!(generator.call_count(), 0);
    assert_eq!(repository.find_call_count(), 1);
}

#[tokio::test]
async fn generate_summary_when_missing_claims_pending_operation_before_calling_generator() {
    let call_log = new_call_log();
    let repository = InMemoryAthleteSummaryRepository::new(call_log.clone());
    let operations = InMemoryAthleteSummaryOperationRepository::new(call_log.clone());
    let generator = StubGenerator::succeeds_with(call_log.clone(), "fresh summary");
    let service = AthleteSummaryService::new(
        repository.clone(),
        operations.clone(),
        generator.clone(),
        FixedClock {
            now_epoch_seconds: NOW_EPOCH_SECONDS,
        },
    );

    let summary = service.generate_summary(USER_ID, false).await.unwrap();

    assert_eq!(summary.summary_text, "fresh summary");
    assert_eq!(operations.claim_pending_call_count(), 1);
    assert_eq!(operations.upsert_call_count(), 1);
    assert_event_order(
        &recorded_calls(&call_log),
        "operation.claim_pending",
        "generator.generate",
    );

    let operation = operations.stored_operation().unwrap();
    assert_eq!(
        operation.status,
        AthleteSummaryGenerationOperationStatus::Completed
    );
    assert_eq!(operation.summary_text.as_deref(), Some("fresh summary"));
}

#[tokio::test]
async fn generate_summary_non_force_reuses_completed_operation_with_persisted_summary_without_second_generator_call(
) {
    let call_log = new_call_log();
    let repository = InMemoryAthleteSummaryRepository::with_summary(
        call_log.clone(),
        summary("persisted summary", THIS_WEEK_EPOCH_SECONDS),
    );
    let operations = InMemoryAthleteSummaryOperationRepository::with_operation(
        call_log.clone(),
        completed_operation("persisted summary"),
    );
    let generator = StubGenerator::succeeds_with(call_log, "new summary");
    let service = AthleteSummaryService::new(
        repository,
        operations.clone(),
        generator.clone(),
        FixedClock {
            now_epoch_seconds: NOW_EPOCH_SECONDS,
        },
    );

    let summary = service.generate_summary(USER_ID, false).await.unwrap();

    assert_eq!(summary.summary_text, "persisted summary");
    assert_eq!(generator.call_count(), 0);
    assert_eq!(operations.claim_pending_call_count(), 0);
    assert_eq!(operations.upsert_call_count(), 0);
}

#[tokio::test]
async fn generate_summary_force_true_ignores_completed_operation_and_regenerates() {
    let call_log = new_call_log();
    let repository = InMemoryAthleteSummaryRepository::with_summary(
        call_log.clone(),
        summary("persisted summary", THIS_WEEK_EPOCH_SECONDS),
    );
    let operations = InMemoryAthleteSummaryOperationRepository::with_operation(
        call_log.clone(),
        completed_operation("persisted summary"),
    );
    let generator = StubGenerator::succeeds_with(call_log, "forced refresh summary");
    let service = AthleteSummaryService::new(
        repository.clone(),
        operations.clone(),
        generator.clone(),
        FixedClock {
            now_epoch_seconds: NOW_EPOCH_SECONDS,
        },
    );

    let summary = service.generate_summary(USER_ID, true).await.unwrap();

    assert_eq!(summary.summary_text, "forced refresh summary");
    assert_eq!(generator.call_count(), 1);
    assert_eq!(repository.upsert_call_count(), 1);
    assert_eq!(operations.upsert_call_count(), 1);
}

#[tokio::test]
async fn generate_summary_reclaims_failed_operation_and_retries() {
    let call_log = new_call_log();
    let repository = InMemoryAthleteSummaryRepository::new(call_log.clone());
    let operations = InMemoryAthleteSummaryOperationRepository::with_operation(
        call_log.clone(),
        failed_operation("provider timed out", NOW_EPOCH_SECONDS - 60, 1),
    );
    let generator = StubGenerator::succeeds_with(call_log, "retried summary");
    let service = AthleteSummaryService::new(
        repository,
        operations.clone(),
        generator.clone(),
        FixedClock {
            now_epoch_seconds: NOW_EPOCH_SECONDS,
        },
    );

    let summary = service.generate_summary(USER_ID, true).await.unwrap();

    assert_eq!(summary.summary_text, "retried summary");
    assert_eq!(generator.call_count(), 1);
    assert_eq!(operations.claim_pending_call_count(), 1);

    let operation = operations.stored_operation().unwrap();
    assert_eq!(
        operation.status,
        AthleteSummaryGenerationOperationStatus::Completed
    );
    assert_eq!(operation.attempt_count, 2);
}

#[tokio::test]
async fn generate_summary_reclaims_stale_pending_operation() {
    let call_log = new_call_log();
    let repository = InMemoryAthleteSummaryRepository::new(call_log.clone());
    let operations = InMemoryAthleteSummaryOperationRepository::with_operation(
        call_log.clone(),
        pending_operation(NOW_EPOCH_SECONDS - 86_400, 1),
    );
    let generator = StubGenerator::succeeds_with(call_log, "fresh after reclaim");
    let service = AthleteSummaryService::new(
        repository,
        operations.clone(),
        generator.clone(),
        FixedClock {
            now_epoch_seconds: NOW_EPOCH_SECONDS,
        },
    );

    let summary = service.generate_summary(USER_ID, true).await.unwrap();

    assert_eq!(summary.summary_text, "fresh after reclaim");
    assert_eq!(generator.call_count(), 1);

    let operation = operations.stored_operation().unwrap();
    assert_eq!(
        operation.status,
        AthleteSummaryGenerationOperationStatus::Completed
    );
    assert_eq!(operation.attempt_count, 2);
}

#[tokio::test]
async fn generate_summary_recovers_from_completed_operation_record_without_regenerating() {
    let call_log = new_call_log();
    let repository = InMemoryAthleteSummaryRepository::new(call_log.clone());
    let operations = InMemoryAthleteSummaryOperationRepository::with_operation(
        call_log.clone(),
        completed_operation("recovered from operation"),
    );
    let generator = StubGenerator::succeeds_with(call_log, "should not run");
    let service = AthleteSummaryService::new(
        repository.clone(),
        operations.clone(),
        generator.clone(),
        FixedClock {
            now_epoch_seconds: NOW_EPOCH_SECONDS,
        },
    );

    let summary = service.generate_summary(USER_ID, false).await.unwrap();

    assert_eq!(summary.summary_text, "recovered from operation");
    assert_eq!(generator.call_count(), 0);
    assert_eq!(repository.upsert_call_count(), 1);
    assert_eq!(operations.claim_pending_call_count(), 1);
    assert_eq!(operations.upsert_call_count(), 0);
}

#[tokio::test]
async fn generate_summary_ignores_stale_completed_operation_when_persisted_summary_is_missing() {
    let call_log = new_call_log();
    let repository = InMemoryAthleteSummaryRepository::new(call_log.clone());
    let operations = InMemoryAthleteSummaryOperationRepository::with_operation(
        call_log.clone(),
        completed_operation_at("stale completed", LAST_WEEK_EPOCH_SECONDS),
    );
    let generator = StubGenerator::succeeds_with(call_log, "fresh summary");
    let service = AthleteSummaryService::new(
        repository.clone(),
        operations.clone(),
        generator.clone(),
        FixedClock {
            now_epoch_seconds: NOW_EPOCH_SECONDS,
        },
    );

    let summary = service.generate_summary(USER_ID, false).await.unwrap();

    assert_eq!(summary.summary_text, "fresh summary");
    assert_eq!(generator.call_count(), 1);
    assert_eq!(repository.upsert_call_count(), 1);
    assert_eq!(operations.upsert_call_count(), 1);
}

#[tokio::test]
async fn generate_summary_returns_summary_write_error_when_completed_operation_write_also_fails() {
    let repository = FailingUpsertAthleteSummaryRepository::new("summary write failed");
    let operations = FailingUpsertAthleteSummaryOperationRepository::new("operation write failed");
    let generator = StubGenerator::succeeds_with(new_call_log(), "fresh summary");
    let service = AthleteSummaryService::new(
        repository,
        operations,
        generator,
        FixedClock {
            now_epoch_seconds: NOW_EPOCH_SECONDS,
        },
    );

    let error = service.generate_summary(USER_ID, false).await.unwrap_err();

    assert_eq!(
        error,
        AthleteSummaryError::Repository("summary write failed".to_string())
    );
}

fn new_call_log() -> CallLog {
    Arc::new(Mutex::new(Vec::new()))
}

fn push_call(call_log: &CallLog, call: &str) {
    call_log.lock().unwrap().push(call.to_string());
}

fn recorded_calls(call_log: &CallLog) -> Vec<String> {
    call_log.lock().unwrap().clone()
}

fn assert_event_order(calls: &[String], first: &str, second: &str) {
    let first_index = calls
        .iter()
        .position(|call| call == first)
        .unwrap_or_else(|| panic!("missing call: {first}"));
    let second_index = calls
        .iter()
        .position(|call| call == second)
        .unwrap_or_else(|| panic!("missing call: {second}"));

    assert!(
        first_index < second_index,
        "expected {first} before {second}, got {calls:?}"
    );
}

fn summary(summary_text: &str, generated_at_epoch_seconds: i64) -> AthleteSummary {
    AthleteSummary {
        user_id: USER_ID.to_string(),
        summary_text: summary_text.to_string(),
        generated_at_epoch_seconds,
        created_at_epoch_seconds: generated_at_epoch_seconds,
        updated_at_epoch_seconds: generated_at_epoch_seconds,
        provider: Some("openrouter".to_string()),
        model: Some(MODEL.to_string()),
    }
}

fn pending_operation(
    last_attempt_at_epoch_seconds: i64,
    attempt_count: u32,
) -> AthleteSummaryGenerationOperation {
    AthleteSummaryGenerationOperation {
        user_id: USER_ID.to_string(),
        status: AthleteSummaryGenerationOperationStatus::Pending,
        summary_text: None,
        provider: None,
        model: None,
        error_message: None,
        started_at_epoch_seconds: LAST_WEEK_EPOCH_SECONDS,
        last_attempt_at_epoch_seconds,
        attempt_count,
        created_at_epoch_seconds: LAST_WEEK_EPOCH_SECONDS,
        updated_at_epoch_seconds: last_attempt_at_epoch_seconds,
    }
}

fn failed_operation(
    error_message: &str,
    last_attempt_at_epoch_seconds: i64,
    attempt_count: u32,
) -> AthleteSummaryGenerationOperation {
    AthleteSummaryGenerationOperation {
        user_id: USER_ID.to_string(),
        status: AthleteSummaryGenerationOperationStatus::Failed,
        summary_text: None,
        provider: None,
        model: None,
        error_message: Some(error_message.to_string()),
        started_at_epoch_seconds: LAST_WEEK_EPOCH_SECONDS,
        last_attempt_at_epoch_seconds,
        attempt_count,
        created_at_epoch_seconds: LAST_WEEK_EPOCH_SECONDS,
        updated_at_epoch_seconds: last_attempt_at_epoch_seconds,
    }
}

fn completed_operation(summary_text: &str) -> AthleteSummaryGenerationOperation {
    completed_operation_at(summary_text, THIS_WEEK_EPOCH_SECONDS)
}

fn completed_operation_at(
    summary_text: &str,
    updated_at_epoch_seconds: i64,
) -> AthleteSummaryGenerationOperation {
    AthleteSummaryGenerationOperation {
        user_id: USER_ID.to_string(),
        status: AthleteSummaryGenerationOperationStatus::Completed,
        summary_text: Some(summary_text.to_string()),
        provider: Some("openrouter".to_string()),
        model: Some(MODEL.to_string()),
        error_message: None,
        started_at_epoch_seconds: updated_at_epoch_seconds,
        last_attempt_at_epoch_seconds: updated_at_epoch_seconds,
        attempt_count: 1,
        created_at_epoch_seconds: updated_at_epoch_seconds,
        updated_at_epoch_seconds,
    }
}

fn reclaim_operation(
    existing: &AthleteSummaryGenerationOperation,
    pending: &AthleteSummaryGenerationOperation,
) -> AthleteSummaryGenerationOperation {
    AthleteSummaryGenerationOperation {
        user_id: existing.user_id.clone(),
        status: AthleteSummaryGenerationOperationStatus::Pending,
        summary_text: existing.summary_text.clone(),
        provider: existing.provider.clone(),
        model: existing.model.clone(),
        error_message: None,
        started_at_epoch_seconds: existing.started_at_epoch_seconds,
        last_attempt_at_epoch_seconds: pending.last_attempt_at_epoch_seconds,
        attempt_count: existing.attempt_count.saturating_add(1),
        created_at_epoch_seconds: existing.created_at_epoch_seconds,
        updated_at_epoch_seconds: pending.updated_at_epoch_seconds,
    }
}

fn llm_response(message: &str) -> LlmChatResponse {
    LlmChatResponse {
        provider: LlmProvider::OpenRouter,
        model: MODEL.to_string(),
        message: message.to_string(),
        provider_request_id: None,
        usage: LlmTokenUsage::default(),
        cache: LlmCacheUsage::default(),
    }
}
