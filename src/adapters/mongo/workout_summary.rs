use futures::TryStreamExt;
use mongodb::{
    bson::{doc, oid::ObjectId, Bson},
    options::IndexOptions,
    Collection, IndexModel,
};
use serde::{Deserialize, Serialize};

use crate::{
    adapters::mongo::error::is_duplicate_key_error,
    domain::workout_summary::{
        BoxFuture, ConversationMessage, MessageRole, WorkoutRecap, WorkoutSummary,
        WorkoutSummaryError, WorkoutSummaryRepository,
    },
};

#[derive(Clone)]
pub struct MongoWorkoutSummaryRepository {
    collection: Collection<WorkoutSummaryDocument>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct WorkoutSummaryDocument {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    id: Option<ObjectId>,
    summary_id: String,
    user_id: String,
    #[serde(alias = "event_id")]
    workout_id: String,
    rpe: Option<i32>,
    messages: Vec<ConversationMessageDocument>,
    saved_at_epoch_seconds: Option<i64>,
    #[serde(default)]
    workout_recap_text: Option<String>,
    #[serde(default)]
    workout_recap_provider: Option<String>,
    #[serde(default)]
    workout_recap_model: Option<String>,
    #[serde(default)]
    workout_recap_generated_at_epoch_seconds: Option<i64>,
    created_at_epoch_seconds: i64,
    updated_at_epoch_seconds: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ConversationMessageDocument {
    id: String,
    role: String,
    content: String,
    created_at_epoch_seconds: i64,
}

#[derive(Clone, Debug, Deserialize)]
struct WorkoutSummaryMessageLookupDocument {
    #[serde(default)]
    messages: Vec<ConversationMessageDocument>,
}

impl MongoWorkoutSummaryRepository {
    pub fn new(client: mongodb::Client, database: impl AsRef<str>) -> Self {
        Self {
            collection: client
                .database(database.as_ref())
                .collection("workout_summaries"),
        }
    }

    pub async fn ensure_indexes(&self) -> Result<(), WorkoutSummaryError> {
        self.collection
            .create_indexes([
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "workout_id": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("workout_summaries_user_workout_unique".to_string())
                            .unique(true)
                            .build(),
                    )
                    .build(),
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "event_id": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("workout_summaries_user_event".to_string())
                            .build(),
                    )
                    .build(),
            ])
            .await
            .map_err(|error| WorkoutSummaryError::Repository(error.to_string()))?;
        Ok(())
    }
}

