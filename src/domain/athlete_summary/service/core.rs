use chrono::{Datelike, TimeZone, Utc, Weekday};

use crate::domain::identity::Clock;

use super::super::{
    AthleteSummary, AthleteSummaryError, AthleteSummaryGenerationClaimResult,
    AthleteSummaryGenerationOperation, AthleteSummaryGenerationOperationRepository,
    AthleteSummaryGenerationOperationStatus, AthleteSummaryGenerator, AthleteSummaryRepository,
    AthleteSummaryState, EnsuredAthleteSummary,
};

pub(crate) const STALE_PENDING_TIMEOUT_SECONDS: i64 = 300;
pub(crate) const GENERATION_ALREADY_PENDING_MESSAGE: &str =
    "athlete summary generation is already pending";

pub(crate) fn current_week_monday_epoch_seconds(now_epoch_seconds: i64) -> i64 {
    let Some(now) = Utc.timestamp_opt(now_epoch_seconds, 0).single() else {
        return 0;
    };
    let date = now.date_naive();
    let offset = match date.weekday() {
        Weekday::Mon => 0,
        weekday => weekday.num_days_from_monday() as i64,
    };
    let monday = date - chrono::Duration::days(offset);
    monday
        .and_hms_opt(0, 0, 0)
        .map(|datetime| datetime.and_utc().timestamp())
        .unwrap_or(0)
}

pub trait AthleteSummaryUseCases: Send + Sync {
    fn get_summary_state(
        &self,
        user_id: &str,
    ) -> super::super::BoxFuture<Result<AthleteSummaryState, AthleteSummaryError>>;

    fn generate_summary(
        &self,
        user_id: &str,
        force: bool,
    ) -> super::super::BoxFuture<Result<AthleteSummary, AthleteSummaryError>>;

    fn ensure_fresh_summary(
        &self,
        user_id: &str,
    ) -> super::super::BoxFuture<Result<AthleteSummary, AthleteSummaryError>>;

    fn ensure_fresh_summary_state(
        &self,
        user_id: &str,
    ) -> super::super::BoxFuture<Result<EnsuredAthleteSummary, AthleteSummaryError>>;
}

#[derive(Clone)]
pub struct AthleteSummaryService<Repo, Ops, Generator, Time>
where
    Repo: AthleteSummaryRepository + Clone,
    Ops: AthleteSummaryGenerationOperationRepository + Clone,
    Generator: AthleteSummaryGenerator + Clone,
    Time: Clock + Clone,
{
    repository: Repo,
    operations: Ops,
    generator: Generator,
    clock: Time,
}

struct SummaryRecord {
    user_id: String,
    summary_text: String,
    created_at_epoch_seconds: i64,
    generated_at_epoch_seconds: i64,
    updated_at_epoch_seconds: i64,
    provider: Option<String>,
    model: Option<String>,
}

