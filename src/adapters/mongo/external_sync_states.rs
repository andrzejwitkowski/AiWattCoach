use mongodb::{bson::doc, options::IndexOptions, Collection, IndexModel};
use serde::{Deserialize, Serialize};

use crate::domain::external_sync::{
    BoxFuture, CanonicalEntityKind, CanonicalEntityRef, ConflictStatus, ExternalProvider,
    ExternalSyncRepositoryError, ExternalSyncState, ExternalSyncStateRepository,
    ExternalSyncStatus,
};

#[derive(Clone)]
pub struct MongoExternalSyncStateRepository {
    collection: Collection<ExternalSyncStateDocument>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ExternalSyncStateDocument {
    user_id: String,
    provider: String,
    canonical_entity_kind: String,
    canonical_entity_id: String,
    external_id: Option<String>,
    sync_status: String,
    last_synced_payload_hash: Option<String>,
    last_seen_remote_payload_hash: Option<String>,
    last_error: Option<String>,
    last_synced_at_epoch_seconds: Option<i64>,
    last_seen_remote_at_epoch_seconds: Option<i64>,
    conflict_status: String,
}

impl MongoExternalSyncStateRepository {
    pub fn new(client: mongodb::Client, database: impl AsRef<str>) -> Self {
        Self {
            collection: client
                .database(database.as_ref())
                .collection("external_sync_states"),
        }
    }

    pub async fn ensure_indexes(&self) -> Result<(), ExternalSyncRepositoryError> {
        self.collection
            .create_indexes([
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "provider": 1, "canonical_entity_kind": 1, "canonical_entity_id": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("external_sync_states_user_provider_entity_unique".to_string())
                            .unique(true)
                            .build(),
                    )
                    .build(),
            ])
            .await
            .map_err(storage_error)?;
        Ok(())
    }
}

impl ExternalSyncStateRepository for MongoExternalSyncStateRepository {
    fn upsert(
        &self,
        state: ExternalSyncState,
    ) -> BoxFuture<Result<ExternalSyncState, ExternalSyncRepositoryError>> {
        let collection = self.collection.clone();
        let document = map_sync_state_to_document(&state);
        Box::pin(async move {
            collection
                .replace_one(
                    doc! {
                        "user_id": &document.user_id,
                        "provider": &document.provider,
                        "canonical_entity_kind": &document.canonical_entity_kind,
                        "canonical_entity_id": &document.canonical_entity_id,
                    },
                    &document,
                )
                .upsert(true)
                .await
                .map_err(storage_error)?;
            Ok(state)
        })
    }

    fn find_by_provider_and_canonical_entity(
        &self,
        user_id: &str,
        provider: ExternalProvider,
        canonical_entity: &CanonicalEntityRef,
    ) -> BoxFuture<Result<Option<ExternalSyncState>, ExternalSyncRepositoryError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let provider = provider_as_str(&provider).to_string();
        let canonical_entity_kind =
            canonical_entity_kind_as_str(&canonical_entity.entity_kind).to_string();
        let canonical_entity_id = canonical_entity.entity_id.clone();
        Box::pin(async move {
            let document = collection
                .find_one(doc! {
                    "user_id": &user_id,
                    "provider": &provider,
                    "canonical_entity_kind": &canonical_entity_kind,
                    "canonical_entity_id": &canonical_entity_id,
                })
                .await
                .map_err(storage_error)?;

            Ok(document.map(map_document_to_sync_state))
        })
    }

    fn delete_by_provider_and_canonical_entity(
        &self,
        user_id: &str,
        provider: ExternalProvider,
        canonical_entity: &CanonicalEntityRef,
    ) -> BoxFuture<Result<(), ExternalSyncRepositoryError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let provider = provider_as_str(&provider).to_string();
        let canonical_entity_kind =
            canonical_entity_kind_as_str(&canonical_entity.entity_kind).to_string();
        let canonical_entity_id = canonical_entity.entity_id.clone();
        Box::pin(async move {
            collection
                .delete_one(doc! {
                    "user_id": &user_id,
                    "provider": &provider,
                    "canonical_entity_kind": &canonical_entity_kind,
                    "canonical_entity_id": &canonical_entity_id,
                })
                .await
                .map_err(storage_error)?;
            Ok(())
        })
    }
}

fn storage_error(error: mongodb::error::Error) -> ExternalSyncRepositoryError {
    ExternalSyncRepositoryError::Storage(error.to_string())
}

fn map_sync_state_to_document(state: &ExternalSyncState) -> ExternalSyncStateDocument {
    ExternalSyncStateDocument {
        user_id: state.user_id.clone(),
        provider: provider_as_str(&state.provider).to_string(),
        canonical_entity_kind: canonical_entity_kind_as_str(&state.canonical_entity.entity_kind)
            .to_string(),
        canonical_entity_id: state.canonical_entity.entity_id.clone(),
        external_id: state.external_id.clone(),
        sync_status: sync_status_as_str(&state.sync_status).to_string(),
        last_synced_payload_hash: state.last_synced_payload_hash.clone(),
        last_seen_remote_payload_hash: state.last_seen_remote_payload_hash.clone(),
        last_error: state.last_error.clone(),
        last_synced_at_epoch_seconds: state.last_synced_at_epoch_seconds,
        last_seen_remote_at_epoch_seconds: state.last_seen_remote_at_epoch_seconds,
        conflict_status: conflict_status_as_str(&state.conflict_status).to_string(),
    }
}

