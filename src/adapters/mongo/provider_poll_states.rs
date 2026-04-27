use futures::TryStreamExt;
use mongodb::{
    bson::{doc, Bson, DateTime, Document},
    options::IndexOptions,
    Collection, IndexModel,
};
use serde::{Deserialize, Serialize};

use super::time::{optional_epoch_seconds_to_bson_datetime, resolve_optional_epoch_seconds};
use crate::domain::external_sync::{
    BoxFuture, ExternalProvider, ExternalSyncRepositoryError, ProviderPollState,
    ProviderPollStateRepository, ProviderPollStream,
};

#[derive(Clone)]
pub struct MongoProviderPollStateRepository {
    collection: Collection<ProviderPollStateDocument>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ProviderPollStateDocument {
    user_id: String,
    provider: String,
    stream: String,
    cursor: Option<String>,
    next_due_at_epoch_seconds: Option<i64>,
    #[serde(default)]
    next_due_at: Option<DateTime>,
    last_attempted_at_epoch_seconds: Option<i64>,
    #[serde(default)]
    last_attempted_at: Option<DateTime>,
    last_successful_at_epoch_seconds: Option<i64>,
    #[serde(default)]
    last_successful_at: Option<DateTime>,
    last_error: Option<String>,
    backoff_until_epoch_seconds: Option<i64>,
    #[serde(default)]
    backoff_until_at: Option<DateTime>,
}

impl MongoProviderPollStateRepository {
    pub fn new(client: mongodb::Client, database: impl AsRef<str>) -> Self {
        Self {
            collection: client
                .database(database.as_ref())
                .collection("provider_poll_states"),
        }
    }

    pub async fn ensure_indexes(&self) -> Result<(), ExternalSyncRepositoryError> {
        self.collection
            .create_indexes([
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "provider": 1, "stream": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("provider_poll_states_user_provider_stream_unique".to_string())
                            .unique(true)
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "next_due_at_epoch_seconds": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("provider_poll_states_next_due_at".to_string())
                            .build(),
                    )
                    .build(),
            ])
            .await
            .map_err(storage_error)?;
        Ok(())
    }

    pub async fn list_user_ids_for_provider(
        &self,
        provider: ExternalProvider,
    ) -> Result<Vec<String>, ExternalSyncRepositoryError> {
        #[derive(Deserialize)]
        struct UserIdDocument {
            user_id: String,
        }

        let collection = self.collection.clone_with_type::<UserIdDocument>();
        collection
            .find(doc! { "provider": provider_as_str(&provider) })
            .projection(doc! { "_id": 0, "user_id": 1 })
            .sort(doc! { "user_id": 1 })
            .await
            .map_err(storage_error)?
            .try_collect::<Vec<_>>()
            .await
            .map_err(storage_error)
            .map(|documents| {
                documents
                    .into_iter()
                    .map(|document| document.user_id)
                    .collect()
            })
    }
}

impl ProviderPollStateRepository for MongoProviderPollStateRepository {
    fn upsert(
        &self,
        state: ProviderPollState,
    ) -> BoxFuture<Result<ProviderPollState, ExternalSyncRepositoryError>> {
        let collection = self.collection.clone();
        let document = map_poll_state_to_document(&state);
        Box::pin(async move {
            collection
                .replace_one(
                    doc! {
                        "user_id": &document.user_id,
                        "provider": &document.provider,
                        "stream": &document.stream,
                    },
                    &document,
                )
                .upsert(true)
                .await
                .map_err(storage_error)?;
            Ok(state)
        })
    }

    fn list_due(
        &self,
        now_epoch_seconds: i64,
    ) -> BoxFuture<Result<Vec<ProviderPollState>, ExternalSyncRepositoryError>> {
        let collection = self.collection.clone();
        Box::pin(async move {
            let documents = collection
                .find(build_due_filter(now_epoch_seconds))
                .sort(doc! { "next_due_at_epoch_seconds": 1, "user_id": 1, "provider": 1, "stream": 1 })
                .await
                .map_err(storage_error)?
                .try_collect::<Vec<_>>()
                .await
                .map_err(storage_error)?;

            documents
                .into_iter()
                .map(map_document_to_poll_state)
                .collect::<Result<Vec<_>, _>>()
        })
    }

