use std::collections::BTreeMap;

use super::*;

#[derive(Clone)]
pub(crate) struct InMemoryAthleteSummaryService {
    state_by_user_id: Arc<Mutex<BTreeMap<String, AthleteSummaryState>>>,
}

impl InMemoryAthleteSummaryService {
    pub(crate) fn seed(&self, user_id: &str, summary: Option<AthleteSummary>, stale: bool) {
        self.state_by_user_id
            .lock()
            .unwrap()
            .insert(user_id.to_string(), AthleteSummaryState { summary, stale });
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
            state_by_user_id: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }
}

impl AthleteSummaryUseCases for InMemoryAthleteSummaryService {
    fn get_summary_state(
        &self,
        user_id: &str,
    ) -> aiwattcoach::domain::athlete_summary::BoxFuture<
        Result<AthleteSummaryState, AthleteSummaryError>,
    > {
        let state = self
            .state_by_user_id
            .lock()
            .unwrap()
            .get(user_id)
            .cloned()
            .unwrap_or(AthleteSummaryState {
                summary: None,
                stale: true,
            });
        Box::pin(async move { Ok(state) })
    }

    fn generate_summary(
        &self,
        user_id: &str,
        force: bool,
    ) -> aiwattcoach::domain::athlete_summary::BoxFuture<Result<AthleteSummary, AthleteSummaryError>>
    {
        let state_by_user_id = self.state_by_user_id.clone();
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let mut states = state_by_user_id.lock().unwrap();
            let state = states
                .entry(user_id.clone())
                .or_insert(AthleteSummaryState {
                    summary: None,
                    stale: true,
                });
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
        let state_by_user_id = self.state_by_user_id.clone();
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let mut states = state_by_user_id.lock().unwrap();
            let state = states
                .entry(user_id.clone())
                .or_insert(AthleteSummaryState {
                    summary: None,
                    stale: true,
                });
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
        let state_by_user_id = self.state_by_user_id.clone();
        let service = self.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let mut states = state_by_user_id.lock().unwrap();
            let state = states
                .entry(user_id.clone())
                .or_insert(AthleteSummaryState {
                    summary: None,
                    stale: true,
                });
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
