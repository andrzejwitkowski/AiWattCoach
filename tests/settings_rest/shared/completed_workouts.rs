use std::sync::{Arc, Mutex};

use aiwattcoach::domain::completed_workouts::{
    BackfillCompletedWorkoutDetailsResult, CompletedWorkoutAdminUseCases, CompletedWorkoutError,
};

use super::app::BoxFuture;

#[derive(Clone, Default)]
pub(crate) struct TestCompletedWorkoutAdminService {
    calls: Arc<Mutex<Vec<(String, String, String)>>>,
    error: Arc<Mutex<Option<String>>>,
}

impl TestCompletedWorkoutAdminService {
    pub(crate) fn failing(message: &str) -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            error: Arc::new(Mutex::new(Some(message.to_string()))),
        }
    }

    pub(crate) fn calls(&self) -> Vec<(String, String, String)> {
        self.calls.lock().unwrap().clone()
    }
}

impl CompletedWorkoutAdminUseCases for TestCompletedWorkoutAdminService {
    fn backfill_missing_details(
        &self,
        user_id: &str,
        oldest: &str,
        newest: &str,
    ) -> BoxFuture<Result<BackfillCompletedWorkoutDetailsResult, CompletedWorkoutError>> {
        let calls = self.calls.clone();
        let user_id = user_id.to_string();
        let oldest = oldest.to_string();
        let newest = newest.to_string();
        let error = self.error.lock().unwrap().clone();
        Box::pin(async move {
            calls.lock().unwrap().push((user_id, oldest, newest));
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
}
