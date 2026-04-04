use std::{future::Future, pin::Pin};

use crate::domain::llm::{LlmChatResponse, LlmError};

use super::{AthleteSummary, AthleteSummaryError};

pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

pub trait AthleteSummaryRepository: Send + Sync {
    fn find_by_user_id(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<Option<AthleteSummary>, AthleteSummaryError>>;

    fn upsert(
        &self,
        summary: AthleteSummary,
    ) -> BoxFuture<Result<AthleteSummary, AthleteSummaryError>>;
}

pub trait AthleteSummaryGenerator: Send + Sync {
    fn generate(&self, user_id: &str) -> BoxFuture<Result<LlmChatResponse, LlmError>>;
}
