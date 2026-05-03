use mongodb::{
    bson::{doc, DateTime},
    options::IndexOptions,
    Collection, IndexModel,
};
use serde::{Deserialize, Serialize};

use super::time::{optional_epoch_seconds_to_bson_datetime, resolve_required_epoch_seconds};
use crate::domain::coach_conversation::{
    BoxFuture, CoachConversation, CoachConversationError, CoachConversationFocus,
    CoachConversationRepository, CoachConversationStatus, CoachConversationSurface,
};
use crate::domain::llm::LlmChatMessage;

#[derive(Clone)]
pub struct MongoCoachConversationRepository {
    collection: Collection<CoachConversationDocument>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct CoachConversationDocument {
    user_id: String,
    conversation_id: String,
    surface: String,
    status: String,
    focus: String,
    #[serde(default)]
    hidden_transcript: Vec<LlmChatMessage>,
    created_at_epoch_seconds: i64,
    #[serde(default)]
    created_at: Option<DateTime>,
    updated_at_epoch_seconds: i64,
    #[serde(default)]
    updated_at: Option<DateTime>,
}

impl MongoCoachConversationRepository {
    pub fn new(client: mongodb::Client, database: impl AsRef<str>) -> Self {
        Self {
            collection: client
                .database(database.as_ref())
                .collection("coach_conversations"),
        }
    }

