use std::{future::Future, pin::Pin};

use super::{AdminPromptPreviewError, AdminPromptPreviewResponse};

pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

pub trait AdminPromptPreviewUseCases: Send + Sync {
    fn preview_post_workout(
        &self,
        user_id: &str,
        date: &str,
    ) -> BoxFuture<Result<AdminPromptPreviewResponse, AdminPromptPreviewError>>;

    fn preview_calendar_coach(
        &self,
        user_id: &str,
        date: &str,
    ) -> BoxFuture<Result<AdminPromptPreviewResponse, AdminPromptPreviewError>>;
}
