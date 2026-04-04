use std::{collections::HashMap, sync::Mutex};

use aiwattcoach::domain::athlete_summary::{
    AthleteSummary, AthleteSummaryError, AthleteSummaryState, AthleteSummaryUseCases,
    EnsuredAthleteSummary,
};

use super::app::BoxFuture;

pub(crate) struct TestAthleteSummaryService {
    summaries: Mutex<HashMap<String, AthleteSummary>>,
}

impl TestAthleteSummaryService {
    pub(crate) fn empty() -> Self {
        Self {
            summaries: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for TestAthleteSummaryService {
    fn default() -> Self {
        Self::empty()
    }
}

impl AthleteSummaryUseCases for TestAthleteSummaryService {
    fn get_summary_state(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<AthleteSummaryState, AthleteSummaryError>> {
        let summary = self.summaries.lock().unwrap().get(user_id).cloned();
        Box::pin(async move {
            Ok(AthleteSummaryState {
                stale: summary.is_none(),
                summary,
            })
        })
    }

    fn generate_summary(
        &self,
        user_id: &str,
        force: bool,
    ) -> BoxFuture<Result<AthleteSummary, AthleteSummaryError>> {
        let mut summaries = self.summaries.lock().unwrap();

        if !force {
            if let Some(existing) = summaries.get(user_id).cloned() {
                return Box::pin(async move { Ok(existing) });
            }
        }

        let existing = summaries.get(user_id).cloned();
        let generated_at_epoch_seconds = existing
            .as_ref()
            .map(|existing| existing.generated_at_epoch_seconds + 1)
            .unwrap_or(2_000);
        let generated = AthleteSummary {
            user_id: user_id.to_string(),
            summary_text: "OK".to_string(),
            generated_at_epoch_seconds,
            created_at_epoch_seconds: existing
                .as_ref()
                .map(|existing| existing.created_at_epoch_seconds)
                .unwrap_or(generated_at_epoch_seconds),
            updated_at_epoch_seconds: generated_at_epoch_seconds,
            provider: Some("openai".to_string()),
            model: Some("gpt-4o-mini".to_string()),
        };
        summaries.insert(user_id.to_string(), generated.clone());
        Box::pin(async move { Ok(generated) })
    }

    fn ensure_fresh_summary(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<AthleteSummary, AthleteSummaryError>> {
        self.generate_summary(user_id, false)
    }

    fn ensure_fresh_summary_state(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<EnsuredAthleteSummary, AthleteSummaryError>> {
        let existing = self.summaries.lock().unwrap().get(user_id).cloned();

        if let Some(existing) = existing {
            return Box::pin(async move {
                Ok(EnsuredAthleteSummary {
                    summary: existing,
                    was_regenerated: false,
                })
            });
        }

        let generated = self.generate_summary(user_id, false);
        Box::pin(async move {
            Ok(EnsuredAthleteSummary {
                summary: generated.await?,
                was_regenerated: true,
            })
        })
    }
}
