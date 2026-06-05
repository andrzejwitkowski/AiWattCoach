use crate::domain::{
    ai_workflow::{AttemptRecord, WorkflowStatus},
    intervals::PlannedWorkout,
    llm_tools::LlmToolLoopState,
};

pub const MESO_CYCLE_WINDOW_DAY_COUNT: usize = 30;
pub const MESO_CYCLE_RECENT_DAY_COUNT: i64 = 30;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MesoCycleError {
    Unavailable(String),
    Repository(String),
    Validation(String),
    AlreadyPending,
    NotConfigured,
}

impl std::fmt::Display for MesoCycleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable(message) => write!(f, "{message}"),
            Self::Repository(message) => write!(f, "{message}"),
            Self::Validation(message) => write!(f, "{message}"),
            Self::AlreadyPending => write!(f, "meso cycle generation is already pending"),
            Self::NotConfigured => write!(f, "meso cycle llm is not configured"),
        }
    }
}

impl std::error::Error for MesoCycleError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MesoCycleWindow {
    pub meso_start: String,
    pub meso_end: String,
    pub ai_coach_last_date: Option<String>,
    pub source_training_plan_operation_key: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MesoCycleDay {
    pub date: String,
    pub rest_day: bool,
    pub rest_day_reason: Option<String>,
    pub workout: Option<PlannedWorkout>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MesoCycleProjectedDay {
    pub user_id: String,
    pub operation_key: String,
    pub date: String,
    pub rest_day: bool,
    pub rest_day_reason: Option<String>,
    pub workout: Option<PlannedWorkout>,
    pub superseded_at_epoch_seconds: Option<i64>,
    pub created_at_epoch_seconds: i64,
    pub updated_at_epoch_seconds: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MesoCycleOverlapStatus {
    Active,
    Outdated,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MesoCycleCalendarDay {
    pub date: String,
    pub rest_day: bool,
    pub rest_day_reason: Option<String>,
    pub name: Option<String>,
    pub raw_workout_doc: Option<String>,
    pub overlap_status: MesoCycleOverlapStatus,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MesoCycleFailureState {
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MesoCycleGenerationOperation {
    pub operation_key: String,
    pub user_id: String,
    pub requested_at_epoch_seconds: i64,
    pub meso_start: Option<String>,
    pub meso_end: Option<String>,
    pub status: WorkflowStatus,
    pub raw_plan_response: Option<String>,
    pub raw_plan_description: Option<String>,
    pub tool_loop_state: Option<LlmToolLoopState>,
    pub projection_persisted_at_epoch_seconds: Option<i64>,
    pub attempts: Vec<AttemptRecord>,
    pub failure: Option<MesoCycleFailureState>,
    pub started_at_epoch_seconds: i64,
    pub last_attempt_at_epoch_seconds: i64,
    pub attempt_count: u32,
    pub created_at_epoch_seconds: i64,
    pub updated_at_epoch_seconds: i64,
}

impl MesoCycleGenerationOperation {
    pub fn pending(
        operation_key: String,
        user_id: String,
        requested_at_epoch_seconds: i64,
        now_epoch_seconds: i64,
    ) -> Self {
        Self {
            operation_key,
            user_id,
            requested_at_epoch_seconds,
            meso_start: None,
            meso_end: None,
            status: WorkflowStatus::Pending,
            raw_plan_response: None,
            raw_plan_description: None,
            tool_loop_state: None,
            projection_persisted_at_epoch_seconds: None,
            attempts: Vec::new(),
            failure: None,
            started_at_epoch_seconds: now_epoch_seconds,
            last_attempt_at_epoch_seconds: now_epoch_seconds,
            attempt_count: 1,
            created_at_epoch_seconds: now_epoch_seconds,
            updated_at_epoch_seconds: now_epoch_seconds,
        }
    }

    pub fn reclaim(&self, now_epoch_seconds: i64) -> Self {
        Self {
            operation_key: self.operation_key.clone(),
            user_id: self.user_id.clone(),
            requested_at_epoch_seconds: self.requested_at_epoch_seconds,
            meso_start: self.meso_start.clone(),
            meso_end: self.meso_end.clone(),
            status: WorkflowStatus::Pending,
            raw_plan_response: self.raw_plan_response.clone(),
            raw_plan_description: self.raw_plan_description.clone(),
            tool_loop_state: self.tool_loop_state.clone(),
            projection_persisted_at_epoch_seconds: self.projection_persisted_at_epoch_seconds,
            attempts: self.attempts.clone(),
            failure: None,
            started_at_epoch_seconds: self.started_at_epoch_seconds,
            last_attempt_at_epoch_seconds: now_epoch_seconds,
            attempt_count: self.attempt_count.saturating_add(1),
            created_at_epoch_seconds: self.created_at_epoch_seconds,
            updated_at_epoch_seconds: now_epoch_seconds,
        }
    }

    pub fn reclaim_for_new_generation(&self, now_epoch_seconds: i64) -> Self {
        Self {
            operation_key: Self::stable_operation_key(&self.user_id),
            user_id: self.user_id.clone(),
            requested_at_epoch_seconds: now_epoch_seconds,
            meso_start: None,
            meso_end: None,
            status: WorkflowStatus::Pending,
            raw_plan_response: None,
            raw_plan_description: None,
            tool_loop_state: None,
            projection_persisted_at_epoch_seconds: None,
            attempts: self.attempts.clone(),
            failure: None,
            started_at_epoch_seconds: self.started_at_epoch_seconds,
            last_attempt_at_epoch_seconds: now_epoch_seconds,
            attempt_count: self.attempt_count.saturating_add(1),
            created_at_epoch_seconds: self.created_at_epoch_seconds,
            updated_at_epoch_seconds: now_epoch_seconds,
        }
    }

    pub fn stable_operation_key(user_id: &str) -> String {
        format!("meso-cycle:{user_id}")
    }

    pub fn with_tool_loop_state(
        &self,
        tool_loop_state: LlmToolLoopState,
        updated_at_epoch_seconds: i64,
    ) -> Self {
        Self {
            tool_loop_state: Some(tool_loop_state),
            updated_at_epoch_seconds,
            last_attempt_at_epoch_seconds: updated_at_epoch_seconds,
            ..self.clone()
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MesoCycleGenerationClaimResult {
    Claimed(MesoCycleGenerationOperation),
    AlreadyPending,
    AlreadyCompleted(MesoCycleGenerationOperation),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MesoCycleStatus {
    pub latest_operation: Option<MesoCycleGenerationOperation>,
    pub window: Option<MesoCycleWindow>,
    pub has_pending_generation: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MesoCyclePhaseOutput {
    pub raw_response: String,
    pub description: Option<String>,
    pub tool_loop_state: LlmToolLoopState,
}