    fn find_by_provider_and_stream(
        &self,
        user_id: &str,
        provider: ExternalProvider,
        stream: ProviderPollStream,
    ) -> BoxFuture<Result<Option<ProviderPollState>, ExternalSyncRepositoryError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let provider = provider_as_str(&provider).to_string();
        let stream = stream_as_str(&stream).to_string();
        Box::pin(async move {
            let document = collection
                .find_one(doc! {
                    "user_id": &user_id,
                    "provider": &provider,
                    "stream": &stream,
                })
                .await
                .map_err(storage_error)?;

            document.map(map_document_to_poll_state).transpose()
        })
    }
}

fn map_poll_state_to_document(state: &ProviderPollState) -> ProviderPollStateDocument {
    ProviderPollStateDocument {
        user_id: state.user_id.clone(),
        provider: provider_as_str(&state.provider).to_string(),
        stream: stream_as_str(&state.stream).to_string(),
        cursor: state.cursor.clone(),
        next_due_at_epoch_seconds: Some(state.next_due_at_epoch_seconds),
        next_due_at: optional_epoch_seconds_to_bson_datetime(
            Some(state.next_due_at_epoch_seconds),
            "next_due_at",
        )
        .expect("next_due_at should fit BSON DateTime"),
        last_attempted_at_epoch_seconds: state.last_attempted_at_epoch_seconds,
        last_attempted_at: optional_epoch_seconds_to_bson_datetime(
            state.last_attempted_at_epoch_seconds,
            "last_attempted_at",
        )
        .expect("last_attempted_at should fit BSON DateTime"),
        last_successful_at_epoch_seconds: state.last_successful_at_epoch_seconds,
        last_successful_at: optional_epoch_seconds_to_bson_datetime(
            state.last_successful_at_epoch_seconds,
            "last_successful_at",
        )
        .expect("last_successful_at should fit BSON DateTime"),
        last_error: state.last_error.clone(),
        backoff_until_epoch_seconds: state.backoff_until_epoch_seconds,
        backoff_until_at: optional_epoch_seconds_to_bson_datetime(
            state.backoff_until_epoch_seconds,
            "backoff_until_at",
        )
        .expect("backoff_until_at should fit BSON DateTime"),
    }
}

fn map_document_to_poll_state(
    document: ProviderPollStateDocument,
) -> Result<ProviderPollState, ExternalSyncRepositoryError> {
    Ok(ProviderPollState {
        user_id: document.user_id,
        provider: map_provider(&document.provider),
        stream: map_stream(&document.stream)?,
        cursor: document.cursor,
        next_due_at_epoch_seconds: resolve_optional_epoch_seconds(
            document.next_due_at,
            document.next_due_at_epoch_seconds,
        )
        .expect("provider poll state documents must store next_due_at"),
        last_attempted_at_epoch_seconds: resolve_optional_epoch_seconds(
            document.last_attempted_at,
            document.last_attempted_at_epoch_seconds,
        ),
        last_successful_at_epoch_seconds: resolve_optional_epoch_seconds(
            document.last_successful_at,
            document.last_successful_at_epoch_seconds,
        ),
        last_error: document.last_error,
        backoff_until_epoch_seconds: resolve_optional_epoch_seconds(
            document.backoff_until_at,
            document.backoff_until_epoch_seconds,
        ),
    })
}

fn provider_as_str(provider: &ExternalProvider) -> &'static str {
    match provider {
        ExternalProvider::Intervals => "intervals",
        ExternalProvider::Wahoo => "wahoo",
        ExternalProvider::Strava => "strava",
        ExternalProvider::Other => "other",
    }
}

