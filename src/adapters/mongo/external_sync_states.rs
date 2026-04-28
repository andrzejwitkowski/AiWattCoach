use futures::TryStreamExt;
use mongodb::{
    bson::{doc, Bson, DateTime},
    options::IndexOptions,
    Collection, IndexModel,
};
use serde::{Deserialize, Serialize};

use super::time::{optional_epoch_seconds_to_bson_datetime, resolve_optional_epoch_seconds};
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
    #[serde(default)]
    wahoo_plan_external_id: Option<String>,
    #[serde(default)]
    wahoo_plan_id: Option<i64>,
    #[serde(default)]
    wahoo_workout_id: Option<i64>,
    #[serde(default)]
    wahoo_workout_token: Option<String>,
    sync_status: String,
    last_synced_payload_hash: Option<String>,
    last_seen_remote_payload_hash: Option<String>,
    last_error: Option<String>,
    last_synced_at_epoch_seconds: Option<i64>,
    #[serde(default)]
    last_synced_at: Option<DateTime>,
    last_seen_remote_at_epoch_seconds: Option<i64>,
    #[serde(default)]
    last_seen_remote_at: Option<DateTime>,
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
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "canonical_entity_kind": 1, "canonical_entity_id": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("external_sync_states_user_entity".to_string())
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "provider": 1, "wahoo_plan_id": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("external_sync_states_user_provider_wahoo_plan_id".to_string())
                            .unique(true)
                            .partial_filter_expression(doc! {
                                "wahoo_plan_id": { "$exists": true, "$ne": Bson::Null }
                            })
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "provider": 1, "wahoo_workout_token": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("external_sync_states_user_provider_wahoo_workout_token".to_string())
                            .unique(true)
                            .partial_filter_expression(doc! {
                                "wahoo_workout_token": { "$exists": true, "$ne": Bson::Null }
                            })
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

    fn find_by_canonical_entities(
        &self,
        user_id: &str,
        canonical_entities: &[CanonicalEntityRef],
    ) -> BoxFuture<Result<Vec<ExternalSyncState>, ExternalSyncRepositoryError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let canonical_entities = canonical_entities.to_vec();
        Box::pin(async move {
            if canonical_entities.is_empty() {
                return Ok(Vec::new());
            }

            let entity_filters = canonical_entities
                .into_iter()
                .map(|canonical_entity| {
                    doc! {
                        "canonical_entity_kind": canonical_entity_kind_as_str(&canonical_entity.entity_kind),
                        "canonical_entity_id": canonical_entity.entity_id,
                    }
                })
                .collect::<Vec<_>>();

            let documents = collection
                .find(doc! {
                    "user_id": &user_id,
                    "$or": entity_filters,
                })
                .await
                .map_err(storage_error)?
                .try_collect::<Vec<_>>()
                .await
                .map_err(storage_error)?;

            documents
                .into_iter()
                .map(map_document_to_sync_state)
                .collect::<Result<Vec<_>, _>>()
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

            document.map(map_document_to_sync_state).transpose()
        })
    }

    fn find_by_provider_and_canonical_entities(
        &self,
        user_id: &str,
        provider: ExternalProvider,
        canonical_entities: &[CanonicalEntityRef],
    ) -> BoxFuture<Result<Vec<ExternalSyncState>, ExternalSyncRepositoryError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let provider = provider_as_str(&provider).to_string();
        let canonical_entities = canonical_entities.to_vec();
        Box::pin(async move {
            if canonical_entities.is_empty() {
                return Ok(Vec::new());
            }

            let entity_filters = canonical_entities
                .into_iter()
                .map(|canonical_entity| {
                    doc! {
                        "canonical_entity_kind": canonical_entity_kind_as_str(&canonical_entity.entity_kind),
                        "canonical_entity_id": canonical_entity.entity_id,
                    }
                })
                .collect::<Vec<_>>();

            let documents = collection
                .find(doc! {
                    "user_id": &user_id,
                    "provider": &provider,
                    "$or": entity_filters,
                })
                .await
                .map_err(storage_error)?
                .try_collect::<Vec<_>>()
                .await
                .map_err(storage_error)?;

            documents
                .into_iter()
                .map(map_document_to_sync_state)
                .collect::<Result<Vec<_>, _>>()
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

    fn find_by_wahoo_plan_id(
        &self,
        user_id: &str,
        wahoo_plan_id: i64,
    ) -> BoxFuture<Result<Option<ExternalSyncState>, ExternalSyncRepositoryError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            find_unique_sync_state(
                &collection,
                doc! {
                    "user_id": &user_id,
                    "provider": "wahoo",
                    "wahoo_plan_id": wahoo_plan_id,
                },
                "wahoo plan id",
            )
            .await
        })
    }

    fn find_by_wahoo_workout_token(
        &self,
        user_id: &str,
        wahoo_workout_token: &str,
    ) -> BoxFuture<Result<Option<ExternalSyncState>, ExternalSyncRepositoryError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let wahoo_workout_token = wahoo_workout_token.to_string();
        Box::pin(async move {
            find_unique_sync_state(
                &collection,
                doc! {
                    "user_id": &user_id,
                    "provider": "wahoo",
                    "wahoo_workout_token": &wahoo_workout_token,
                },
                "wahoo workout token",
            )
            .await
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
        wahoo_plan_external_id: state.wahoo_plan_external_id.clone(),
        wahoo_plan_id: state.wahoo_plan_id,
        wahoo_workout_id: state.wahoo_workout_id,
        wahoo_workout_token: state.wahoo_workout_token.clone(),
        sync_status: sync_status_as_str(&state.sync_status).to_string(),
        last_synced_payload_hash: state.last_synced_payload_hash.clone(),
        last_seen_remote_payload_hash: state.last_seen_remote_payload_hash.clone(),
        last_error: state.last_error.clone(),
        last_synced_at_epoch_seconds: state.last_synced_at_epoch_seconds,
        last_synced_at: optional_epoch_seconds_to_bson_datetime(
            state.last_synced_at_epoch_seconds,
            "last_synced_at",
        )
        .expect("last_synced_at should fit BSON DateTime"),
        last_seen_remote_at_epoch_seconds: state.last_seen_remote_at_epoch_seconds,
        last_seen_remote_at: optional_epoch_seconds_to_bson_datetime(
            state.last_seen_remote_at_epoch_seconds,
            "last_seen_remote_at",
        )
        .expect("last_seen_remote_at should fit BSON DateTime"),
        conflict_status: conflict_status_as_str(&state.conflict_status).to_string(),
    }
}

