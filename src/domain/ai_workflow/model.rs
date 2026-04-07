#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkflowStatus {
    Pending,
    Completed,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkflowPhase {
    WorkoutRecap,
    InitialGeneration,
    Correction,
    ProjectionUpdate,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttemptRecord {
    pub phase: WorkflowPhase,
    pub attempt_number: u32,
    pub recorded_at_epoch_seconds: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidationIssue {
    pub scope: String,
    pub message: String,
}