fn map_document_to_sync_state(document: ExternalSyncStateDocument) -> ExternalSyncState {
    ExternalSyncState {
        user_id: document.user_id,
        provider: map_provider(&document.provider),
        canonical_entity: CanonicalEntityRef {
            entity_kind: map_canonical_entity_kind(&document.canonical_entity_kind),
            entity_id: document.canonical_entity_id,
        },
        external_id: document.external_id,
        sync_status: map_sync_status(&document.sync_status),
        last_synced_payload_hash: document.last_synced_payload_hash,
        last_seen_remote_payload_hash: document.last_seen_remote_payload_hash,
        last_error: document.last_error,
        last_synced_at_epoch_seconds: document.last_synced_at_epoch_seconds,
        last_seen_remote_at_epoch_seconds: document.last_seen_remote_at_epoch_seconds,
        conflict_status: map_conflict_status(&document.conflict_status),
    }
}

fn provider_as_str(provider: &ExternalProvider) -> &'static str {
    match provider {
        ExternalProvider::Intervals => "intervals",
        ExternalProvider::Wahoo => "wahoo",
        ExternalProvider::Strava => "strava",
        ExternalProvider::Other => "other",
    }
}

fn canonical_entity_kind_as_str(kind: &CanonicalEntityKind) -> &'static str {
    match kind {
        CanonicalEntityKind::PlannedWorkout => "planned_workout",
        CanonicalEntityKind::CompletedWorkout => "completed_workout",
        CanonicalEntityKind::Race => "race",
        CanonicalEntityKind::SpecialDay => "special_day",
    }
}

fn conflict_status_as_str(status: &ConflictStatus) -> &'static str {
    match status {
        ConflictStatus::Unknown => "unknown",
        ConflictStatus::InSync => "in_sync",
        ConflictStatus::ConflictDetected => "conflict_detected",
    }
}

fn sync_status_as_str(status: &ExternalSyncStatus) -> &'static str {
    match status {
        ExternalSyncStatus::Pending => "pending",
        ExternalSyncStatus::Synced => "synced",
        ExternalSyncStatus::Failed => "failed",
        ExternalSyncStatus::PendingDelete => "pending_delete",
    }
}

fn map_provider(value: &str) -> ExternalProvider {
    match value {
        "intervals" => ExternalProvider::Intervals,
        "wahoo" => ExternalProvider::Wahoo,
        "strava" => ExternalProvider::Strava,
        _ => ExternalProvider::Other,
    }
}

fn map_canonical_entity_kind(value: &str) -> CanonicalEntityKind {
    match value {
        "planned_workout" => CanonicalEntityKind::PlannedWorkout,
        "completed_workout" => CanonicalEntityKind::CompletedWorkout,
        "race" => CanonicalEntityKind::Race,
        _ => CanonicalEntityKind::SpecialDay,
    }
}

fn map_conflict_status(value: &str) -> ConflictStatus {
    match value {
        "in_sync" => ConflictStatus::InSync,
        "conflict_detected" => ConflictStatus::ConflictDetected,
        _ => ConflictStatus::Unknown,
    }
}

fn map_sync_status(value: &str) -> ExternalSyncStatus {
    match value {
        "synced" => ExternalSyncStatus::Synced,
        "failed" => ExternalSyncStatus::Failed,
        "pending_delete" => ExternalSyncStatus::PendingDelete,
        _ => ExternalSyncStatus::Pending,
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::external_sync::{
        CanonicalEntityKind, CanonicalEntityRef, ConflictStatus, ExternalProvider,
        ExternalSyncState, ExternalSyncStatus,
    };

    use super::{map_document_to_sync_state, map_sync_state_to_document};

    #[test]
    fn sync_state_document_round_trip_preserves_fields() {
        let state = ExternalSyncState::new(
            "user-1".to_string(),
            ExternalProvider::Intervals,
            CanonicalEntityRef::new(CanonicalEntityKind::Race, "race-1".to_string()),
        )
        .mark_synced("77".to_string(), "hash-1".to_string(), 1_700_000_000)
        .observe_remote("hash-2".to_string(), 1_700_000_100);

        let mapped = map_document_to_sync_state(map_sync_state_to_document(&state));

        assert_eq!(mapped.conflict_status, ConflictStatus::ConflictDetected);
        assert_eq!(mapped.external_id.as_deref(), Some("77"));
        assert_eq!(mapped.sync_status, ExternalSyncStatus::Synced);
        assert_eq!(mapped, state);
    }
}
