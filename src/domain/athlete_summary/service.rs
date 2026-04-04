use chrono::{Datelike, TimeZone, Utc, Weekday};

use crate::domain::identity::Clock;

use super::{
    AthleteSummary, AthleteSummaryError, AthleteSummaryGenerator, AthleteSummaryRepository,
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
pub struct AthleteSummaryService<Repo, Generator, Time>
where
    Repo: AthleteSummaryRepository + Clone,
    Generator: AthleteSummaryGenerator + Clone,
    Time: Clock + Clone,
{
    repository: Repo,
    generator: Generator,
    clock: Time,
}

impl<Repo, Generator, Time> AthleteSummaryService<Repo, Generator, Time>
where
    Repo: AthleteSummaryRepository + Clone,
    Generator: AthleteSummaryGenerator + Clone,
    Time: Clock + Clone,
{
    pub fn new(repository: Repo, generator: Generator, clock: Time) -> Self {
        Self {
            repository,
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

    async fn generate_and_upsert_summary(
        &self,
        user_id: String,
        created_at_epoch_seconds: i64,
    ) -> Result<AthleteSummary, AthleteSummaryError> {
        let response = self
            .generator
            .generate(&user_id)
            .await
            .map_err(AthleteSummaryError::Llm)?;
        let now = self.clock.now_epoch_seconds();

        self.repository
            .upsert(AthleteSummary {
                user_id,
                summary_text: response.message,
                generated_at_epoch_seconds: now,
                created_at_epoch_seconds,
                updated_at_epoch_seconds: now,
                provider: Some(response.provider.to_string()),
                model: Some(response.model),
            })
            .await
    }
}

impl<Repo, Generator, Time> AthleteSummaryUseCases for AthleteSummaryService<Repo, Generator, Time>
where
    Repo: AthleteSummaryRepository + Clone + 'static,
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

            let created_at = existing
                .as_ref()
                .map(|summary| summary.created_at_epoch_seconds)
                .unwrap_or_else(|| service.clock.now_epoch_seconds());

            service
                .generate_and_upsert_summary(user_id, created_at)
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

                let updated = service
                    .generate_and_upsert_summary(user_id, existing.created_at_epoch_seconds)
                    .await?;

                return Ok(EnsuredAthleteSummary {
                    summary: updated,
                    was_regenerated: true,
                });
            }

            let now = service.clock.now_epoch_seconds();
            let created = service.generate_and_upsert_summary(user_id, now).await?;

            Ok(EnsuredAthleteSummary {
                summary: created,
                was_regenerated: true,
            })
        })
    }
}