impl WorkoutSummaryRepository for MongoWorkoutSummaryRepository {
    fn find_by_user_id_and_workout_id(
        &self,
        user_id: &str,
        workout_id: &str,
    ) -> BoxFuture<Result<Option<WorkoutSummary>, WorkoutSummaryError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        Box::pin(async move {
            let document = find_preferred_document(&collection, &user_id, &workout_id).await?;
            document.map(map_document_to_domain).transpose()
        })
    }

    fn find_by_user_id_and_workout_ids(
        &self,
        user_id: &str,
        workout_ids: Vec<String>,
    ) -> BoxFuture<Result<Vec<WorkoutSummary>, WorkoutSummaryError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        Box::pin(async move {
            workout_ids
                .into_iter()
                .map(|workout_id| {
                    let collection = collection.clone();
                    let user_id = user_id.clone();
                    async move { find_preferred_document(&collection, &user_id, &workout_id).await }
                })
                .collect::<futures::stream::FuturesOrdered<_>>()
                .try_filter_map(|document| async move { Ok(document) })
                .try_collect::<Vec<_>>()
                .await?
                .into_iter()
                .map(map_document_to_domain)
                .collect::<Result<Vec<_>, _>>()
        })
    }

    fn create(
        &self,
        summary: WorkoutSummary,
    ) -> BoxFuture<Result<WorkoutSummary, WorkoutSummaryError>> {
        let collection = self.collection.clone();
        let document = map_domain_to_document(&summary);
        Box::pin(async move {
            match collection.insert_one(document).await {
                Ok(_) => Ok(summary),
                Err(error) if is_duplicate_key_error(&error) => {
                    Err(WorkoutSummaryError::AlreadyExists)
                }
                Err(error) => Err(WorkoutSummaryError::Repository(error.to_string())),
            }
        })
    }

    fn update_rpe(
        &self,
        user_id: &str,
        workout_id: &str,
        rpe: u8,
        updated_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<(), WorkoutSummaryError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        Box::pin(async move {
            let Some(document) =
                find_preferred_document(&collection, &user_id, &workout_id).await?
            else {
                return Err(WorkoutSummaryError::NotFound);
            };
            if document.saved_at_epoch_seconds.is_some() {
                return Err(WorkoutSummaryError::Locked);
            }

            let result = collection
                .update_one(
                    document_identity_filter(&document),
                    doc! {
                        "$set": {
                            "rpe": i32::from(rpe),
                            "updated_at_epoch_seconds": updated_at_epoch_seconds,
                        }
                    },
                )
                .await
                .map_err(|error| WorkoutSummaryError::Repository(error.to_string()))?;

            if result.matched_count == 0 {
                return Err(WorkoutSummaryError::NotFound);
            }

            Ok(())
        })
    }

    fn append_message(
        &self,
        user_id: &str,
        workout_id: &str,
        message: ConversationMessage,
        updated_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<(), WorkoutSummaryError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        let message = map_message_to_document(message);
        Box::pin(async move {
            let Some(document) =
                find_preferred_document(&collection, &user_id, &workout_id).await?
            else {
                return Err(WorkoutSummaryError::NotFound);
            };
            if document
                .messages
                .iter()
                .any(|existing_message| existing_message.id == message.id)
            {
                return Ok(());
            }
            if document.saved_at_epoch_seconds.is_some() {
                return Err(WorkoutSummaryError::Locked);
            }

            let result = collection
                .update_one(
                    with_message_append_filter(document_identity_filter(&document), &message.id),
                    doc! {
                        "$push": { "messages": mongodb::bson::to_bson(&message).map_err(|error| WorkoutSummaryError::Repository(error.to_string()))? },
                        "$set": { "updated_at_epoch_seconds": updated_at_epoch_seconds },
                    },
                )
                .await
                .map_err(|error| WorkoutSummaryError::Repository(error.to_string()))?;

            if result.matched_count == 0 {
                let existing = find_preferred_document(&collection, &user_id, &workout_id).await?;

                return match existing {
                    Some(document)
                        if document
                            .messages
                            .iter()
                            .any(|existing_message| existing_message.id == message.id) =>
                    {
                        Ok(())
                    }
                    Some(document) if document.saved_at_epoch_seconds.is_some() => {
                        Err(WorkoutSummaryError::Locked)
                    }
                    Some(_) => Err(WorkoutSummaryError::NotFound),
                    None => Err(WorkoutSummaryError::NotFound),
                };
            }

            Ok(())
        })
    }

    fn set_saved_state(
        &self,
        user_id: &str,
        workout_id: &str,
        saved_at_epoch_seconds: Option<i64>,
        updated_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<(), WorkoutSummaryError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        Box::pin(async move {
            let Some(document) =
                find_preferred_document(&collection, &user_id, &workout_id).await?
            else {
                return Err(WorkoutSummaryError::NotFound);
            };

            let result = collection
                .update_one(
                    document_identity_filter(&document),
                    doc! {
                        "$set": {
                            "saved_at_epoch_seconds": saved_at_epoch_seconds,
                            "updated_at_epoch_seconds": updated_at_epoch_seconds,
                        }
                    },
                )
                .await
                .map_err(|error| WorkoutSummaryError::Repository(error.to_string()))?;

            if result.matched_count == 0 {
                return Err(WorkoutSummaryError::NotFound);
            }

            Ok(())
        })
    }

    fn persist_workout_recap(
        &self,
        user_id: &str,
        workout_id: &str,
        recap: WorkoutRecap,
        updated_at_epoch_seconds: i64,
    ) -> BoxFuture<Result<(), WorkoutSummaryError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        Box::pin(async move {
            let Some(document) =
                find_preferred_document(&collection, &user_id, &workout_id).await?
            else {
                return Err(WorkoutSummaryError::NotFound);
            };

            let result = collection
                .update_one(
                    document_identity_filter(&document),
                    doc! {
                        "$set": {
                            "workout_recap_text": recap.text,
                            "workout_recap_provider": recap.provider,
                            "workout_recap_model": recap.model,
                            "workout_recap_generated_at_epoch_seconds": recap.generated_at_epoch_seconds,
                            "updated_at_epoch_seconds": updated_at_epoch_seconds,
                        }
                    },
                )
                .await
                .map_err(|error| WorkoutSummaryError::Repository(error.to_string()))?;

            if result.matched_count == 0 {
                return Err(WorkoutSummaryError::NotFound);
            }

            Ok(())
        })
    }

    fn find_message_by_id(
        &self,
        user_id: &str,
        workout_id: &str,
        message_id: &str,
    ) -> BoxFuture<Result<Option<ConversationMessage>, WorkoutSummaryError>> {
        let collection = self
            .collection
            .clone_with_type::<WorkoutSummaryMessageLookupDocument>();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        let message_id = message_id.to_string();
        Box::pin(async move {
            let document = find_preferred_message_lookup_document(
                &collection,
                &user_id,
                &workout_id,
                &message_id,
            )
            .await?;

            let message = document
                .and_then(|document| document.messages.into_iter().next())
                .map(map_message_to_domain)
                .transpose()?;

            Ok(message)
        })
    }
}

async fn find_preferred_document(
    collection: &Collection<WorkoutSummaryDocument>,
    user_id: &str,
    workout_id: &str,
) -> Result<Option<WorkoutSummaryDocument>, WorkoutSummaryError> {
    if let Some(document) = collection
        .find_one(current_workout_id_filter(user_id, workout_id))
        .await
        .map_err(|error| WorkoutSummaryError::Repository(error.to_string()))?
    {
        return Ok(Some(document));
    }

    collection
        .find_one(legacy_event_id_filter(user_id, workout_id))
        .await
        .map_err(|error| WorkoutSummaryError::Repository(error.to_string()))
}

