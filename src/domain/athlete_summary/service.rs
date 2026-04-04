use chrono::{Datelike, TimeZone, Utc, Weekday};

use crate::domain::identity::Clock;

use super::{
    AthleteSummary, AthleteSummaryError, AthleteSummaryGenerationClaimResult,
    AthleteSummaryGenerationOperation, AthleteSummaryGenerationOperationRepository,
    AthleteSummaryGenerationOperationStatus, AthleteSummaryGenerator, AthleteSummaryRepository,
    AthleteSummaryState, EnsuredAthleteSummary,
};

pub trait AthleteSummaryUseCases: Send + Sync {
    fn get_summary_state(
        &self,
        user_id: &str,
    ) -> super::BoxFuture<Result<AthleteSummaryState, AthleteSummaryError>>;

    fn generate_summary(
        &self,
        user_id: &str,
        force: bool,
    ) -> super::BoxFuture<Result<AthleteSummary, AthleteSummaryError>>;

    fn ensure_fresh_summary(
        &self,
        user_id: &str,
    ) -> super::BoxFuture<Result<AthleteSummary, AthleteSummaryError>>;

    fn ensure_fresh_summary_state(
        &self,
        user_id: &str,
    ) -> super::BoxFuture<Result<EnsuredAthleteSummary, AthleteSummaryError>>;
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
    const STALE_PENDING_TIMEOUT_SECONDS: i64 = 300;

    pub fn new(repository: Repo, operations: Ops, generator: Generator, clock: Time) -> Self {
        Self {
            repository,
            operations,
            generator,
            clock,
        }
    }

    fn current_week_monday_epoch_seconds(&self) -> i64 {
        let now = self.clock.now_epoch_seconds();
        let Some(now) = Utc.timestamp_opt(now, 0).single() else {
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

    fn is_stale(&self, summary: &AthleteSummary) -> bool {
        summary.generated_at_epoch_seconds < self.current_week_monday_epoch_seconds()
    }

    fn stale_pending_before_epoch_seconds(&self) -> i64 {
        self.clock.now_epoch_seconds() - Self::STALE_PENDING_TIMEOUT_SECONDS
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
        let summary = self.build_summary(SummaryRecord {
            user_id: operation.user_id.clone(),
            summary_text: response.message.clone(),
            created_at_epoch_seconds,
            generated_at_epoch_seconds: now,
            updated_at_epoch_seconds: now,
            provider: Some(response.provider.to_string()),
            model: Some(response.model.clone()),
        });
        let completed = self.completed_operation(
            &operation,
            response.message,
            response.provider.to_string(),
            response.model,
            now,
        );

        match self.repository.upsert(summary).await {
            Ok(summary) => {
                self.operations.upsert(completed).await?;
                Ok(summary)
            }
            Err(error) => {
                let _ = self.operations.upsert(completed).await;
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
    ) -> super::BoxFuture<Result<AthleteSummaryState, AthleteSummaryError>> {
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
    ) -> super::BoxFuture<Result<AthleteSummary, AthleteSummaryError>> {
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
                            if let Some(summary) = service
                                .recover_completed_operation(existing.as_ref(), &operation)
                                .await?
                            {
                                return Ok(summary);
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
                                "athlete summary generation is already pending".to_string(),
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
    ) -> super::BoxFuture<Result<AthleteSummary, AthleteSummaryError>> {
        self.generate_summary(user_id, false)
    }

    fn ensure_fresh_summary_state(
        &self,
        user_id: &str,
    ) -> super::BoxFuture<Result<EnsuredAthleteSummary, AthleteSummaryError>> {
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
