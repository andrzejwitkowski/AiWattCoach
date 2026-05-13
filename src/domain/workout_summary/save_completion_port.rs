use super::service::SaveWorkflowStatus;

pub trait SaveWorkflowCompletionPort: Send + Sync + 'static {
    fn on_completed(
        &self,
        user_id: &str,
        workout_id: &str,
        recap_status: SaveWorkflowStatus,
        plan_status: SaveWorkflowStatus,
        messages: Vec<String>,
    );
}

pub struct NoopSaveWorkflowCompletionPort;

impl SaveWorkflowCompletionPort for NoopSaveWorkflowCompletionPort {
    fn on_completed(
        &self,
        _user_id: &str,
        _workout_id: &str,
        _recap_status: SaveWorkflowStatus,
        _plan_status: SaveWorkflowStatus,
        _messages: Vec<String>,
    ) {
    }
}