impl<Repo, Ops, Generator, Time> AthleteSummaryService<Repo, Ops, Generator, Time>
where
    Repo: AthleteSummaryRepository + Clone,
    Ops: AthleteSummaryGenerationOperationRepository + Clone,
    Generator: AthleteSummaryGenerator + Clone,
    Time: Clock + Clone,
{
    pub fn new(repository: Repo, operations: Ops, generator: Generator, clock: Time) -> Self {
        Self {
            repository,
            operations,
            generator,
            clock,
        }
    }

    pub(crate) fn current_week_monday_epoch_seconds(&self) -> i64 {
        current_week_monday_epoch_seconds(self.clock.now_epoch_seconds())
    }

    pub(crate) fn is_stale(&self, summary: &AthleteSummary) -> bool {
        summary.generated_at_epoch_seconds < self.current_week_monday_epoch_seconds()
    }

    fn stale_pending_before_epoch_seconds(&self) -> i64 {
        self.clock.now_epoch_seconds() - STALE_PENDING_TIMEOUT_SECONDS
    }

    fn pending_operation(
        &self,
        user_id: String,
        now_epoch_seconds: i64,
    ) -> AthleteSummaryGenerationOperation {
        AthleteSummaryGenerationOperation {
            user_id,
            status: AthleteSummaryGenerationOperationStatus::Pending,
            summary_text: None,
            provider: None,
            model: None,
            error_message: None,
            started_at_epoch_seconds: now_epoch_seconds,
            last_attempt_at_epoch_seconds: now_epoch_seconds,
            attempt_count: 1,
            created_at_epoch_seconds: now_epoch_seconds,
            updated_at_epoch_seconds: now_epoch_seconds,
        }
    }

    fn build_summary(&self, record: SummaryRecord) -> AthleteSummary {
        AthleteSummary {
            user_id: record.user_id,
            summary_text: record.summary_text,
            generated_at_epoch_seconds: record.generated_at_epoch_seconds,
            created_at_epoch_seconds: record.created_at_epoch_seconds,
            updated_at_epoch_seconds: record.updated_at_epoch_seconds,
            provider: record.provider,
            model: record.model,
        }
    }

    fn completed_operation(
        &self,
        operation: &AthleteSummaryGenerationOperation,
        summary_text: String,
        provider: String,
        model: String,
        updated_at_epoch_seconds: i64,
    ) -> AthleteSummaryGenerationOperation {
        AthleteSummaryGenerationOperation {
            user_id: operation.user_id.clone(),
            status: AthleteSummaryGenerationOperationStatus::Completed,
            summary_text: Some(summary_text),
            provider: Some(provider),
            model: Some(model),
            error_message: None,
            started_at_epoch_seconds: operation.started_at_epoch_seconds,
            last_attempt_at_epoch_seconds: operation.last_attempt_at_epoch_seconds,
            attempt_count: operation.attempt_count,
            created_at_epoch_seconds: operation.created_at_epoch_seconds,
            updated_at_epoch_seconds,
        }
    }

    fn failed_operation(
        &self,
        operation: &AthleteSummaryGenerationOperation,
        error_message: String,
        updated_at_epoch_seconds: i64,
    ) -> AthleteSummaryGenerationOperation {
        AthleteSummaryGenerationOperation {
            user_id: operation.user_id.clone(),
            status: AthleteSummaryGenerationOperationStatus::Failed,
            summary_text: operation.summary_text.clone(),
            provider: operation.provider.clone(),
            model: operation.model.clone(),
            error_message: Some(error_message),
            started_at_epoch_seconds: operation.started_at_epoch_seconds,
            last_attempt_at_epoch_seconds: operation.last_attempt_at_epoch_seconds,
            attempt_count: operation.attempt_count,
            created_at_epoch_seconds: operation.created_at_epoch_seconds,
            updated_at_epoch_seconds,
        }
    }

    async fn recover_completed_operation(
        &self,
        existing_summary: Option<&AthleteSummary>,
        operation: &AthleteSummaryGenerationOperation,
    ) -> Result<Option<AthleteSummary>, AthleteSummaryError> {
        if let Some(summary) = existing_summary {
            if !self.is_stale(summary) {
                return Ok(Some(summary.clone()));
            }
        }

        let recovered_summary = self.build_summary(SummaryRecord {
            user_id: operation.user_id.clone(),
            summary_text: operation.summary_text.clone().ok_or_else(|| {
                AthleteSummaryError::Repository(
                    "completed athlete summary generation operation missing stored summary"
                        .to_string(),
                )
            })?,
            created_at_epoch_seconds: existing_summary
                .map(|summary| summary.created_at_epoch_seconds)
                .unwrap_or(operation.created_at_epoch_seconds),
            generated_at_epoch_seconds: operation.updated_at_epoch_seconds,
            updated_at_epoch_seconds: operation.updated_at_epoch_seconds,
            provider: operation.provider.clone(),
            model: operation.model.clone(),
        });

        if self.is_stale(&recovered_summary) {
            return Ok(None);
        }

        self.repository.upsert(recovered_summary).await.map(Some)
    }

    async fn finalize_generated_summary(
        &self,
        existing_summary: Option<&AthleteSummary>,
        operation: AthleteSummaryGenerationOperation,
    ) -> Result<AthleteSummary, AthleteSummaryError> {
        let response = match self.generator.generate(&operation.user_id).await {
            Ok(response) => response,
            Err(error) => {
                let failed = self.failed_operation(
                    &operation,
                    error.to_string(),
                    self.clock.now_epoch_seconds(),
                );
                self.operations.upsert(failed).await?;
                return Err(AthleteSummaryError::Llm(error));
            }
        };
        let now = self.clock.now_epoch_seconds();
        let created_at_epoch_seconds = existing_summary
            .map(|summary| summary.created_at_epoch_seconds)
            .unwrap_or(now);
        let Some(summary_text) = response
            .assistant_text()
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_string)
        else {
            let error = crate::domain::llm::LlmError::InvalidResponse(
                "assistant summary missing final text".to_string(),
            );
            let failed = self.failed_operation(
                &operation,
                error.to_string(),
                self.clock.now_epoch_seconds(),
            );
            self.operations.upsert(failed).await?;
            return Err(AthleteSummaryError::Llm(error));
        };
        let provider = response.provider.to_string();
        let model = response.model.clone();
        let summary = self.build_summary(SummaryRecord {
            user_id: operation.user_id.clone(),
            summary_text: summary_text.clone(),
            created_at_epoch_seconds,
            generated_at_epoch_seconds: now,
            updated_at_epoch_seconds: now,
            provider: Some(provider.clone()),
            model: Some(model.clone()),
        });
        let completed = self.completed_operation(&operation, summary_text, provider, model, now);

        match self.repository.upsert(summary).await {
            Ok(summary) => {
                self.operations.upsert(completed).await?;
                Ok(summary)
            }
            Err(error) => {
                let failed = self.failed_operation(
                    &operation,
                    error.to_string(),
                    self.clock.now_epoch_seconds(),
                );
                self.operations.upsert(failed).await?;
                Err(error)
            }
        }
    }
}

