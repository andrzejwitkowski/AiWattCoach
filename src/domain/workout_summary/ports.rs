use std::{future::Future, pin::Pin};

use super::{ConversationMessage, WorkoutSummary, WorkoutSummaryError};

pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

pub trait WorkoutSummaryRepository: Clone + Send + Sync + 'static {
    fn find_by_user_id_and_event_id(
        &self,
        user_id: &str,
        event_id: &str,
    ) -> BoxFuture<Result<Option<WorkoutSummary>, WorkoutSummaryError>>;

    fn find_by_user_id_and_event_ids(
        &self,
        user_id: &str,
        event_ids: Vec<String>,
    ) -> BoxFuture<Result<Vec<WorkoutSummary>, WorkoutSummaryError>>;

    fn create(
        &self,
        summary: WorkoutSummary,
    ) -> BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>>;

    fn update_rpe(
        &self,
        user_id: &str,
        event_id: &str,
        rpe: u8,
        updated_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<(), WorkoutSummaryError>>;

    fn append_message(
        &self,
        user_id: &str,
        event_id: &str,
        message: ConversationMessage,
        updated_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<(), WorkoutSummaryError>>;
}