fn stream_as_str(stream: &ProviderPollStream) -> &'static str {
    match stream {
        ProviderPollStream::Calendar => "calendar",
        ProviderPollStream::CompletedWorkouts => "completed_workouts",
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

fn map_stream(value: &str) -> Result<ProviderPollStream, ExternalSyncRepositoryError> {
    match value {
        "calendar" => Ok(ProviderPollStream::Calendar),
        "completed_workouts" => Ok(ProviderPollStream::CompletedWorkouts),
        other => Err(ExternalSyncRepositoryError::CorruptData(format!(
            "unknown provider poll stream: {other}"
        ))),
    }
}

fn storage_error(error: mongodb::error::Error) -> ExternalSyncRepositoryError {
    ExternalSyncRepositoryError::Storage(error.to_string())
}

fn build_due_filter(now_epoch_seconds: i64) -> Document {
    doc! {
        "next_due_at_epoch_seconds": { "$lte": now_epoch_seconds },
        "$or": [
            { "backoff_until_epoch_seconds": { "$exists": false } },
            { "backoff_until_epoch_seconds": Bson::Null },
            { "backoff_until_epoch_seconds": { "$lte": now_epoch_seconds } },
        ],
    }
}

#[cfg(test)]
mod tests {
    use mongodb::bson::{doc, Bson};

    use crate::domain::external_sync::{ExternalProvider, ProviderPollState, ProviderPollStream};

    use super::{
        build_due_filter, map_document_to_poll_state, map_poll_state_to_document,
        ProviderPollStateDocument,
    };

    #[test]
    fn poll_state_document_round_trip_preserves_fields() {
        let state = ProviderPollState {
            user_id: "user-1".to_string(),
            provider: ExternalProvider::Intervals,
            stream: ProviderPollStream::Calendar,
            cursor: Some("cursor-1".to_string()),
            next_due_at_epoch_seconds: 1_700_000_000,
            last_attempted_at_epoch_seconds: Some(1_700_000_001),
            last_successful_at_epoch_seconds: Some(1_700_000_002),
            last_error: Some("temporary upstream error".to_string()),
            backoff_until_epoch_seconds: Some(1_700_000_300),
        };

        let mapped = map_document_to_poll_state(map_poll_state_to_document(&state)).unwrap();

        assert_eq!(mapped, state);
    }

    #[test]
    fn poll_state_document_rejects_unknown_stream() {
        let error = map_document_to_poll_state(ProviderPollStateDocument {
            user_id: "user-1".to_string(),
            provider: "intervals".to_string(),
            stream: "mystery".to_string(),
            cursor: None,
            next_due_at_epoch_seconds: Some(1_700_000_000),
            next_due_at: None,
            last_attempted_at_epoch_seconds: None,
            last_attempted_at: None,
            last_successful_at_epoch_seconds: None,
            last_successful_at: None,
            last_error: None,
            backoff_until_epoch_seconds: None,
            backoff_until_at: None,
        })
        .unwrap_err();

        assert!(matches!(
            error,
            crate::domain::external_sync::ExternalSyncRepositoryError::CorruptData(_)
        ));
    }

    #[test]
    fn due_filter_respects_backoff_window() {
        let filter = build_due_filter(1_700_000_000);

        assert_eq!(
            filter.get_document("next_due_at_epoch_seconds").unwrap(),
            &doc! { "$lte": Bson::Int64(1_700_000_000) }
        );
        assert_eq!(
            filter.get_array("$or").unwrap(),
            &vec![
                Bson::Document(doc! { "backoff_until_epoch_seconds": { "$exists": false } }),
                Bson::Document(doc! { "backoff_until_epoch_seconds": Bson::Null }),
                Bson::Document(
                    doc! { "backoff_until_epoch_seconds": { "$lte": Bson::Int64(1_700_000_000) } },
                ),
            ]
        );
    }

    #[test]
    fn poll_state_document_reads_datetime_fields_without_legacy_epoch() {
        let mapped = map_document_to_poll_state(ProviderPollStateDocument {
            user_id: "user-1".to_string(),
            provider: "intervals".to_string(),
            stream: "calendar".to_string(),
            cursor: Some("cursor-1".to_string()),
            next_due_at_epoch_seconds: None,
            next_due_at: Some(mongodb::bson::DateTime::from_millis(1_700_000_000_000)),
            last_attempted_at_epoch_seconds: None,
            last_attempted_at: Some(mongodb::bson::DateTime::from_millis(1_700_000_010_000)),
            last_successful_at_epoch_seconds: None,
            last_successful_at: Some(mongodb::bson::DateTime::from_millis(1_700_000_020_000)),
            last_error: Some("temporary upstream error".to_string()),
            backoff_until_epoch_seconds: None,
            backoff_until_at: Some(mongodb::bson::DateTime::from_millis(1_700_000_030_000)),
        })
        .expect("datetime-backed poll state should map");

        assert_eq!(mapped.next_due_at_epoch_seconds, 1_700_000_000);
        assert_eq!(mapped.last_attempted_at_epoch_seconds, Some(1_700_000_010));
        assert_eq!(mapped.last_successful_at_epoch_seconds, Some(1_700_000_020));
        assert_eq!(mapped.backoff_until_epoch_seconds, Some(1_700_000_030));
    }
}