impl<Repo, Ops, Generator, Time> AthleteSummaryUseCases
    for AthleteSummaryService<Repo, Ops, Generator, Time>
where
    Repo: AthleteSummaryRepository + Clone + 'static,
    Ops: AthleteSummaryGenerationOperationRepository + Clone + 'static,
    Generator: AthleteSummaryGenerator + Clone + 'static,
    Time: Clock + Clone + 'static,
{
    fn get_summary_state(
        &self,
        user_id: &str,
    ) -> super::super::BoxFuture<Result<AthleteSummaryState, AthleteSummaryError>> {
        let repository = self.repository.clone();
        let user_id = user_id.to_string();
        let service = self.clone();
        Box::pin(async move {
            let summary = repository.find_by_user_id(&user_id).await?;
            let stale = summary
                .as_ref()
                .is_none_or(|summary| service.is_stale(summary));
            Ok(AthleteSummaryState { summary, stale })
        })
    }

    fn generate_summary(
        &self,
        user_id: &str,
        force: bool,
    ) -> super::super::BoxFuture<Result<AthleteSummary, AthleteSummaryError>> {
        let user_id = user_id.to_string();
        let service = self.clone();
        Box::pin(async move {
            let existing = service.repository.find_by_user_id(&user_id).await?;

            if !force {
                if let Some(existing) = existing.as_ref() {
                    if !service.is_stale(existing) {
                        return Ok(existing.clone());
                    }
                }
            }

            let pending =
                service.pending_operation(user_id.clone(), service.clock.now_epoch_seconds());
            let operation = match service
                .operations
                .claim_pending(pending, service.stale_pending_before_epoch_seconds())
                .await?
            {
                AthleteSummaryGenerationClaimResult::Claimed(operation) => operation,
                AthleteSummaryGenerationClaimResult::Existing(operation) => {
                    match operation.status {
                        AthleteSummaryGenerationOperationStatus::Completed => {
                            if !force {
                                if let Some(summary) = service
                                    .recover_completed_operation(existing.as_ref(), &operation)
                                    .await?
                                {
                                    return Ok(summary);
                                }
                            }

                            operation
                        }
                        AthleteSummaryGenerationOperationStatus::Failed => {
                            return Err(AthleteSummaryError::Unavailable(
                                "athlete summary generation failed and could not be reclaimed"
                                    .to_string(),
                            ));
                        }
                        AthleteSummaryGenerationOperationStatus::Pending => {
                            return Err(AthleteSummaryError::Unavailable(
                                GENERATION_ALREADY_PENDING_MESSAGE.to_string(),
                            ));
                        }
                    }
                }
            };

            service
                .finalize_generated_summary(existing.as_ref(), operation)
                .await
        })
    }

    fn ensure_fresh_summary(
        &self,
        user_id: &str,
    ) -> super::super::BoxFuture<Result<AthleteSummary, AthleteSummaryError>> {
        self.generate_summary(user_id, false)
    }

    fn ensure_fresh_summary_state(
        &self,
        user_id: &str,
    ) -> super::super::BoxFuture<Result<EnsuredAthleteSummary, AthleteSummaryError>> {
        let user_id = user_id.to_string();
        let service = self.clone();
        Box::pin(async move {
            if let Some(existing) = service.repository.find_by_user_id(&user_id).await? {
                if !service.is_stale(&existing) {
                    return Ok(EnsuredAthleteSummary {
                        summary: existing,
                        was_regenerated: false,
                    });
                }

                let updated = service.generate_summary(&user_id, false).await?;

                return Ok(EnsuredAthleteSummary {
                    summary: updated,
                    was_regenerated: true,
                });
            }

            let created = service.generate_summary(&user_id, false).await?;

            Ok(EnsuredAthleteSummary {
                summary: created,
                was_regenerated: true,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use crate::domain::{
        athlete_summary::{
            AthleteSummary, AthleteSummaryError, AthleteSummaryGenerationClaimResult,
            AthleteSummaryGenerationOperation, AthleteSummaryGenerationOperationRepository,
            AthleteSummaryGenerationOperationStatus, AthleteSummaryGenerator,
            AthleteSummaryRepository, BoxFuture,
        },
        identity::Clock,
        llm::{
            LlmCacheUsage, LlmChatMessage, LlmChatResponse, LlmError, LlmProvider, LlmTokenUsage,
        },
    };

    use super::{AthleteSummaryService, AthleteSummaryUseCases};

    #[derive(Clone)]
    struct FixedClock {
        now_epoch_seconds: i64,
    }

    impl Clock for FixedClock {
        fn now_epoch_seconds(&self) -> i64 {
            self.now_epoch_seconds
        }
    }

    #[derive(Clone, Default)]
    struct FailingSummaryRepository;

    impl AthleteSummaryRepository for FailingSummaryRepository {
        fn find_by_user_id(
            &self,
            _user_id: &str,
        ) -> BoxFuture<Result<Option<AthleteSummary>, AthleteSummaryError>> {
            Box::pin(async { Ok(None) })
        }

        fn upsert(
            &self,
            _summary: AthleteSummary,
        ) -> BoxFuture<Result<AthleteSummary, AthleteSummaryError>> {
            Box::pin(async {
                Err(AthleteSummaryError::Repository(
                    "summary upsert failed".to_string(),
                ))
            })
        }
    }

    #[derive(Clone, Default)]
    struct RecordingOperationRepository {
        last_operation: Arc<Mutex<Option<AthleteSummaryGenerationOperation>>>,
    }

    impl AthleteSummaryGenerationOperationRepository for RecordingOperationRepository {
        fn find_by_user_id(
            &self,
            _user_id: &str,
        ) -> BoxFuture<Result<Option<AthleteSummaryGenerationOperation>, AthleteSummaryError>>
        {
            let operation = self.last_operation.lock().unwrap().clone();
            Box::pin(async move { Ok(operation) })
        }

        fn claim_pending(
            &self,
            operation: AthleteSummaryGenerationOperation,
            _stale_before_epoch_seconds: i64,
        ) -> BoxFuture<Result<AthleteSummaryGenerationClaimResult, AthleteSummaryError>> {
            let store = self.last_operation.clone();
            Box::pin(async move {
                *store.lock().unwrap() = Some(operation.clone());
                Ok(AthleteSummaryGenerationClaimResult::Claimed(operation))
            })
        }

        fn upsert(
            &self,
            operation: AthleteSummaryGenerationOperation,
        ) -> BoxFuture<Result<AthleteSummaryGenerationOperation, AthleteSummaryError>> {
            let store = self.last_operation.clone();
            Box::pin(async move {
                *store.lock().unwrap() = Some(operation.clone());
                Ok(operation)
            })
        }
    }

    #[derive(Clone)]
    struct FixedGenerator;

    impl AthleteSummaryGenerator for FixedGenerator {
        fn generate(&self, _user_id: &str) -> BoxFuture<Result<LlmChatResponse, LlmError>> {
            Box::pin(async {
                Ok(LlmChatResponse {
                    provider: LlmProvider::OpenRouter,
                    model: "test-model".to_string(),
                    message: LlmChatMessage::assistant("Weekly summary text"),
                    finish_reason: None,
                    provider_request_id: None,
                    usage: LlmTokenUsage::default(),
                    cache: LlmCacheUsage::default(),
                })
            })
        }
    }

    #[tokio::test]
    async fn persists_failed_operation_when_summary_upsert_fails() {
        let operations = RecordingOperationRepository::default();
        let service = AthleteSummaryService::new(
            FailingSummaryRepository,
            operations.clone(),
            FixedGenerator,
            FixedClock {
                now_epoch_seconds: 1_700_000_000,
            },
        );

        let error = service
            .generate_summary("user-1", true)
            .await
            .expect_err("summary upsert should fail");

        assert!(matches!(error, AthleteSummaryError::Repository(_)));

        let stored = operations
            .last_operation
            .lock()
            .unwrap()
            .clone()
            .expect("operation should be stored");

        assert_eq!(
            stored.status,
            AthleteSummaryGenerationOperationStatus::Failed
        );
        assert_eq!(
            stored.error_message.as_deref(),
            Some("summary upsert failed")
        );
    }
}
