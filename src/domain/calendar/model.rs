use serde::{Deserialize, Serialize};

use crate::domain::intervals::Event;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CalendarError {
    NotFound,
    Unauthenticated,
    CredentialsNotConfigured,
    Validation(String),
    Unavailable(String),
    Internal(String),
}

impl std::fmt::Display for CalendarError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => write!(f, "Calendar entry not found"),
            Self::Unauthenticated => write!(f, "Authentication is required"),
            Self::CredentialsNotConfigured => {
                write!(f, "Intervals.icu credentials are not configured")
            }
            Self::Validation(message) => write!(f, "{message}"),
            Self::Unavailable(message) => write!(f, "{message}"),
            Self::Internal(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for CalendarError {}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CalendarEventSource {
    Intervals,
    Predicted,
}

impl CalendarEventSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Intervals => "intervals",
            Self::Predicted => "predicted",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlannedWorkoutSyncStatus {
    Unsynced,
    Pending,
    Synced,
    Modified,
    Failed,
}

impl PlannedWorkoutSyncStatus {
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
pub struct PlannedWorkoutSyncRecord {
    pub user_id: String,
    pub operation_key: String,
    pub date: String,
    pub source_workout_id: String,
    pub intervals_event_id: Option<i64>,
    pub status: PlannedWorkoutSyncStatus,
    pub synced_payload_hash: Option<String>,
    pub last_error: Option<String>,
    pub created_at_epoch_seconds: i64,
    pub updated_at_epoch_seconds: i64,
    pub last_synced_at_epoch_seconds: Option<i64>,
}

impl PlannedWorkoutSyncRecord {
    pub fn pending(
        user_id: String,
        operation_key: String,
        date: String,
        source_workout_id: String,
        now_epoch_seconds: i64,
    ) -> Self {
        Self {
            user_id,
            operation_key,
            date,
            source_workout_id,
            intervals_event_id: None,
            status: PlannedWorkoutSyncStatus::Pending,
            synced_payload_hash: None,
            last_error: None,
            created_at_epoch_seconds: now_epoch_seconds,
            updated_at_epoch_seconds: now_epoch_seconds,
            last_synced_at_epoch_seconds: None,
        }
    }

    pub fn mark_pending(&self, source_workout_id: String, now_epoch_seconds: i64) -> Self {
        Self {
            user_id: self.user_id.clone(),
            operation_key: self.operation_key.clone(),
            date: self.date.clone(),
            source_workout_id,
            intervals_event_id: self.intervals_event_id,
            status: PlannedWorkoutSyncStatus::Pending,
            synced_payload_hash: self.synced_payload_hash.clone(),
            last_error: None,
            created_at_epoch_seconds: self.created_at_epoch_seconds,
            updated_at_epoch_seconds: now_epoch_seconds,
            last_synced_at_epoch_seconds: self.last_synced_at_epoch_seconds,
        }
    }

    pub fn mark_synced(
        &self,
        intervals_event_id: i64,
        source_workout_id: String,
        synced_payload_hash: String,
        now_epoch_seconds: i64,
    ) -> Self {
        Self {
            user_id: self.user_id.clone(),
            operation_key: self.operation_key.clone(),
            date: self.date.clone(),
            source_workout_id,
            intervals_event_id: Some(intervals_event_id),
            status: PlannedWorkoutSyncStatus::Synced,
            synced_payload_hash: Some(synced_payload_hash),
            last_error: None,
            created_at_epoch_seconds: self.created_at_epoch_seconds,
            updated_at_epoch_seconds: now_epoch_seconds,
            last_synced_at_epoch_seconds: Some(now_epoch_seconds),
        }
    }

    pub fn mark_failed(
        &self,
        source_workout_id: String,
        error: String,
        now_epoch_seconds: i64,
    ) -> Self {
        Self {
            user_id: self.user_id.clone(),
            operation_key: self.operation_key.clone(),
            date: self.date.clone(),
            source_workout_id,
            intervals_event_id: self.intervals_event_id,
            status: PlannedWorkoutSyncStatus::Failed,
            synced_payload_hash: self.synced_payload_hash.clone(),
            last_error: Some(error),
            created_at_epoch_seconds: self.created_at_epoch_seconds,
            updated_at_epoch_seconds: now_epoch_seconds,
            last_synced_at_epoch_seconds: self.last_synced_at_epoch_seconds,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CalendarProjectedWorkout {
    pub projected_workout_id: String,
    pub operation_key: String,
    pub date: String,
    pub source_workout_id: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CalendarEvent {
    pub calendar_entry_id: String,
    pub event: Event,
    pub source: CalendarEventSource,
    pub projected_workout: Option<CalendarProjectedWorkout>,
    pub sync_status: Option<PlannedWorkoutSyncStatus>,
    pub linked_intervals_event_id: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncPlannedWorkout {
    pub operation_key: String,
    pub date: String,
}
