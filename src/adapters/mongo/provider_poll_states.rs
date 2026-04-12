use futures::TryStreamExt;
use mongodb::{bson::doc, options::IndexOptions, Collection, IndexModel};
use serde::{Deserialize, Serialize};

use crate::domain::external_sync::{
    BoxFuture, ExternalProvider, ProviderPollState, ProviderPollStateRepository, ProviderPollStream,
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
    next_due_at_epoch_seconds: i64,
}

impl MongoProviderPollStateRepository {
    pub fn new(client: mongodb::Client, database: impl AsRef<str>) -> Self {
        Self {
            collection: client
                .database(database.as_ref())
                .collection("provider_poll_states"),
        }
    }

    pub async fn ensure_indexes(&self) -> Result<(), std::convert::Infallible> {
        let _ = self
            .collection
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
            .await;
        Ok(())
    }
}

impl ProviderPollStateRepository for MongoProviderPollStateRepository {
    fn upsert(
        &self,
        state: ProviderPollState,
    ) -> BoxFuture<Result<ProviderPollState, std::convert::Infallible>> {
        let collection = self.collection.clone();
        let document = map_poll_state_to_document(&state);
        Box::pin(async move {
            let _ = collection
                .replace_one(
                    doc! {
                        "user_id": &document.user_id,
                        "provider": &document.provider,
                        "stream": &document.stream,
                    },
                    &document,
                )
                .upsert(true)
                .await;
            Ok(state)
        })
    }

    fn list_due(
        &self,
        now_epoch_seconds: i64,
    ) -> BoxFuture<Result<Vec<ProviderPollState>, std::convert::Infallible>> {
        let collection = self.collection.clone();
        Box::pin(async move {
            let documents = match collection
                .find(doc! { "next_due_at_epoch_seconds": { "$lte": now_epoch_seconds } })
                .sort(doc! { "next_due_at_epoch_seconds": 1, "user_id": 1, "provider": 1, "stream": 1 })
                .await
            {
                Ok(cursor) => cursor.try_collect::<Vec<_>>().await.unwrap_or_default(),
                Err(_) => Vec::new(),
            };

            Ok(documents
                .into_iter()
                .map(map_document_to_poll_state)
                .collect())
        })
    }

    fn find_by_provider_and_stream(
        &self,
        user_id: &str,
        provider: ExternalProvider,
        stream: ProviderPollStream,
    ) -> BoxFuture<Result<Option<ProviderPollState>, std::convert::Infallible>> {
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
                .ok()
                .flatten();

            Ok(document.map(map_document_to_poll_state))
        })
    }
}

fn map_poll_state_to_document(state: &ProviderPollState) -> ProviderPollStateDocument {
    ProviderPollStateDocument {
        user_id: state.user_id.clone(),
        provider: provider_as_str(&state.provider).to_string(),
        stream: stream_as_str(&state.stream).to_string(),
        cursor: state.cursor.clone(),
        next_due_at_epoch_seconds: state.next_due_at_epoch_seconds,
    }
}

fn map_document_to_poll_state(document: ProviderPollStateDocument) -> ProviderPollState {
    ProviderPollState {
        user_id: document.user_id,
        provider: map_provider(&document.provider),
        stream: map_stream(&document.stream),
        cursor: document.cursor,
        next_due_at_epoch_seconds: document.next_due_at_epoch_seconds,
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

fn map_stream(value: &str) -> ProviderPollStream {
    match value {
        "calendar" => ProviderPollStream::Calendar,
        _ => ProviderPollStream::CompletedWorkouts,
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::external_sync::{ExternalProvider, ProviderPollState, ProviderPollStream};

    use super::{map_document_to_poll_state, map_poll_state_to_document};

    #[test]
    fn poll_state_document_round_trip_preserves_fields() {
        let state = ProviderPollState {
            user_id: "user-1".to_string(),
            provider: ExternalProvider::Intervals,
            stream: ProviderPollStream::Calendar,
            cursor: Some("cursor-1".to_string()),
            next_due_at_epoch_seconds: 1_700_000_000,
        };

        let mapped = map_document_to_poll_state(map_poll_state_to_document(&state));

        assert_eq!(mapped, state);
    }
}
