use std::sync::{Arc, Mutex};

use aiwattcoach::domain::completed_workouts::{
    BackfillCompletedWorkoutDetailsResult, BackfillCompletedWorkoutMetricsResult,
    CompletedWorkoutAdminUseCases, CompletedWorkoutError,
};

use super::app::BoxFuture;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DetailBackfillCall {
    pub(crate) user_id: String,
    pub(crate) oldest: String,
    pub(crate) newest: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MetricsBackfillRange {
    pub(crate) oldest: Option<String>,
    pub(crate) newest: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MetricsBackfillCall {
    pub(crate) user_id: String,
    pub(crate) range: MetricsBackfillRange,
}

#[derive(Clone, Default)]
pub(crate) struct TestCompletedWorkoutAdminService {
    detail_calls: Arc<Mutex<Vec<DetailBackfillCall>>>,
    metric_calls: Arc<Mutex<Vec<MetricsBackfillCall>>>,
    error: Arc<Mutex<Option<String>>>,
}

impl TestCompletedWorkoutAdminService {
    pub(crate) fn failing(message: &str) -> Self {
        Self {
            detail_calls: Arc::new(Mutex::new(Vec::new())),
            metric_calls: Arc::new(Mutex::new(Vec::new())),
            error: Arc::new(Mutex::new(Some(message.to_string()))),
        }
    }

    pub(crate) fn detail_calls(&self) -> Vec<DetailBackfillCall> {
        self.detail_calls.lock().unwrap().clone()
    }

    pub(crate) fn metric_calls(&self) -> Vec<MetricsBackfillCall> {
        self.metric_calls.lock().unwrap().clone()
    }
}

impl CompletedWorkoutAdminUseCases for TestCompletedWorkoutAdminService {
    fn backfill_missing_details(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> BoxFuture<Result<BackfillCompletedWorkoutDetailsResult, CompletedWorkoutError>> {
        let calls = self.detail_calls.clone();
        let user_id = user_id.to_string();
        let oldest = oldest.to_string();
        let newest = newest.to_string();
        let error = self.error.lock().unwrap().clone();
        Box::pin(async move {
            calls.lock().unwrap().push(DetailBackfillCall {
                user_id,
                oldest,
                newest,
            });
            if let Some(message) = error {
                return Err(CompletedWorkoutError::Repository(message));
            }
            Ok(BackfillCompletedWorkoutDetailsResult {
                scanned: 0,
                enriched: 0,
                skipped: 0,
                failed: 0,
            })
        })
    }

    fn backfill_missing_metrics(
        &self,
        user_id: &str,
        oldest: Option<&str>,
        newest: Option<&str>,
    ) -> BoxFuture<Result<BackfillCompletedWorkoutMetricsResult, CompletedWorkoutError>> {
        let calls = self.metric_calls.clone();
        let user_id = user_id.to_string();
        let oldest = oldest.map(ToString::to_string);
        let newest = newest.map(ToString::to_string);
        let error = self.error.lock().unwrap().clone();
        Box::pin(async move {
            calls.lock().unwrap().push(MetricsBackfillCall {
                user_id,
                range: MetricsBackfillRange {
                    oldest: oldest.clone(),
                    newest: newest.clone(),
                },
            });
            if let Some(message) = error {
                return Err(CompletedWorkoutError::Repository(message));
            }
            Ok(BackfillCompletedWorkoutMetricsResult {
                scanned: 0,
                enriched: 0,
                skipped: 0,
                failed: 0,
                recomputed_from: oldest,
            })
        })
    }
}
