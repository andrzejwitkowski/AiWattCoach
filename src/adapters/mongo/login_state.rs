use mongodb::{
    bson::{doc, oid::ObjectId},
    Collection,
};
use serde::{Deserialize, Serialize};

use crate::domain::identity::{BoxFuture, IdentityError, LoginState, LoginStateRepository};

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
}

impl LoginStateRepository for MongoLoginStateRepository {
    fn create(&self, login_state: LoginState) -> BoxFuture<Result<LoginState, IdentityError>> {
        let collection = self.collection.clone();
        let document = LoginStateDocument::from_login_state(&login_state);
        Box::pin(async move {
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
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct LoginStateDocument {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    id: Option<ObjectId>,
    state_id: String,
    return_to: Option<String>,
    expires_at_epoch_seconds: i64,
    created_at_epoch_seconds: i64,
}

impl LoginStateDocument {
    fn from_login_state(login_state: &LoginState) -> Self {
        Self {
            id: None,
            state_id: login_state.id.clone(),
            return_to: login_state.return_to.clone(),
            expires_at_epoch_seconds: login_state.expires_at_epoch_seconds,
            created_at_epoch_seconds: login_state.created_at_epoch_seconds,
        }
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
