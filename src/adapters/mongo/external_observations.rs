use futures::TryStreamExt;
use mongodb::{bson::doc, options::IndexOptions, Collection, IndexModel};
use serde::{Deserialize, Serialize};

use crate::domain::external_sync::{
    BoxFuture, CanonicalEntityKind, CanonicalEntityRef, ExternalObjectKind, ExternalObservation,
    ExternalObservationRepository, ExternalProvider, ExternalSyncRepositoryError,
};

#[derive(Clone)]
pub struct MongoExternalObservationRepository {
    collection: Collection<ExternalObservationDocument>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ExternalObservationDocument {
    user_id: String,
    provider: String,
    external_object_kind: String,
    external_id: String,
    canonical_entity_kind: String,
    canonical_entity_id: String,
    normalized_payload_hash: Option<String>,
    dedup_key: Option<String>,
    observed_at_epoch_seconds: i64,
}

impl MongoExternalObservationRepository {
    pub fn new(client: mongodb::Client, database: impl AsRef<str>) -> Self {
        Self {
            collection: client
                .database(database.as_ref())
                .collection("external_observations"),
        }
    }

    pub async fn ensure_indexes(&self) -> Result<(), ExternalSyncRepositoryError> {
        self.collection
            .create_indexes([
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "provider": 1, "external_id": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("external_observations_user_provider_external_unique".to_string())
                            .unique(true)
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(
                        doc! { "user_id": 1, "canonical_entity_kind": 1, "canonical_entity_id": 1 },
                    )
                    .options(
                        IndexOptions::builder()
                            .name("external_observations_user_canonical_entity".to_string())
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "external_object_kind": 1, "dedup_key": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("external_observations_user_object_dedup_key".to_string())
                            .build(),
                    )
                    .build(),
            ])
            .await
            .map_err(storage_error)?;
        Ok(())
    }
}

impl ExternalObservationRepository for MongoExternalObservationRepository {
    fn upsert(
        &self,
        observation: ExternalObservation,
    ) -> BoxFuture<Result<ExternalObservation, ExternalSyncRepositoryError>> {
        let collection = self.collection.clone();
        let document = map_observation_to_document(&observation);
        Box::pin(async move {
            collection
                .replace_one(
                    doc! {
                        "user_id": &document.user_id,
                        "provider": &document.provider,
                        "external_id": &document.external_id,
                    },
                    &document,
                )
                .upsert(true)
                .await
                .map_err(storage_error)?;
            Ok(observation)
        })
    }

    fn find_by_provider_and_external_id(
        &self,
        user_id: &str,
        provider: ExternalProvider,
        external_id: &str,
    ) -> BoxFuture<Result<Option<ExternalObservation>, ExternalSyncRepositoryError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let provider = provider_as_str(&provider).to_string();
        let external_id = external_id.to_string();
        Box::pin(async move {
            let document = collection
                .find_one(doc! {
                    "user_id": &user_id,
                    "provider": &provider,
                    "external_id": &external_id,
                })
                .await
                .map_err(storage_error)?;

            Ok(document.map(map_document_to_observation))
        })
    }

    fn find_by_dedup_key(
        &self,
        user_id: &str,
        external_object_kind: ExternalObjectKind,
        dedup_key: &str,
    ) -> BoxFuture<Result<Vec<ExternalObservation>, ExternalSyncRepositoryError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let external_object_kind = external_object_kind_as_str(&external_object_kind).to_string();
        let dedup_key = dedup_key.to_string();
        Box::pin(async move {
            collection
                .find(doc! {
                    "user_id": &user_id,
                    "external_object_kind": &external_object_kind,
                    "dedup_key": &dedup_key,
                })
                .await
                .map_err(storage_error)?
                .try_collect::<Vec<_>>()
                .await
                .map_err(storage_error)
                .map(|documents| {
                    documents
                        .into_iter()
                        .map(map_document_to_observation)
                        .collect()
                })
        })
    }
}

fn storage_error(error: mongodb::error::Error) -> ExternalSyncRepositoryError {
    ExternalSyncRepositoryError::Storage(error.to_string())
}

fn map_observation_to_document(observation: &ExternalObservation) -> ExternalObservationDocument {
    ExternalObservationDocument {
        user_id: observation.user_id.clone(),
        provider: provider_as_str(&observation.provider).to_string(),
        external_object_kind: external_object_kind_as_str(&observation.external_object_kind)
            .to_string(),
        external_id: observation.external_id.clone(),
        canonical_entity_kind: canonical_entity_kind_as_str(
            &observation.canonical_entity.entity_kind,
        )
        .to_string(),
        canonical_entity_id: observation.canonical_entity.entity_id.clone(),
        normalized_payload_hash: observation.normalized_payload_hash.clone(),
        dedup_key: observation.dedup_key.clone(),
        observed_at_epoch_seconds: observation.observed_at_epoch_seconds,
    }
}

fn map_document_to_observation(document: ExternalObservationDocument) -> ExternalObservation {
    ExternalObservation {
        user_id: document.user_id,
        provider: map_provider(&document.provider),
        external_object_kind: map_external_object_kind(&document.external_object_kind),
        external_id: document.external_id,
        canonical_entity: CanonicalEntityRef {
            entity_kind: map_canonical_entity_kind(&document.canonical_entity_kind),
            entity_id: document.canonical_entity_id,
        },
        normalized_payload_hash: document.normalized_payload_hash,
        dedup_key: document.dedup_key,
        observed_at_epoch_seconds: document.observed_at_epoch_seconds,
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

fn external_object_kind_as_str(kind: &ExternalObjectKind) -> &'static str {
    match kind {
        ExternalObjectKind::PlannedWorkout => "planned_workout",
        ExternalObjectKind::CompletedWorkout => "completed_workout",
        ExternalObjectKind::Race => "race",
        ExternalObjectKind::SpecialDay => "special_day",
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

fn map_provider(value: &str) -> ExternalProvider {
    match value {
        "intervals" => ExternalProvider::Intervals,
        "wahoo" => ExternalProvider::Wahoo,
        "strava" => ExternalProvider::Strava,
        _ => ExternalProvider::Other,
    }
}

fn map_external_object_kind(value: &str) -> ExternalObjectKind {
    match value {
        "planned_workout" => ExternalObjectKind::PlannedWorkout,
        "completed_workout" => ExternalObjectKind::CompletedWorkout,
        "race" => ExternalObjectKind::Race,
        _ => ExternalObjectKind::SpecialDay,
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

#[cfg(test)]
mod tests {
    use crate::domain::external_sync::{
        CanonicalEntityKind, CanonicalEntityRef, ExternalObjectKind, ExternalObservation,
        ExternalObservationParams, ExternalProvider,
    };

    use super::{map_document_to_observation, map_observation_to_document};

    #[test]
    fn observation_document_round_trip_preserves_fields() {
        let observation = ExternalObservation::new(ExternalObservationParams {
            user_id: "user-1".to_string(),
            provider: ExternalProvider::Intervals,
            external_object_kind: ExternalObjectKind::Race,
            external_id: "remote-1".to_string(),
            canonical_entity: CanonicalEntityRef::new(
                CanonicalEntityKind::Race,
                "race-1".to_string(),
            ),
            normalized_payload_hash: Some("hash-1".to_string()),
            dedup_key: Some("dedup-1".to_string()),
            observed_at_epoch_seconds: 1_700_000_000,
        });

        let mapped = map_document_to_observation(map_observation_to_document(&observation));

        assert_eq!(mapped, observation);
    }
}
