use std::sync::Mutex;

use aiwattcoach::domain::athlete_summary::{
    AthleteSummary, AthleteSummaryError, AthleteSummaryState, AthleteSummaryUseCases,
    EnsuredAthleteSummary,
};

use super::app::BoxFuture;

pub(crate) struct TestAthleteSummaryService {
    summary: Mutex<Option<AthleteSummary>>,
}

impl TestAthleteSummaryService {
    pub(crate) fn empty() -> Self {
        Self {
            summary: Mutex::new(None),
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
        _user_id: &str,
    ) -> BoxFuture<Result<AthleteSummaryState, AthleteSummaryError>> {
        let summary = self.summary.lock().unwrap().clone();
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
        _force: bool,
    ) -> BoxFuture<Result<AthleteSummary, AthleteSummaryError>> {
        let mut summary = self.summary.lock().unwrap();
        let generated = AthleteSummary {
            user_id: user_id.to_string(),
            summary_text: "OK".to_string(),
            generated_at_epoch_seconds: 2_000,
            created_at_epoch_seconds: summary
                .as_ref()
                .map(|existing| existing.created_at_epoch_seconds)
                .unwrap_or(2_000),
            updated_at_epoch_seconds: 2_000,
            provider: Some("openai".to_string()),
            model: Some("gpt-4o-mini".to_string()),
        };
        *summary = Some(generated.clone());
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
        let mut summary = self.summary.lock().unwrap();
        let was_regenerated = summary.is_none();
        let generated = AthleteSummary {
            user_id: user_id.to_string(),
            summary_text: if was_regenerated {
                "OK (generated)".to_string()
            } else {
                summary
                    .as_ref()
                    .map(|existing| existing.summary_text.clone())
                    .unwrap_or_else(|| "OK".to_string())
            },
            generated_at_epoch_seconds: 2_000,
            created_at_epoch_seconds: summary
                .as_ref()
                .map(|existing| existing.created_at_epoch_seconds)
                .unwrap_or(2_000),
            updated_at_epoch_seconds: 2_000,
            provider: Some("openai".to_string()),
            model: Some("gpt-4o-mini".to_string()),
        };
        *summary = Some(generated.clone());
        Box::pin(async move {
            Ok(EnsuredAthleteSummary {
                summary: generated,
                was_regenerated,
            })
        })
    }
}
