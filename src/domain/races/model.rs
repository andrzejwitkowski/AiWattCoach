use sha2::{Digest, Sha256};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RaceError {
    NotFound,
    Unauthenticated,
    Validation(String),
    Unavailable(String),
    Internal(String),
}

impl std::fmt::Display for RaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => write!(f, "Race not found"),
            Self::Unauthenticated => write!(f, "Authentication is required"),
            Self::Validation(message) => write!(f, "{message}"),
            Self::Unavailable(message) => write!(f, "{message}"),
            Self::Internal(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for RaceError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RaceDiscipline {
    Road,
    Mtb,
    Gravel,
    Cyclocross,
    Timetrial,
}

impl RaceDiscipline {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Road => "road",
            Self::Mtb => "mtb",
            Self::Gravel => "gravel",
            Self::Cyclocross => "cyclocross",
            Self::Timetrial => "timetrial",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RacePriority {
    A,
    B,
    C,
}

impl RacePriority {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::A => "A",
            Self::B => "B",
            Self::C => "C",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RaceSyncStatus {
    Pending,
    Synced,
    Failed,
    PendingDelete,
}

impl RaceSyncStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Synced => "synced",
            Self::Failed => "failed",
            Self::PendingDelete => "pending_delete",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RaceResult {
    Finished,
    Dnf,
    Dsq,
}

impl RaceResult {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Finished => "finished",
            Self::Dnf => "dnf",
            Self::Dsq => "dsq",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Race {
    pub race_id: String,
    pub user_id: String,
    pub date: String,
    pub name: String,
    pub distance_meters: i32,
    pub discipline: RaceDiscipline,
    pub priority: RacePriority,
    pub linked_intervals_event_id: Option<i64>,
    pub sync_status: RaceSyncStatus,
    pub synced_payload_hash: Option<String>,
    pub last_error: Option<String>,
    pub result: Option<RaceResult>,
    pub created_at_epoch_seconds: i64,
    pub updated_at_epoch_seconds: i64,
    pub last_synced_at_epoch_seconds: Option<i64>,
}

impl Race {
    pub fn pending_new(
        race_id: String,
        user_id: String,
        request: CreateRace,
        now_epoch_seconds: i64,
    ) -> Self {
        Self {
            race_id,
            user_id,
            date: request.date,
            name: request.name,
            distance_meters: request.distance_meters,
            discipline: request.discipline,
            priority: request.priority,
            linked_intervals_event_id: None,
            sync_status: RaceSyncStatus::Pending,
            synced_payload_hash: None,
            last_error: None,
            result: None,
            created_at_epoch_seconds: now_epoch_seconds,
            updated_at_epoch_seconds: now_epoch_seconds,
            last_synced_at_epoch_seconds: None,
        }
    }

    pub fn mark_pending_update(&self, request: UpdateRace, now_epoch_seconds: i64) -> Self {
        Self {
            race_id: self.race_id.clone(),
            user_id: self.user_id.clone(),
            date: request.date,
            name: request.name,
            distance_meters: request.distance_meters,
            discipline: request.discipline,
            priority: request.priority,
            linked_intervals_event_id: self.linked_intervals_event_id,
            sync_status: RaceSyncStatus::Pending,
            synced_payload_hash: self.synced_payload_hash.clone(),
            last_error: None,
            result: self.result.clone(),
            created_at_epoch_seconds: self.created_at_epoch_seconds,
            updated_at_epoch_seconds: now_epoch_seconds,
            last_synced_at_epoch_seconds: self.last_synced_at_epoch_seconds,
        }
    }

    pub fn mark_synced(
        &self,
        linked_intervals_event_id: i64,
        synced_payload_hash: String,
        now_epoch_seconds: i64,
    ) -> Self {
        Self {
            race_id: self.race_id.clone(),
            user_id: self.user_id.clone(),
            date: self.date.clone(),
            name: self.name.clone(),
            distance_meters: self.distance_meters,
            discipline: self.discipline.clone(),
            priority: self.priority.clone(),
            linked_intervals_event_id: Some(linked_intervals_event_id),
            sync_status: RaceSyncStatus::Synced,
            synced_payload_hash: Some(synced_payload_hash),
            last_error: None,
            result: self.result.clone(),
            created_at_epoch_seconds: self.created_at_epoch_seconds,
            updated_at_epoch_seconds: now_epoch_seconds,
            last_synced_at_epoch_seconds: Some(now_epoch_seconds),
        }
    }

    pub fn mark_failed(&self, error: String, now_epoch_seconds: i64) -> Self {
        Self {
            race_id: self.race_id.clone(),
            user_id: self.user_id.clone(),
            date: self.date.clone(),
            name: self.name.clone(),
            distance_meters: self.distance_meters,
            discipline: self.discipline.clone(),
            priority: self.priority.clone(),
            linked_intervals_event_id: self.linked_intervals_event_id,
            sync_status: RaceSyncStatus::Failed,
            synced_payload_hash: self.synced_payload_hash.clone(),
            last_error: Some(error),
            result: self.result.clone(),
            created_at_epoch_seconds: self.created_at_epoch_seconds,
            updated_at_epoch_seconds: now_epoch_seconds,
            last_synced_at_epoch_seconds: self.last_synced_at_epoch_seconds,
        }
    }

    pub fn mark_pending_delete(&self, now_epoch_seconds: i64) -> Self {
        Self {
            race_id: self.race_id.clone(),
            user_id: self.user_id.clone(),
            date: self.date.clone(),
            name: self.name.clone(),
            distance_meters: self.distance_meters,
            discipline: self.discipline.clone(),
            priority: self.priority.clone(),
            linked_intervals_event_id: self.linked_intervals_event_id,
            sync_status: RaceSyncStatus::PendingDelete,
            synced_payload_hash: self.synced_payload_hash.clone(),
            last_error: None,
            result: self.result.clone(),
            created_at_epoch_seconds: self.created_at_epoch_seconds,
            updated_at_epoch_seconds: now_epoch_seconds,
            last_synced_at_epoch_seconds: self.last_synced_at_epoch_seconds,
        }
    }

    pub fn payload_hash(&self) -> String {
        let digest = Sha256::digest(format!(
            "{}\n{}\n{}\n{}\n{}",
            self.date,
            self.name,
            self.distance_meters,
            self.discipline.as_str(),
            self.priority.as_str()
        ));
        format!("{digest:x}")
    }

    /// Formatted title for display on calendar labels (e.g. "Race Gravel Attack").
    pub fn label_title(&self) -> String {
        format!("Race {}", self.name)
    }

    /// Formatted subtitle for display on calendar labels (e.g. "120 km • Kat. A").
    pub fn label_subtitle(&self) -> String {
        format!(
            "{} km • Kat. {}",
            self.distance_meters / 1000,
            self.priority.as_str()
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateRace {
    pub date: String,
    pub name: String,
    pub distance_meters: i32,
    pub discipline: RaceDiscipline,
    pub priority: RacePriority,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateRace {
    pub date: String,
    pub name: String,
    pub distance_meters: i32,
    pub discipline: RaceDiscipline,
    pub priority: RacePriority,
}
