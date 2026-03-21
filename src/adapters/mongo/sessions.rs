use mongodb::{
    bson::{doc, oid::ObjectId, DateTime},
    options::IndexOptions,
    Collection, IndexModel,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::domain::identity::{AuthSession, BoxFuture, IdentityError, SessionRepository};

#[derive(Clone)]
pub struct MongoSessionRepository {
    collection: Collection<SessionDocument>,
}

impl MongoSessionRepository {
    pub fn new(client: mongodb::Client, database: impl AsRef<str>) -> Self {
        Self {
            collection: client
                .database(database.as_ref())
                .collection("auth_sessions"),
        }
    }

    pub async fn ensure_indexes(&self) -> Result<(), IdentityError> {
        self.collection
            .create_indexes([
                IndexModel::builder()
                    .keys(doc! { "session_id": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("auth_sessions_session_id_unique".to_string())
                            .unique(true)
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "expires_at": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("auth_sessions_expires_at_ttl".to_string())
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

impl SessionRepository for MongoSessionRepository {
    fn find_by_id(
        &self,
        session_id: &str,
    ) -> BoxFuture<Result<Option<AuthSession>, IdentityError>> {
        let collection = self.collection.clone();
        let session_id = session_id.to_string();
        Box::pin(async move {
            let document = collection
                .find_one(doc! { "session_id": &session_id })
                .await
                .map_err(|error| IdentityError::Repository(error.to_string()))?;

            Ok(document.map(map_session_document))
        })
    }

    fn save(&self, session: AuthSession) -> BoxFuture<Result<AuthSession, IdentityError>> {
        let collection = self.collection.clone();
        let document = SessionDocument::from_session(&session);
        Box::pin(async move {
            collection
                .replace_one(doc! { "session_id": &document.session_id }, &document)
                .upsert(true)
                .await
                .map_err(|error| IdentityError::Repository(error.to_string()))?;

            Ok(session)
        })
    }

    fn delete(&self, session_id: &str) -> BoxFuture<Result<(), IdentityError>> {
        let collection = self.collection.clone();
        let session_id = session_id.to_string();
        Box::pin(async move {
            collection
                .delete_one(doc! { "session_id": &session_id })
                .await
                .map_err(|error| IdentityError::Repository(error.to_string()))?;

            Ok(())
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct SessionDocument {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    id: Option<ObjectId>,
    session_id: String,
    user_id: String,
    expires_at_epoch_seconds: i64,
    created_at_epoch_seconds: i64,
    expires_at: DateTime,
}

impl SessionDocument {
    fn from_session(session: &AuthSession) -> Self {
        Self {
            id: None,
            session_id: session.id.clone(),
            user_id: session.user_id.clone(),
            expires_at_epoch_seconds: session.expires_at_epoch_seconds,
            created_at_epoch_seconds: session.created_at_epoch_seconds,
            expires_at: DateTime::from_millis(session.expires_at_epoch_seconds * 1000),
        }
    }
}

fn map_session_document(document: SessionDocument) -> AuthSession {
    AuthSession::new(
        document.session_id,
        document.user_id,
        document.expires_at_epoch_seconds,
        document.created_at_epoch_seconds,
    )
}