    pub async fn ensure_indexes(&self) -> Result<(), CoachConversationError> {
        self.collection
            .create_indexes([
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "conversation_id": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("coach_conversations_user_conversation_unique".to_string())
                            .unique(true)
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "surface": 1, "status": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("coach_conversations_user_surface_active_unique".to_string())
                            .unique(true)
                            .partial_filter_expression(doc! {
                                "status": status_as_str(&CoachConversationStatus::Active)
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

impl CoachConversationRepository for MongoCoachConversationRepository {
    fn find_active_by_user_id_and_surface(
        &self,
        user_id: &str,
        surface: &CoachConversationSurface,
    ) -> BoxFuture<Result<Option<CoachConversation>, CoachConversationError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let surface = surface_as_str(surface).to_string();
        Box::pin(async move {
            let document = collection
                .find_one(doc! {
                    "user_id": &user_id,
                    "surface": &surface,
                    "status": status_as_str(&CoachConversationStatus::Active),
                })
                .await
                .map_err(storage_error)?;
            document.map(map_document_to_domain).transpose()
        })
    }

    fn find_by_user_id_and_conversation_id(
        &self,
        user_id: &str,
        conversation_id: &str,
    ) -> BoxFuture<Result<Option<CoachConversation>, CoachConversationError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let conversation_id = conversation_id.to_string();
        Box::pin(async move {
            let document = collection
                .find_one(doc! {
                    "user_id": &user_id,
                    "conversation_id": &conversation_id,
                })
                .await
                .map_err(storage_error)?;
            document.map(map_document_to_domain).transpose()
        })
    }

    fn create(
        &self,
        conversation: CoachConversation,
    ) -> BoxFuture<Result<CoachConversation, CoachConversationError>> {
        let collection = self.collection.clone();
        let document = map_domain_to_document(&conversation);
        Box::pin(async move {
            collection
                .insert_one(&document)
                .await
                .map_err(storage_error)?;
            Ok(conversation)
        })
    }

    fn update_status(
        &self,
        user_id: &str,
        conversation_id: &str,
        status: CoachConversationStatus,
        updated_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<(), CoachConversationError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let conversation_id = conversation_id.to_string();
        Box::pin(async move {
            let result = collection
                .update_one(
                    doc! {
                        "user_id": &user_id,
                        "conversation_id": &conversation_id,
                    },
                    doc! {
                        "$set": {
                            "status": status_as_str(&status),
                            "updated_at_epoch_seconds": updated_at_epoch_seconds,
                            "updated_at": optional_epoch_seconds_to_bson_datetime(
                                Some(updated_at_epoch_seconds),
                                "updated_at",
                            )
                            .map_err(CoachConversationError::Repository)?,
                        }
                    },
                )
                .await
                .map_err(storage_error)?;

            if result.matched_count == 0 {
                return Err(CoachConversationError::NotFound);
            }

            Ok(())
        })
    }

    fn touch_updated_at(
        &self,
        user_id: &str,
        conversation_id: &str,
        updated_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<(), CoachConversationError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let conversation_id = conversation_id.to_string();
        Box::pin(async move {
            let result = collection
                .update_one(
                    doc! {
                        "user_id": &user_id,
                        "conversation_id": &conversation_id,
                    },
                    doc! {
                        "$set": {
                            "updated_at_epoch_seconds": updated_at_epoch_seconds,
                            "updated_at": optional_epoch_seconds_to_bson_datetime(
                                Some(updated_at_epoch_seconds),
                                "updated_at",
                            )
                            .map_err(CoachConversationError::Repository)?,
                        }
                    },
                )
                .await
                .map_err(storage_error)?;

            if result.matched_count == 0 {
                return Err(CoachConversationError::NotFound);
            }

            Ok(())
        })
    }

    fn replace_hidden_transcript(
        &self,
        user_id: &str,
        conversation_id: &str,
        hidden_transcript: Vec<LlmChatMessage>,
        updated_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<(), CoachConversationError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let conversation_id = conversation_id.to_string();
        Box::pin(async move {
            let result = collection
                .update_one(
                    doc! {
                        "user_id": &user_id,
                        "conversation_id": &conversation_id,
                    },
                    doc! {
                        "$set": {
                            "hidden_transcript": mongodb::bson::to_bson(&hidden_transcript)
                                .map_err(|error| CoachConversationError::Repository(error.to_string()))?,
                            "updated_at_epoch_seconds": updated_at_epoch_seconds,
                            "updated_at": optional_epoch_seconds_to_bson_datetime(
                                Some(updated_at_epoch_seconds),
                                "updated_at",
                            )
                            .map_err(CoachConversationError::Repository)?,
                        }
                    },
                )
                .await
                .map_err(storage_error)?;

            if result.matched_count == 0 {
                return Err(CoachConversationError::NotFound);
            }

            Ok(())
        })
    }
}

fn map_domain_to_document(conversation: &CoachConversation) -> CoachConversationDocument {
    CoachConversationDocument {
        user_id: conversation.user_id.clone(),
        conversation_id: conversation.conversation_id.clone(),
        surface: surface_as_str(&conversation.surface).to_string(),
        status: status_as_str(&conversation.status).to_string(),
        focus: focus_as_str(&conversation.focus).to_string(),
        hidden_transcript: conversation.hidden_transcript.clone(),
        created_at_epoch_seconds: conversation.created_at_epoch_seconds,
        created_at: optional_epoch_seconds_to_bson_datetime(
            Some(conversation.created_at_epoch_seconds),
            "created_at",
        )
        .expect("created_at should fit BSON DateTime"),
        updated_at_epoch_seconds: conversation.updated_at_epoch_seconds,
        updated_at: optional_epoch_seconds_to_bson_datetime(
            Some(conversation.updated_at_epoch_seconds),
            "updated_at",
        )
        .expect("updated_at should fit BSON DateTime"),
    }
}

fn map_document_to_domain(
    document: CoachConversationDocument,
) -> Result<CoachConversation, CoachConversationError> {
    Ok(CoachConversation {
        conversation_id: document.conversation_id,
        user_id: document.user_id,
        surface: map_surface(document.surface)?,
        status: map_status(document.status)?,
        focus: map_focus(document.focus)?,
        hidden_transcript: document.hidden_transcript,
        created_at_epoch_seconds: resolve_required_epoch_seconds(
            document.created_at,
            Some(document.created_at_epoch_seconds),
            "created_at",
        )
        .map_err(CoachConversationError::Repository)?,
        updated_at_epoch_seconds: resolve_required_epoch_seconds(
            document.updated_at,
            Some(document.updated_at_epoch_seconds),
            "updated_at",
        )
        .map_err(CoachConversationError::Repository)?,
    })
}

fn surface_as_str(surface: &CoachConversationSurface) -> &'static str {
    match surface {
        CoachConversationSurface::Calendar => "calendar",
    }
}

fn status_as_str(status: &CoachConversationStatus) -> &'static str {
    match status {
        CoachConversationStatus::Active => "active",
        CoachConversationStatus::Archived => "archived",
    }
}

fn focus_as_str(focus: &CoachConversationFocus) -> &'static str {
    match focus {
        CoachConversationFocus::Overview => "overview",
    }
}

fn map_surface(value: String) -> Result<CoachConversationSurface, CoachConversationError> {
    match value.as_str() {
        "calendar" => Ok(CoachConversationSurface::Calendar),
        other => Err(CoachConversationError::Repository(format!(
            "unknown coach conversation surface: {other}"
        ))),
    }
}

fn map_status(value: String) -> Result<CoachConversationStatus, CoachConversationError> {
    match value.as_str() {
        "active" => Ok(CoachConversationStatus::Active),
        "archived" => Ok(CoachConversationStatus::Archived),
        other => Err(CoachConversationError::Repository(format!(
            "unknown coach conversation status: {other}"
        ))),
    }
}

fn map_focus(value: String) -> Result<CoachConversationFocus, CoachConversationError> {
    match value.as_str() {
        "overview" => Ok(CoachConversationFocus::Overview),
        other => Err(CoachConversationError::Repository(format!(
            "unknown coach conversation focus: {other}"
        ))),
    }
}

fn storage_error(error: mongodb::error::Error) -> CoachConversationError {
    CoachConversationError::Repository(error.to_string())
}