async fn find_preferred_message_lookup_document(
    collection: &Collection<WorkoutSummaryMessageLookupDocument>,
    user_id: &str,
    workout_id: &str,
    message_id: &str,
) -> Result<Option<WorkoutSummaryMessageLookupDocument>, WorkoutSummaryError> {
    let projection = doc! {
        "messages": { "$elemMatch": { "id": message_id } },
        "_id": 0,
    };

    if let Some(document) = collection
        .find_one(current_workout_id_filter(user_id, workout_id))
        .projection(projection.clone())
        .await
        .map_err(|error| WorkoutSummaryError::Repository(error.to_string()))?
    {
        return Ok(Some(document));
    }

    collection
        .find_one(legacy_event_id_filter(user_id, workout_id))
        .projection(projection)
        .await
        .map_err(|error| WorkoutSummaryError::Repository(error.to_string()))
}

fn document_identity_filter(document: &WorkoutSummaryDocument) -> mongodb::bson::Document {
    match document.id {
        Some(id) => doc! { "_id": id },
        None => doc! {
            "summary_id": &document.summary_id,
            "user_id": &document.user_id,
        },
    }
}

fn current_workout_id_filter(user_id: &str, workout_id: &str) -> mongodb::bson::Document {
    doc! {
        "user_id": user_id,
        "workout_id": workout_id,
    }
}

fn legacy_event_id_filter(user_id: &str, workout_id: &str) -> mongodb::bson::Document {
    doc! {
        "user_id": user_id,
        "event_id": workout_id,
    }
}

fn with_message_append_filter(
    mut filter: mongodb::bson::Document,
    message_id: &str,
) -> mongodb::bson::Document {
    filter.insert("saved_at_epoch_seconds", Bson::Null);
    filter.insert("messages.id", doc! { "$ne": message_id });
    filter
}

fn map_document_to_domain(
    document: WorkoutSummaryDocument,
) -> Result<WorkoutSummary, WorkoutSummaryError> {
    Ok(WorkoutSummary {
        id: document.summary_id,
        user_id: document.user_id,
        workout_id: document.workout_id,
        rpe: document.rpe.map(map_rpe_to_domain).transpose()?,
        messages: document
            .messages
            .into_iter()
            .map(map_message_to_domain)
            .collect::<Result<Vec<_>, _>>()?,
        saved_at_epoch_seconds: document.saved_at_epoch_seconds,
        workout_recap_text: document.workout_recap_text,
        workout_recap_provider: document.workout_recap_provider,
        workout_recap_model: document.workout_recap_model,
        workout_recap_generated_at_epoch_seconds: document.workout_recap_generated_at_epoch_seconds,
        created_at_epoch_seconds: document.created_at_epoch_seconds,
        updated_at_epoch_seconds: document.updated_at_epoch_seconds,
    })
}

fn map_domain_to_document(summary: &WorkoutSummary) -> WorkoutSummaryDocument {
    WorkoutSummaryDocument {
        id: None,
        summary_id: summary.id.clone(),
        user_id: summary.user_id.clone(),
        workout_id: summary.workout_id.clone(),
        rpe: summary.rpe.map(i32::from),
        messages: summary
            .messages
            .iter()
            .cloned()
            .map(map_message_to_document)
            .collect(),
        saved_at_epoch_seconds: summary.saved_at_epoch_seconds,
        workout_recap_text: summary.workout_recap_text.clone(),
        workout_recap_provider: summary.workout_recap_provider.clone(),
        workout_recap_model: summary.workout_recap_model.clone(),
        workout_recap_generated_at_epoch_seconds: summary.workout_recap_generated_at_epoch_seconds,
        created_at_epoch_seconds: summary.created_at_epoch_seconds,
        updated_at_epoch_seconds: summary.updated_at_epoch_seconds,
    }
}

fn map_message_to_document(message: ConversationMessage) -> ConversationMessageDocument {
    ConversationMessageDocument {
        id: message.id,
        role: match message.role {
            MessageRole::User => "user".to_string(),
            MessageRole::Coach => "coach".to_string(),
        },
        content: message.content,
        created_at_epoch_seconds: message.created_at_epoch_seconds,
    }
}

fn map_message_to_domain(
    message: ConversationMessageDocument,
) -> Result<ConversationMessage, WorkoutSummaryError> {
    let role = match message.role.as_str() {
        "user" => MessageRole::User,
        "coach" => MessageRole::Coach,
        other => {
            return Err(WorkoutSummaryError::Repository(format!(
                "unknown message role: {other}"
            )))
        }
    };

    Ok(ConversationMessage {
        id: message.id,
        role,
        content: message.content,
        created_at_epoch_seconds: message.created_at_epoch_seconds,
    })
}

fn map_rpe_to_domain(value: i32) -> Result<u8, WorkoutSummaryError> {
    u8::try_from(value)
        .ok()
        .filter(|value| (1..=10).contains(value))
        .ok_or_else(|| {
            WorkoutSummaryError::Repository(format!("invalid workout summary rpe: {value}"))
        })
}

#[cfg(test)]
#[path = "workout_summary_tests.rs"]
mod tests;
