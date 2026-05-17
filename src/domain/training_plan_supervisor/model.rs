#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrainingPlanSupervisorStatus {
    Pending,
    Accepted,
    Replaced,
    Failed,
}

impl TrainingPlanSupervisorStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Accepted => "accepted",
            Self::Replaced => "replaced",
            Self::Failed => "failed",
        }
    }
}

impl TryFrom<&str> for TrainingPlanSupervisorStatus {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "pending" => Ok(Self::Pending),
            "accepted" => Ok(Self::Accepted),
            "replaced" => Ok(Self::Replaced),
            "failed" => Ok(Self::Failed),
            other => Err(format!("unknown training plan supervisor status: {other}")),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrainingPlanSupervisorOperation {
    pub worker_operation_key: String,
    pub user_id: String,
    pub worker_saved_at_epoch_seconds: i64,
    pub model: String,
    pub status: TrainingPlanSupervisorStatus,
    pub created_at_epoch_seconds: i64,
    pub updated_at_epoch_seconds: i64,
}

impl TrainingPlanSupervisorOperation {
    pub fn pending(
        worker_operation_key: String,
        user_id: String,
        worker_saved_at_epoch_seconds: i64,
        model: String,
        now_epoch_seconds: i64,
    ) -> Self {
        Self {
            worker_operation_key,
            user_id,
            worker_saved_at_epoch_seconds,
            model,
            status: TrainingPlanSupervisorStatus::Pending,
            created_at_epoch_seconds: now_epoch_seconds,
            updated_at_epoch_seconds: now_epoch_seconds,
        }
    }
}
