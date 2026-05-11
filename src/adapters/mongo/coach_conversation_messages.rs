use futures::TryStreamExt;
use mongodb::{
    bson::{doc, DateTime},
    options::IndexOptions,
    Collection, IndexModel,
};
use serde::{Deserialize, Serialize};

use super::{
    error::is_duplicate_key_error,
    time::{optional_epoch_seconds_to_bson_datetime, resolve_required_epoch_seconds},
};
use crate::domain::coach_conversation::{
    BoxFuture, CoachConversationError, CoachConversationMessage,
    CoachConversationMessageRepository, CoachConversationMessageRole,
};

#[derive(Clone)]
pub struct MongoCoachConversationMessageRepository {
    collection: Collection<CoachConversationMessageDocument>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct CoachConversationMessageDocument {
    user_id: String,
    conversation_id: String,
    id: String,
    role: String,
    content: String,
    #[serde(default)]
    tool_call: Option<crate::domain::workout_summary::PublicToolCall>,
    #[serde(default)]
    reasoning_content: Option<String>,
    created_at_epoch_seconds: i64,
    #[serde(default)]
    created_at: Option<DateTime>,
}

impl MongoCoachConversationMessageRepository {
    pub fn new(client: mongodb::Client, database: impl AsRef<str>) -> Self {
        Self {
            collection: client
                .database(database.as_ref())
                .collection("coach_conversation_messages"),
        }
    }

    pub async fn ensure_indexes(&self) -> Result<(), CoachConversationError> {
        self.collection
            .create_indexes([
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "conversation_id": 1, "id": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("coach_conversation_messages_user_conversation_id_unique".to_string())
                            .unique(true)
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "conversation_id": 1, "created_at_epoch_seconds": 1, "id": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("coach_conversation_messages_user_conversation_created".to_string())
                            .build(),
                    )
                    .build(),
            ])
            .await
            .map_err(storage_error)?;
        Ok(())
    }
}

impl CoachConversationMessageRepository for MongoCoachConversationMessageRepository {
    fn list_by_user_id_and_conversation_id(
        &self,
        user_id: &str,
        conversation_id: &str,
    ) -> BoxFuture<Result<Vec<CoachConversationMessage>, CoachConversationError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let conversation_id = conversation_id.to_string();
        Box::pin(async move {
            let documents = collection
                .find(doc! {
                    "user_id": &user_id,
                    "conversation_id": &conversation_id,
                })
                .sort(doc! { "created_at_epoch_seconds": 1, "id": 1 })
                .await
                .map_err(storage_error)?
                .try_collect::<Vec<_>>()
                .await
                .map_err(storage_error)?;

            documents
                .into_iter()
                .map(map_document_to_domain)
                .collect::<Result<Vec<_>, _>>()
        })
    }

    fn append(
        &self,
        message: CoachConversationMessage,
    ) -> BoxFuture<Result<CoachConversationMessage, CoachConversationError>> {
        let collection = self.collection.clone();
        let document = map_domain_to_document(&message);
        Box::pin(async move {
            let inserted = collection
                .insert_one(&document)
                .await
                .map(|_| true)
                .or_else(|error| {
                    if is_duplicate_key_error(&error) {
                        Ok(false)
                    } else {
                        Err(storage_error(error))
                    }
                })?;

            if inserted {
                return Ok(message);
            }

            let existing = collection
                .find_one(doc! {
                    "user_id": &document.user_id,
                    "conversation_id": &document.conversation_id,
                    "id": &document.id,
                })
                .await
                .map_err(storage_error)?
                .ok_or_else(|| {
                    CoachConversationError::Repository(
                        "persisted coach conversation message disappeared before reload"
                            .to_string(),
                    )
                })?;

            map_document_to_domain(existing)
        })
    }

    fn find_by_user_id_and_conversation_id_and_message_id(
        &self,
        user_id: &str,
        conversation_id: &str,
        message_id: &str,
    ) -> BoxFuture<Result<Option<CoachConversationMessage>, CoachConversationError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let conversation_id = conversation_id.to_string();
        let message_id = message_id.to_string();
        Box::pin(async move {
            let document = collection
                .find_one(doc! {
                    "user_id": &user_id,
                    "conversation_id": &conversation_id,
                    "id": &message_id,
                })
                .await
                .map_err(storage_error)?;

            document.map(map_document_to_domain).transpose()
        })
    }
}

fn map_domain_to_document(message: &CoachConversationMessage) -> CoachConversationMessageDocument {
    CoachConversationMessageDocument {
        user_id: message.user_id.clone(),
        conversation_id: message.conversation_id.clone(),
        id: message.id.clone(),
        role: role_as_str(&message.role).to_string(),
        content: message.content.clone(),
        tool_call: message.tool_call.clone(),
        reasoning_content: message.reasoning_content.clone(),
        created_at_epoch_seconds: message.created_at_epoch_seconds,
        created_at: optional_epoch_seconds_to_bson_datetime(
            Some(message.created_at_epoch_seconds),
            "created_at",
        )
        .expect("created_at should fit BSON DateTime"),
    }
}

fn map_document_to_domain(
    document: CoachConversationMessageDocument,
) -> Result<CoachConversationMessage, CoachConversationError> {
    Ok(CoachConversationMessage {
        id: document.id,
        conversation_id: document.conversation_id,
        user_id: document.user_id,
        role: map_role(document.role)?,
        content: document.content,
        tool_call: document.tool_call,
        reasoning_content: document.reasoning_content,
        created_at_epoch_seconds: resolve_required_epoch_seconds(
            document.created_at,
            Some(document.created_at_epoch_seconds),
            "created_at",
        )
        .map_err(CoachConversationError::Repository)?,
    })
}

fn role_as_str(role: &CoachConversationMessageRole) -> &'static str {
    match role {
        CoachConversationMessageRole::User => "user",
        CoachConversationMessageRole::Coach => "coach",
        CoachConversationMessageRole::System => "system",
        CoachConversationMessageRole::Tool => "tool",
    }
}

fn map_role(value: String) -> Result<CoachConversationMessageRole, CoachConversationError> {
    match value.as_str() {
        "user" => Ok(CoachConversationMessageRole::User),
        "coach" => Ok(CoachConversationMessageRole::Coach),
        "system" => Ok(CoachConversationMessageRole::System),
        "tool" => Ok(CoachConversationMessageRole::Tool),
        other => Err(CoachConversationError::Repository(format!(
            "unknown coach conversation message role: {other}"
        ))),
    }
}

fn storage_error(error: mongodb::error::Error) -> CoachConversationError {
    CoachConversationError::Repository(error.to_string())
}
