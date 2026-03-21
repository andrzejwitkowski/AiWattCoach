use mongodb::{
    bson::{doc, oid::ObjectId, DateTime},
    options::IndexOptions,
    Collection, IndexModel,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::domain::identity::{BoxFuture, IdentityError, LoginState, LoginStateRepository};

use super::time::epoch_seconds_to_bson_datetime;

#[derive(Clone)]
pub struct MongoLoginStateRepository {
    collection: Collection<LoginStateDocument>,
}

impl MongoLoginStateRepository {
    pub fn new(client: mongodb::Client, database: impl AsRef<str>) -> Self {
        Self {
            collection: client
                .database(database.as_ref())
                .collection("oauth_login_states"),
        }
    }

    pub async fn ensure_indexes(&self) -> Result<(), IdentityError> {
        self.collection
            .create_indexes([
                IndexModel::builder()
                    .keys(doc! { "state_id": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("oauth_login_states_state_id_unique".to_string())
                            .unique(true)
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "expires_at": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("oauth_login_states_expires_at_ttl".to_string())
                            .expire_after(Duration::from_secs(0))
                            .build(),
                    )
                    .build(),
            ])
            .await
            .map_err(|error| IdentityError::Repository(error.to_string()))?;

        Ok(())
    }
}

impl LoginStateRepository for MongoLoginStateRepository {
    fn create(&self, login_state: LoginState) -> BoxFuture<Result<LoginState, IdentityError>> {
        let collection = self.collection.clone();
        Box::pin(async move {
            let document = LoginStateDocument::from_login_state(&login_state)?;
            collection
                .insert_one(&document)
                .await
                .map_err(|error| IdentityError::Repository(error.to_string()))?;

            Ok(login_state)
        })
    }

    fn find_by_id(&self, state_id: &str) -> BoxFuture<Result<Option<LoginState>, IdentityError>> {
        let collection = self.collection.clone();
        let state_id = state_id.to_string();
        Box::pin(async move {
            let document = collection
                .find_one(doc! { "state_id": &state_id })
                .await
                .map_err(|error| IdentityError::Repository(error.to_string()))?;

            Ok(document.map(map_login_state_document))
        })
    }

    fn delete(&self, state_id: &str) -> BoxFuture<Result<(), IdentityError>> {
        let collection = self.collection.clone();
        let state_id = state_id.to_string();
        Box::pin(async move {
            collection
                .delete_one(doc! { "state_id": &state_id })
                .await
                .map_err(|error| IdentityError::Repository(error.to_string()))?;

            Ok(())
        })
    }

    fn consume(&self, state_id: &str) -> BoxFuture<Result<Option<LoginState>, IdentityError>> {
        let collection = self.collection.clone();
        let state_id = state_id.to_string();
        Box::pin(async move {
            let document = collection
                .find_one_and_delete(doc! { "state_id": &state_id })
                .await
                .map_err(|error| IdentityError::Repository(error.to_string()))?;

            Ok(document.map(map_login_state_document))
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct LoginStateDocument {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    id: Option<ObjectId>,
    state_id: String,
    return_to: Option<String>,
    expires_at_epoch_seconds: i64,
    created_at_epoch_seconds: i64,
    expires_at: DateTime,
}

impl LoginStateDocument {
    fn from_login_state(login_state: &LoginState) -> Result<Self, IdentityError> {
        Ok(Self {
            id: None,
            state_id: login_state.id.clone(),
            return_to: login_state.return_to.clone(),
            expires_at_epoch_seconds: login_state.expires_at_epoch_seconds,
            created_at_epoch_seconds: login_state.created_at_epoch_seconds,
            expires_at: epoch_seconds_to_bson_datetime(login_state.expires_at_epoch_seconds)?,
        })
    }
}

fn map_login_state_document(document: LoginStateDocument) -> LoginState {
    LoginState::new(
        document.state_id,
        document.return_to,
        document.expires_at_epoch_seconds,
        document.created_at_epoch_seconds,
    )
}

#[cfg(test)]
mod tests {
    use crate::domain::identity::{IdentityError, LoginState};

    use super::LoginStateDocument;

    #[test]
    fn rejects_login_state_expiry_that_cannot_be_converted_to_bson_datetime() {
        let login_state = LoginState::new(
            "state-1".to_string(),
            Some("/app".to_string()),
            i64::MAX / 1000 + 1,
            100,
        );

        let error = LoginStateDocument::from_login_state(&login_state).unwrap_err();

        assert!(
            matches!(error, IdentityError::Repository(message) if message.contains("expires_at timestamp exceeds BSON DateTime range"))
        );
    }
}
