use std::{collections::BTreeSet, future::Future, pin::Pin, sync::Arc};

use crate::domain::llm_tools::LlmToolLoopState;

use super::{
    MesoCycleError, MesoCycleGenerationClaimResult, MesoCycleGenerationOperation,
    MesoCyclePhaseOutput, MesoCycleProjectedDay, MesoCycleWindow,
};

pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;
pub type MesoCycleToolLoopCheckpoint =
    Arc<dyn Fn(LlmToolLoopState) -> BoxFuture<Result<(), MesoCycleError>> + Send + Sync>;

pub trait MesoCycleWindowPort: Send + Sync + 'static {
    fn resolve_window(
        &self,
        user_id: &str,
        today: &str,
    ) -> BoxFuture<Result<MesoCycleWindow, MesoCycleError>>;

    fn ai_coach_active_dates(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<BTreeSet<String>, MesoCycleError>>;
}

pub trait MesoCycleGenerationOperationRepository: Send + Sync + 'static {
    fn find_by_operation_key(
        &self,
        operation_key: &str,
    ) -> BoxFuture<Result<Option<MesoCycleGenerationOperation>, MesoCycleError>>;

    fn find_by_operation_key_for_user(
        &self,
        operation_key: &str,
        user_id: &str,
    ) -> BoxFuture<Result<Option<MesoCycleGenerationOperation>, MesoCycleError>>;

    fn find_latest_by_user_id(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<Option<MesoCycleGenerationOperation>, MesoCycleError>>;

    fn find_pending_by_user_id(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<Option<MesoCycleGenerationOperation>, MesoCycleError>>;

    fn claim_pending(
        &self,
        operation: MesoCycleGenerationOperation,
        stale_before_epoch_seconds: i64,
    ) -> BoxFuture<Result<MesoCycleGenerationClaimResult, MesoCycleError>>;

    fn upsert(
        &self,
        operation: MesoCycleGenerationOperation,
    ) -> BoxFuture<Result<MesoCycleGenerationOperation, MesoCycleError>>;
}

pub trait MesoCycleProjectionRepository: Send + Sync + 'static {
    fn list_active_by_user_id(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<Vec<MesoCycleProjectedDay>, MesoCycleError>>;

    fn find_active_by_operation_key(
        &self,
        operation_key: &str,
    ) -> BoxFuture<Result<Vec<MesoCycleProjectedDay>, MesoCycleError>>;

    fn replace_window(
        &self,
        user_id: &str,
        operation_key: &str,
        projected_days: Vec<MesoCycleProjectedDay>,
        replaced_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<(), MesoCycleError>>;
}

pub trait MesoCycleGenerator: Send + Sync + 'static {
    fn generate_plan_window_with_state(
        &self,
        user_id: &str,
        window: &MesoCycleWindow,
        restored_state: Option<LlmToolLoopState>,
        checkpoint: Option<MesoCycleToolLoopCheckpoint>,
    ) -> BoxFuture<Result<MesoCyclePhaseOutput, MesoCycleError>>;
}

pub trait MesoCycleLlmConfigPort: Send + Sync + 'static {
    fn get_meso_cycle_config(
        &self,
        user_id: &str,
    ) -> BoxFuture<Result<crate::domain::llm::LlmProviderConfig, MesoCycleError>>;
}
