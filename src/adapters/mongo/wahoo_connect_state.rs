use mongodb::{
    bson::{doc, oid::ObjectId, DateTime},
    options::IndexOptions,
    Collection, IndexModel,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::domain::wahoo::{BoxFuture, WahooConnectState, WahooConnectStateRepository, WahooError};

use super::time::epoch_seconds_to_bson_datetime;

#[derive(Clone)]
pub struct MongoWahooConnectStateRepository {
    collection: Collection<WahooConnectStateDocument>,
}

impl MongoWahooConnectStateRepository {
    pub fn new(client: mongodb::Client, database: impl AsRef<str>) -> Self {
        Self {
            collection: client
                .database(database.as_ref())
                .collection("wahoo_connect_states"),
        }
    }

    pub async fn ensure_indexes(&self) -> Result<(), WahooError> {
        self.collection
            .create_indexes([
                IndexModel::builder()
                    .keys(doc! { "state_id": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("wahoo_connect_states_state_id_unique".to_string())
                            .unique(true)
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "expires_at": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("wahoo_connect_states_expires_at_ttl".to_string())
                            .expire_after(Duration::from_secs(0))
                            .build(),
                    )
                    .build(),
            ])
            .await
            .map_err(|error| WahooError::Repository(error.to_string()))?;

        Ok(())
    }
}

impl WahooConnectStateRepository for MongoWahooConnectStateRepository {
    fn create(&self, state: WahooConnectState) -> BoxFuture<Result<WahooConnectState, WahooError>> {
        let collection = self.collection.clone();
        Box::pin(async move {
            collection
                .insert_one(WahooConnectStateDocument::from_state(&state)?)
                .await
                .map_err(|error| WahooError::Repository(error.to_string()))?;
            Ok(state)
        })
    }

    fn consume(
        &self,
        state_id: &str,
        user_id: &str,
    ) -> BoxFuture<Result<Option<WahooConnectState>, WahooError>> {
        let collection = self.collection.clone();
        let state_id = state_id.to_string();
        let user_id = user_id.to_string();
        Box::pin(async move {
            let document = collection
                .find_one_and_delete(doc! { "state_id": state_id, "user_id": user_id })
                .await
                .map_err(|error| WahooError::Repository(error.to_string()))?;
            Ok(document.map(map_document_to_domain))
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct WahooConnectStateDocument {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    id: Option<ObjectId>,
    state_id: String,
    user_id: String,
    return_to: Option<String>,
    expires_at_epoch_seconds: i64,
    created_at_epoch_seconds: i64,
    expires_at: DateTime,
}

impl WahooConnectStateDocument {
    fn from_state(state: &WahooConnectState) -> Result<Self, WahooError> {
        Ok(Self {
            id: None,
            state_id: state.id.clone(),
            user_id: state.user_id.clone(),
            return_to: state.return_to.clone(),
            expires_at_epoch_seconds: state.expires_at_epoch_seconds,
            created_at_epoch_seconds: state.created_at_epoch_seconds,
            expires_at: epoch_seconds_to_bson_datetime(state.expires_at_epoch_seconds)
                .map_err(|error| WahooError::Repository(error.to_string()))?,
        })
    }
}

fn map_document_to_domain(document: WahooConnectStateDocument) -> WahooConnectState {
    WahooConnectState::new(
        document.state_id,
        document.user_id,
        document.return_to,
        document.expires_at_epoch_seconds,
        document.created_at_epoch_seconds,
    )
}
