#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlannedWorkoutWahooSyncStatus {
    Unsynced,
    Pending,
    Synced,
    Modified,
    Failed,
}

impl PlannedWorkoutWahooSyncStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Unsynced => "unsynced",
            Self::Pending => "pending",
            Self::Synced => "synced",
            Self::Modified => "modified",
            Self::Failed => "failed",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlannedWorkoutWahooSyncRecord {
    pub user_id: String,
    pub operation_key: String,
    pub date: String,
    pub planned_workout_id: String,
    pub source_workout_id: String,
    pub payload_hash: Option<String>,
    pub status: PlannedWorkoutWahooSyncStatus,
    pub wahoo_plan_external_id: String,
    pub wahoo_plan_id: Option<i64>,
    pub wahoo_workout_id: Option<i64>,
    pub wahoo_workout_token: Option<String>,
    pub last_error: Option<String>,
    pub created_at_epoch_seconds: i64,
    pub updated_at_epoch_seconds: i64,
    pub last_synced_at_epoch_seconds: Option<i64>,
}

impl PlannedWorkoutWahooSyncRecord {
    pub fn pending(
        user_id: String,
        operation_key: String,
        date: String,
        planned_workout_id: String,
        source_workout_id: String,
        wahoo_plan_external_id: String,
        now_epoch_seconds: i64,
    ) -> Self {
        Self {
            user_id,
            operation_key,
            date,
            planned_workout_id,
            source_workout_id,
            payload_hash: None,
            status: PlannedWorkoutWahooSyncStatus::Pending,
            wahoo_plan_external_id,
            wahoo_plan_id: None,
            wahoo_workout_id: None,
            wahoo_workout_token: None,
            last_error: None,
            created_at_epoch_seconds: now_epoch_seconds,
            updated_at_epoch_seconds: now_epoch_seconds,
            last_synced_at_epoch_seconds: None,
        }
    }

    pub fn mark_pending(&self, now_epoch_seconds: i64) -> Self {
        let mut updated = self.clone();
        updated.status = PlannedWorkoutWahooSyncStatus::Pending;
        updated.last_error = None;
        updated.updated_at_epoch_seconds = now_epoch_seconds;
        updated
    }

    pub fn with_plan_id(&self, wahoo_plan_id: i64, now_epoch_seconds: i64) -> Self {
        let mut updated = self.clone();
        updated.wahoo_plan_id = Some(wahoo_plan_id);
        updated.updated_at_epoch_seconds = now_epoch_seconds;
        updated
    }

    pub fn mark_synced(
        &self,
        payload_hash: String,
        wahoo_plan_id: i64,
        wahoo_workout_id: i64,
        wahoo_workout_token: String,
        now_epoch_seconds: i64,
    ) -> Self {
        let mut updated = self.clone();
        updated.payload_hash = Some(payload_hash);
        updated.status = PlannedWorkoutWahooSyncStatus::Synced;
        updated.wahoo_plan_id = Some(wahoo_plan_id);
        updated.wahoo_workout_id = Some(wahoo_workout_id);
        updated.wahoo_workout_token = Some(wahoo_workout_token);
        updated.last_error = None;
        updated.updated_at_epoch_seconds = now_epoch_seconds;
        updated.last_synced_at_epoch_seconds = Some(now_epoch_seconds);
        updated
    }

    pub fn mark_failed(&self, error: String, now_epoch_seconds: i64) -> Self {
        let mut updated = self.clone();
        updated.status = PlannedWorkoutWahooSyncStatus::Failed;
        updated.last_error = Some(error);
        updated.updated_at_epoch_seconds = now_epoch_seconds;
        updated
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlannedWorkoutWahooSyncError {
    Repository(String),
}

impl std::fmt::Display for PlannedWorkoutWahooSyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Repository(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for PlannedWorkoutWahooSyncError {}