fn map_document_to_sync_state(
    document: ExternalSyncStateDocument,
) -> Result<ExternalSyncState, ExternalSyncRepositoryError> {
    Ok(ExternalSyncState {
        user_id: document.user_id,
        provider: map_provider(&document.provider)?,
        canonical_entity: CanonicalEntityRef {
            entity_kind: map_canonical_entity_kind(&document.canonical_entity_kind)?,
            entity_id: document.canonical_entity_id,
        },
        external_id: document.external_id,
        wahoo_plan_external_id: document.wahoo_plan_external_id,
        wahoo_plan_id: document.wahoo_plan_id,
        wahoo_workout_id: document.wahoo_workout_id,
        wahoo_workout_token: document.wahoo_workout_token,
        sync_status: map_sync_status(&document.sync_status)?,
        last_synced_payload_hash: document.last_synced_payload_hash,
        last_seen_remote_payload_hash: document.last_seen_remote_payload_hash,
        last_error: document.last_error,
        last_synced_at_epoch_seconds: resolve_optional_epoch_seconds(
            document.last_synced_at,
            document.last_synced_at_epoch_seconds,
        ),
        last_seen_remote_at_epoch_seconds: resolve_optional_epoch_seconds(
            document.last_seen_remote_at,
            document.last_seen_remote_at_epoch_seconds,
        ),
        conflict_status: map_conflict_status(&document.conflict_status)?,
    })
}

