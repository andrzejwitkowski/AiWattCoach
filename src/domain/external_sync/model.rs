#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExternalProvider {
    Intervals,
    Wahoo,
    Strava,
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExternalSyncRepositoryError {
    Storage(String),
    CorruptData(String),
}

impl std::fmt::Display for ExternalSyncRepositoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Storage(message) | Self::CorruptData(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for ExternalSyncRepositoryError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExternalObjectKind {
    PlannedWorkout,
    CompletedWorkout,
    Race,
    SpecialDay,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CanonicalEntityKind {
    PlannedWorkout,
    CompletedWorkout,
    Race,
    SpecialDay,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanonicalEntityRef {
    pub entity_kind: CanonicalEntityKind,
    pub entity_id: String,
}

impl CanonicalEntityRef {
    pub fn new(entity_kind: CanonicalEntityKind, entity_id: String) -> Self {
        Self {
            entity_kind,
            entity_id,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalObservation {
    pub user_id: String,
    pub provider: ExternalProvider,
    pub external_object_kind: ExternalObjectKind,
    pub external_id: String,
    pub canonical_entity: CanonicalEntityRef,
    pub normalized_payload_hash: Option<String>,
    pub dedup_key: Option<String>,
    pub observed_at_epoch_seconds: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalObservationParams {
    pub user_id: String,
    pub provider: ExternalProvider,
    pub external_object_kind: ExternalObjectKind,
    pub external_id: String,
    pub canonical_entity: CanonicalEntityRef,
    pub normalized_payload_hash: Option<String>,
    pub dedup_key: Option<String>,
    pub observed_at_epoch_seconds: i64,
}

impl ExternalObservation {
    pub fn new(params: ExternalObservationParams) -> Self {
        Self {
            user_id: params.user_id,
            provider: params.provider,
            external_object_kind: params.external_object_kind,
            external_id: params.external_id,
            canonical_entity: params.canonical_entity,
            normalized_payload_hash: params.normalized_payload_hash,
            dedup_key: params.dedup_key,
            observed_at_epoch_seconds: params.observed_at_epoch_seconds,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConflictStatus {
    Unknown,
    InSync,
    ConflictDetected,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExternalSyncStatus {
    Pending,
    Synced,
    Failed,
    PendingDelete,
}

impl ExternalSyncStatus {
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
pub struct ExternalSyncState {
    pub user_id: String,
    pub provider: ExternalProvider,
    pub canonical_entity: CanonicalEntityRef,
    pub external_id: Option<String>,
    pub sync_status: ExternalSyncStatus,
    pub last_synced_payload_hash: Option<String>,
    pub last_seen_remote_payload_hash: Option<String>,
    pub last_error: Option<String>,
    pub last_synced_at_epoch_seconds: Option<i64>,
    pub last_seen_remote_at_epoch_seconds: Option<i64>,
    pub conflict_status: ConflictStatus,
}

impl ExternalSyncState {
    pub fn new(
        user_id: String,
        provider: ExternalProvider,
        canonical_entity: CanonicalEntityRef,
    ) -> Self {
        Self {
            user_id,
            provider,
            canonical_entity,
            external_id: None,
            sync_status: ExternalSyncStatus::Pending,
            last_synced_payload_hash: None,
            last_seen_remote_payload_hash: None,
            last_error: None,
            last_synced_at_epoch_seconds: None,
            last_seen_remote_at_epoch_seconds: None,
            conflict_status: ConflictStatus::Unknown,
        }
    }

    pub fn mark_pending_push(mut self) -> Self {
        self.sync_status = ExternalSyncStatus::Pending;
        self.last_error = None;
        self
    }

    pub fn record_local_push(mut self, payload_hash: String, synced_at_epoch_seconds: i64) -> Self {
        self.sync_status = ExternalSyncStatus::Synced;
        self.last_synced_payload_hash = Some(payload_hash);
        self.last_synced_at_epoch_seconds = Some(synced_at_epoch_seconds);
        self.last_error = None;
        self.conflict_status = ConflictStatus::InSync;
        self
    }

    pub fn mark_synced(
        mut self,
        external_id: String,
        payload_hash: String,
        synced_at_epoch_seconds: i64,
    ) -> Self {
        self.external_id = Some(external_id);
        self.sync_status = ExternalSyncStatus::Synced;
        self.last_synced_payload_hash = Some(payload_hash.clone());
        self.last_seen_remote_payload_hash = Some(payload_hash);
        self.last_error = None;
        self.last_synced_at_epoch_seconds = Some(synced_at_epoch_seconds);
        self.last_seen_remote_at_epoch_seconds = Some(synced_at_epoch_seconds);
        self.conflict_status = ConflictStatus::InSync;
        self
    }

    pub fn mark_remote_created(mut self, external_id: String) -> Self {
        self.external_id = Some(external_id);
        self.sync_status = ExternalSyncStatus::Pending;
        self.last_error = None;
        self
    }

    pub fn mark_failed(mut self, error: String) -> Self {
        self.sync_status = ExternalSyncStatus::Failed;
        self.last_error = Some(error);
        self.conflict_status = ConflictStatus::Unknown;
        self
    }

    pub fn mark_pending_delete(mut self) -> Self {
        self.sync_status = ExternalSyncStatus::PendingDelete;
        self.last_error = None;
        self
    }

    pub fn observe_remote(mut self, payload_hash: String, observed_at_epoch_seconds: i64) -> Self {
        self.conflict_status = match self.last_synced_payload_hash.as_deref() {
            Some(last_synced) if last_synced == payload_hash => ConflictStatus::InSync,
            Some(_) => ConflictStatus::ConflictDetected,
            None => ConflictStatus::Unknown,
        };
        self.last_seen_remote_payload_hash = Some(payload_hash);
        self.last_seen_remote_at_epoch_seconds = Some(observed_at_epoch_seconds);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProviderPollStream {
    Calendar,
    CompletedWorkouts,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderPollState {
    pub user_id: String,
    pub provider: ExternalProvider,
    pub stream: ProviderPollStream,
    pub cursor: Option<String>,
    pub next_due_at_epoch_seconds: i64,
    pub last_attempted_at_epoch_seconds: Option<i64>,
    pub last_successful_at_epoch_seconds: Option<i64>,
    pub last_error: Option<String>,
    pub backoff_until_epoch_seconds: Option<i64>,
}

impl ProviderPollState {
    pub fn new(
        user_id: String,
        provider: ExternalProvider,
        stream: ProviderPollStream,
        next_due_at_epoch_seconds: i64,
    ) -> Self {
        Self {
            user_id,
            provider,
            stream,
            cursor: None,
            next_due_at_epoch_seconds,
            last_attempted_at_epoch_seconds: None,
            last_successful_at_epoch_seconds: None,
            last_error: None,
            backoff_until_epoch_seconds: None,
        }
    }

    pub fn is_due(&self, now_epoch_seconds: i64) -> bool {
        now_epoch_seconds >= self.next_due_at_epoch_seconds
            && self
                .backoff_until_epoch_seconds
                .map(|backoff_until| now_epoch_seconds >= backoff_until)
                .unwrap_or(true)
    }

    pub fn mark_attempted(mut self, attempted_at_epoch_seconds: i64) -> Self {
        self.last_attempted_at_epoch_seconds = Some(attempted_at_epoch_seconds);
        self.last_error = None;
        self
    }

    pub fn mark_succeeded(
        mut self,
        cursor: Option<String>,
        successful_at_epoch_seconds: i64,
        next_due_at_epoch_seconds: i64,
    ) -> Self {
        self.cursor = cursor;
        self.next_due_at_epoch_seconds = next_due_at_epoch_seconds;
        self.last_attempted_at_epoch_seconds = Some(successful_at_epoch_seconds);
        self.last_successful_at_epoch_seconds = Some(successful_at_epoch_seconds);
        self.last_error = None;
        self.backoff_until_epoch_seconds = None;
        self
    }

    pub fn mark_failed(
        mut self,
        error: String,
        attempted_at_epoch_seconds: i64,
        next_due_at_epoch_seconds: i64,
        backoff_until_epoch_seconds: Option<i64>,
    ) -> Self {
        self.last_attempted_at_epoch_seconds = Some(attempted_at_epoch_seconds);
        self.last_error = Some(error);
        self.next_due_at_epoch_seconds = next_due_at_epoch_seconds;
        self.backoff_until_epoch_seconds = backoff_until_epoch_seconds;
        self
    }

    pub fn mark_due_soon(mut self, now_epoch_seconds: i64) -> Self {
        self.next_due_at_epoch_seconds = now_epoch_seconds;
        self.backoff_until_epoch_seconds = None;
        self
    }
}
