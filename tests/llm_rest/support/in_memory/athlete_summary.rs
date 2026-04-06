use super::*;

#[derive(Clone)]
pub(crate) struct InMemoryAthleteSummaryService {
    state: Arc<Mutex<AthleteSummaryState>>,
}

impl InMemoryAthleteSummaryService {
    pub(crate) fn seed(&self, summary: Option<AthleteSummary>, stale: bool) {
        *self.state.lock().unwrap() = AthleteSummaryState { summary, stale };
    }

    fn make_summary(&self, user_id: String, created_at_epoch_seconds: i64) -> AthleteSummary {
        AthleteSummary {
            user_id,
            summary_text: "Generated athlete summary".to_string(),
            generated_at_epoch_seconds: 1_700_000_000,
            created_at_epoch_seconds,
            updated_at_epoch_seconds: 1_700_000_000,
            provider: Some("openrouter".to_string()),
            model: Some("google/gemini-3-flash-preview".to_string()),
        }
    }
}

impl Default for InMemoryAthleteSummaryService {
    fn default() -> Self {
        Self {
            state: Arc::new(Mutex::new(AthleteSummaryState {
                summary: None,
                stale: true,
            })),
        }
    }
}

impl AthleteSummaryUseCases for InMemoryAthleteSummaryService {
    fn get_summary_state(
        &self,
        _user_id: &str,
    ) -> aiwattcoach::domain::athlete_summary::BoxFuture<
        Result<AthleteSummaryState, AthleteSummaryError>,
    > {
        let state = self.state.lock().unwrap().clone();
        Box::pin(async move { Ok(state) })
    }

    fn generate_summary(
        &self,
        user_id: &str,
        force: bool,
    ) -> aiwattcoach::domain::athlete_summary::BoxFuture<Result<AthleteSummary, AthleteSummaryError>>
    {
        let state = self.state.clone();
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let mut state = state.lock().unwrap();
            if !force && !state.stale {
                if let Some(summary) = state.summary.clone() {
                    return Ok(summary);
                }
            }

            let summary = service.make_summary(
                user_id,
                state
                    .summary
                    .as_ref()
                    .map(|summary| summary.created_at_epoch_seconds)
                    .unwrap_or(1_700_000_000),
            );
            state.summary = Some(summary.clone());
            state.stale = false;
            Ok(summary)
        })
    }

    fn ensure_fresh_summary(
        &self,
        user_id: &str,
    ) -> aiwattcoach::domain::athlete_summary::BoxFuture<Result<AthleteSummary, AthleteSummaryError>>
    {
        let state = self.state.clone();
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let mut state = state.lock().unwrap();
            if !state.stale {
                if let Some(summary) = state.summary.clone() {
                    return Ok(summary);
                }
            }

            let summary = service.make_summary(
                user_id,
                state
                    .summary
                    .as_ref()
                    .map(|summary| summary.created_at_epoch_seconds)
                    .unwrap_or(1_700_000_000),
            );
            state.summary = Some(summary.clone());
            state.stale = false;
            Ok(summary)
        })
    }

    fn ensure_fresh_summary_state(
        &self,
        user_id: &str,
    ) -> aiwattcoach::domain::athlete_summary::BoxFuture<
        Result<EnsuredAthleteSummary, AthleteSummaryError>,
    > {
        let state = self.state.clone();
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let mut state = state.lock().unwrap();
            let was_regenerated = state.stale || state.summary.is_none();

            let summary = if !state.stale {
                if let Some(summary) = state.summary.clone() {
                    summary
                } else {
                    let summary = service.make_summary(user_id, 1_700_000_000);
                    state.summary = Some(summary.clone());
                    summary
                }
            } else {
                let summary = service.make_summary(
                    user_id,
                    state
                        .summary
                        .as_ref()
                        .map(|summary| summary.created_at_epoch_seconds)
                        .unwrap_or(1_700_000_000),
                );
                state.summary = Some(summary.clone());
                state.stale = false;
                summary
            };

            Ok(EnsuredAthleteSummary {
                summary,
                was_regenerated,
            })
        })
    }
}