async fn find_unique_sync_state(
    collection: &Collection<ExternalSyncStateDocument>,
    filter: mongodb::bson::Document,
    lookup_kind: &str,
) -> Result<Option<ExternalSyncState>, ExternalSyncRepositoryError> {
    let documents = collection
        .find(filter)
        .await
        .map_err(storage_error)?
        .try_collect::<Vec<_>>()
        .await
        .map_err(storage_error)?;

    match documents.as_slice() {
        [] => Ok(None),
        [document] => map_document_to_sync_state(document.clone()).map(Some),
        _ => Err(ExternalSyncRepositoryError::CorruptData(format!(
            "multiple external sync states found for {lookup_kind} lookup"
        ))),
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

fn map_provider(value: &str) -> Result<ExternalProvider, ExternalSyncRepositoryError> {
    match value {
        "intervals" => Ok(ExternalProvider::Intervals),
        "wahoo" => Ok(ExternalProvider::Wahoo),
        "strava" => Ok(ExternalProvider::Strava),
        "other" => Ok(ExternalProvider::Other),
        other => Err(ExternalSyncRepositoryError::CorruptData(format!(
            "unknown external sync provider: {other}"
        ))),
    }
}

fn map_canonical_entity_kind(
    value: &str,
) -> Result<CanonicalEntityKind, ExternalSyncRepositoryError> {
    match value {
        "planned_workout" => Ok(CanonicalEntityKind::PlannedWorkout),
        "completed_workout" => Ok(CanonicalEntityKind::CompletedWorkout),
        "race" => Ok(CanonicalEntityKind::Race),
        "special_day" => Ok(CanonicalEntityKind::SpecialDay),
        other => Err(ExternalSyncRepositoryError::CorruptData(format!(
            "unknown canonical entity kind: {other}"
        ))),
    }
}

fn map_conflict_status(value: &str) -> Result<ConflictStatus, ExternalSyncRepositoryError> {
    match value {
        "unknown" => Ok(ConflictStatus::Unknown),
        "in_sync" => Ok(ConflictStatus::InSync),
        "conflict_detected" => Ok(ConflictStatus::ConflictDetected),
        other => Err(ExternalSyncRepositoryError::CorruptData(format!(
            "unknown conflict status: {other}"
        ))),
    }
}

fn map_sync_status(value: &str) -> Result<ExternalSyncStatus, ExternalSyncRepositoryError> {
    match value {
        "pending" => Ok(ExternalSyncStatus::Pending),
        "synced" => Ok(ExternalSyncStatus::Synced),
        "failed" => Ok(ExternalSyncStatus::Failed),
        "pending_delete" => Ok(ExternalSyncStatus::PendingDelete),
        other => Err(ExternalSyncRepositoryError::CorruptData(format!(
            "unknown external sync status: {other}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::external_sync::{
        CanonicalEntityKind, CanonicalEntityRef, ConflictStatus, ExternalProvider,
        ExternalSyncRepositoryError, ExternalSyncState, ExternalSyncStatus,
    };

    use super::{
        map_document_to_sync_state, map_sync_state_to_document, ExternalSyncStateDocument,
    };

    #[test]
    fn sync_state_document_round_trip_preserves_fields() {
        let state = ExternalSyncState::new(
            "user-1".to_string(),
            ExternalProvider::Intervals,
            CanonicalEntityRef::new(CanonicalEntityKind::Race, "race-1".to_string()),
        )
        .mark_synced("77".to_string(), "hash-1".to_string(), 1_700_000_000)
        .observe_remote("hash-2".to_string(), 1_700_000_100);

        let mapped = map_document_to_sync_state(map_sync_state_to_document(&state)).unwrap();

        assert_eq!(mapped.conflict_status, ConflictStatus::ConflictDetected);
        assert_eq!(mapped.external_id.as_deref(), Some("77"));
        assert_eq!(mapped.sync_status, ExternalSyncStatus::Synced);
        assert_eq!(mapped, state);
    }

    #[test]
    fn sync_state_document_rejects_unknown_provider() {
        let error = map_document_to_sync_state(ExternalSyncStateDocument {
            user_id: "user-1".to_string(),
            provider: "mystery".to_string(),
            canonical_entity_kind: "race".to_string(),
            canonical_entity_id: "race-1".to_string(),
            external_id: Some("77".to_string()),
            wahoo_plan_external_id: None,
            wahoo_plan_id: None,
            wahoo_workout_id: None,
            wahoo_workout_token: None,
            sync_status: "synced".to_string(),
            last_synced_payload_hash: Some("hash-1".to_string()),
            last_seen_remote_payload_hash: Some("hash-1".to_string()),
            last_error: None,
            last_synced_at_epoch_seconds: Some(1_700_000_000),
            last_synced_at: None,
            last_seen_remote_at_epoch_seconds: Some(1_700_000_000),
            last_seen_remote_at: None,
            conflict_status: "in_sync".to_string(),
        })
        .unwrap_err();

        assert!(matches!(error, ExternalSyncRepositoryError::CorruptData(_)));
    }

    #[test]
    fn sync_state_document_rejects_unknown_sync_status() {
        let error = map_document_to_sync_state(ExternalSyncStateDocument {
            user_id: "user-1".to_string(),
            provider: "intervals".to_string(),
            canonical_entity_kind: "race".to_string(),
            canonical_entity_id: "race-1".to_string(),
            external_id: Some("77".to_string()),
            wahoo_plan_external_id: None,
            wahoo_plan_id: None,
            wahoo_workout_id: None,
            wahoo_workout_token: None,
            sync_status: "mystery".to_string(),
            last_synced_payload_hash: Some("hash-1".to_string()),
            last_seen_remote_payload_hash: Some("hash-1".to_string()),
            last_error: None,
            last_synced_at_epoch_seconds: Some(1_700_000_000),
            last_synced_at: None,
            last_seen_remote_at_epoch_seconds: Some(1_700_000_000),
            last_seen_remote_at: None,
            conflict_status: "in_sync".to_string(),
        })
        .unwrap_err();

        assert!(matches!(error, ExternalSyncRepositoryError::CorruptData(_)));
    }

    #[test]
    fn sync_state_document_reads_datetime_fields_without_legacy_epoch() {
        let mapped = map_document_to_sync_state(ExternalSyncStateDocument {
            user_id: "user-1".to_string(),
            provider: "intervals".to_string(),
            canonical_entity_kind: "race".to_string(),
            canonical_entity_id: "race-1".to_string(),
            external_id: Some("77".to_string()),
            wahoo_plan_external_id: None,
            wahoo_plan_id: None,
            wahoo_workout_id: None,
            wahoo_workout_token: None,
            sync_status: "synced".to_string(),
            last_synced_payload_hash: Some("hash-1".to_string()),
            last_seen_remote_payload_hash: Some("hash-2".to_string()),
            last_error: None,
            last_synced_at_epoch_seconds: None,
            last_synced_at: Some(mongodb::bson::DateTime::from_millis(1_700_000_000_000)),
            last_seen_remote_at_epoch_seconds: None,
            last_seen_remote_at: Some(mongodb::bson::DateTime::from_millis(1_700_000_100_000)),
            conflict_status: "conflict_detected".to_string(),
        })
        .expect("datetime-backed sync state should map");

        assert_eq!(mapped.last_synced_at_epoch_seconds, Some(1_700_000_000));
        assert_eq!(
            mapped.last_seen_remote_at_epoch_seconds,
            Some(1_700_000_100)
        );
    }
}
