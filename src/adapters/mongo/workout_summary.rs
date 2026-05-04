use futures::TryStreamExt;
use mongodb::{
    bson::{doc, oid::ObjectId, Bson, DateTime},
    options::IndexOptions,
    Collection, IndexModel,
};
use serde::{Deserialize, Serialize};

use super::time::{
    optional_epoch_seconds_to_bson_datetime, resolve_optional_epoch_seconds,
    resolve_required_epoch_seconds,
};
use crate::{
    adapters::mongo::error::is_duplicate_key_error,
    domain::{
        completed_workouts::{canonical_completed_workout_id, completed_workout_activity_id},
        llm::LlmChatMessage,
        workout_summary::{
            BoxFuture, ConversationMessage, MessageRole, PublicToolCall, WorkoutRecap,
            WorkoutSummary, WorkoutSummaryError, WorkoutSummaryRepository,
        },
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
    #[serde(default)]
    hidden_transcript: Vec<LlmChatMessage>,
    saved_at_epoch_seconds: Option<i64>,
    #[serde(default)]
    saved_at: Option<DateTime>,
    #[serde(default)]
    workout_recap_text: Option<String>,
    #[serde(default)]
    workout_recap_provider: Option<String>,
    #[serde(default)]
    workout_recap_model: Option<String>,
    #[serde(default)]
    workout_recap_generated_at_epoch_seconds: Option<i64>,
    #[serde(default)]
    workout_recap_generated_at: Option<DateTime>,
    created_at_epoch_seconds: Option<i64>,
    #[serde(default)]
    created_at: Option<DateTime>,
    updated_at_epoch_seconds: Option<i64>,
    #[serde(default)]
    updated_at: Option<DateTime>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ConversationMessageDocument {
    id: String,
    role: String,
    content: String,
    #[serde(default)]
    tool_call: Option<PublicToolCall>,
    created_at_epoch_seconds: Option<i64>,
    #[serde(default)]
    created_at: Option<DateTime>,
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
            let preferred_documents = find_preferred_documents(&collection, &user_id, &workout_ids)
                .await?
                .into_iter()
                .map(map_document_to_domain)
                .collect::<Result<Vec<_>, _>>()?;
            Ok(preferred_documents)
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
            if document_is_locked(&document) {
                return Err(WorkoutSummaryError::Locked);
            }

            let result = collection
                .update_one(
                    editable_document_identity_filter(&document),
                    doc! {
                        "$set": {
                            "rpe": i32::from(rpe),
                            "updated_at_epoch_seconds": updated_at_epoch_seconds,
                            "updated_at": optional_epoch_seconds_to_bson_datetime(Some(updated_at_epoch_seconds), "updated_at")
                                .map_err(WorkoutSummaryError::Repository)?,
                        }
                    },
                )
                .await
                .map_err(|error| WorkoutSummaryError::Repository(error.to_string()))?;

            if result.matched_count == 0 {
                let existing = find_preferred_document(&collection, &user_id, &workout_id).await?;

                return match existing {
                    Some(document) if document_is_locked(&document) => {
                        Err(WorkoutSummaryError::Locked)
                    }
                    Some(_) => Err(WorkoutSummaryError::NotFound),
                    None => Err(WorkoutSummaryError::NotFound),
                };
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
            if document_is_locked(&document) && message.role == "user" {
                return Err(WorkoutSummaryError::Locked);
            }

            let result = collection
                .update_one(
                    with_message_append_filter(document_identity_filter(&document), &message.id),
                    doc! {
                        "$push": { "messages": mongodb::bson::to_bson(&message).map_err(|error| WorkoutSummaryError::Repository(error.to_string()))? },
                        "$set": {
                            "updated_at_epoch_seconds": updated_at_epoch_seconds,
                            "updated_at": optional_epoch_seconds_to_bson_datetime(Some(updated_at_epoch_seconds), "updated_at")
                                .map_err(WorkoutSummaryError::Repository)?,
                        },
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
                    Some(document) if document_is_locked(&document) && message.role == "user" => {
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
                            "saved_at": optional_epoch_seconds_to_bson_datetime(saved_at_epoch_seconds, "saved_at")
                                .map_err(WorkoutSummaryError::Repository)?,
                            "updated_at_epoch_seconds": updated_at_epoch_seconds,
                            "updated_at": optional_epoch_seconds_to_bson_datetime(Some(updated_at_epoch_seconds), "updated_at")
                                .map_err(WorkoutSummaryError::Repository)?,
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

    fn replace_hidden_transcript(
        &self,
        user_id: &str,
        workout_id: &str,
        hidden_transcript: Vec<LlmChatMessage>,
        expected_updated_at_epoch_seconds: i64,
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
                    doc! {
                        "$and": [
                            document_identity_filter(&document),
                            { "user_id": &user_id },
                            { "updated_at_epoch_seconds": expected_updated_at_epoch_seconds },
                        ]
                    },
                    doc! {
                        "$set": {
                            "hidden_transcript": mongodb::bson::to_bson(&hidden_transcript)
                                .map_err(|error| WorkoutSummaryError::Repository(error.to_string()))?,
                            "updated_at_epoch_seconds": updated_at_epoch_seconds,
                            "updated_at": optional_epoch_seconds_to_bson_datetime(Some(updated_at_epoch_seconds), "updated_at")
                                .map_err(WorkoutSummaryError::Repository)?,
                        }
                    },
                )
                .await
                .map_err(|error| WorkoutSummaryError::Repository(error.to_string()))?;

            if result.matched_count == 0 {
                return Err(WorkoutSummaryError::Repository(
                    "hidden transcript update lost compare-and-set race".to_string(),
                ));
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
                            "workout_recap_generated_at": optional_epoch_seconds_to_bson_datetime(
                                Some(recap.generated_at_epoch_seconds),
                                "workout_recap_generated_at",
                            )
                            .map_err(WorkoutSummaryError::Repository)?,
                            "updated_at_epoch_seconds": updated_at_epoch_seconds,
                            "updated_at": optional_epoch_seconds_to_bson_datetime(Some(updated_at_epoch_seconds), "updated_at")
                                .map_err(WorkoutSummaryError::Repository)?,
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

async fn find_preferred_documents(
    collection: &Collection<WorkoutSummaryDocument>,
    user_id: &str,
    workout_ids: &[String],
) -> Result<Vec<WorkoutSummaryDocument>, WorkoutSummaryError> {
    if workout_ids.is_empty() {
        return Ok(Vec::new());
    }

    let current_lookup_ids = workout_ids
        .iter()
        .flat_map(|workout_id| current_lookup_ids_for_request(workout_id))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    let current_documents = collection
        .find(doc! {
            "user_id": user_id,
            "workout_id": { "$in": current_lookup_ids },
        })
        .await
        .map_err(|error| WorkoutSummaryError::Repository(error.to_string()))?
        .try_collect::<Vec<_>>()
        .await
        .map_err(|error| WorkoutSummaryError::Repository(error.to_string()))?;

    let mut preferred_by_storage_workout_id = current_documents
        .into_iter()
        .map(|document| (document.workout_id.clone(), document))
        .collect::<std::collections::BTreeMap<_, _>>();

    // Migration semantics: current documents keyed by `workout_id` win first.
    // Legacy documents are fetched by `event_id` only for missing workout ids,
    // and `or_insert` keeps the current `workout_id` match preferred when both exist.

    let stored_ids = preferred_by_storage_workout_id
        .keys()
        .cloned()
        .collect::<std::collections::HashSet<_>>();
    let stored_activity_ids = stored_ids
        .iter()
        .map(|id| completed_workout_activity_id(id))
        .collect::<std::collections::HashSet<_>>();

    let missing_workout_ids = workout_ids
        .iter()
        .filter(|workout_id| {
            let activity_id = completed_workout_activity_id(workout_id);
            !stored_ids.contains(workout_id.as_str())
                && !stored_ids.contains(&canonical_completed_workout_id(workout_id))
                && !stored_activity_ids.contains(activity_id)
        })
        .cloned()
        .collect::<Vec<_>>();

    if !missing_workout_ids.is_empty() {
        let legacy_documents = collection
            .find(doc! {
                "user_id": user_id,
                "event_id": { "$in": &missing_workout_ids },
            })
            .await
            .map_err(|error| WorkoutSummaryError::Repository(error.to_string()))?
            .try_collect::<Vec<_>>()
            .await
            .map_err(|error| WorkoutSummaryError::Repository(error.to_string()))?;

        for document in legacy_documents {
            preferred_by_storage_workout_id
                .entry(document.workout_id.clone())
                .or_insert(document);
        }
    }

    let mut preferred_by_requested_workout_id = std::collections::BTreeMap::new();

    for workout_id in workout_ids {
        if let Some(mut document) = preferred_by_storage_workout_id.remove(workout_id) {
            document.workout_id = workout_id.clone();
            preferred_by_requested_workout_id.insert(workout_id.clone(), document);
            continue;
        }

        let document = current_lookup_ids_for_request(workout_id)
            .into_iter()
            .find_map(|candidate| preferred_by_storage_workout_id.remove(&candidate));
        if let Some(mut document) = document {
            document.workout_id = workout_id.clone();
            preferred_by_requested_workout_id.insert(workout_id.clone(), document);
        }
    }

    Ok(workout_ids
        .iter()
        .filter_map(|workout_id| preferred_by_requested_workout_id.remove(workout_id))
        .collect())
}

fn current_lookup_ids_for_request(requested_workout_id: &str) -> Vec<String> {
    let mut lookup_ids = Vec::new();
    let activity_id = completed_workout_activity_id(requested_workout_id);

    push_unique_lookup_id(&mut lookup_ids, requested_workout_id.to_string());
    push_unique_lookup_id(&mut lookup_ids, activity_id.to_string());
    push_unique_lookup_id(
        &mut lookup_ids,
        canonical_completed_workout_id(requested_workout_id),
    );
    push_unique_lookup_id(&mut lookup_ids, format!("wahoo-workout:{activity_id}"));
    push_unique_lookup_id(&mut lookup_ids, format!("intervals-activity:{activity_id}"));

    lookup_ids
}

fn push_unique_lookup_id(lookup_ids: &mut Vec<String>, workout_id: String) {
    if !lookup_ids.contains(&workout_id) {
        lookup_ids.push(workout_id);
    }
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

fn editable_document_identity_filter(document: &WorkoutSummaryDocument) -> mongodb::bson::Document {
    let mut filter = document_identity_filter(document);
    filter.insert("saved_at_epoch_seconds", Bson::Null);
    filter.insert("saved_at", Bson::Null);
    filter
}

fn document_is_locked(document: &WorkoutSummaryDocument) -> bool {
    document.saved_at.is_some() || document.saved_at_epoch_seconds.is_some()
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
    filter.insert("saved_at", Bson::Null);
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
        hidden_transcript: document.hidden_transcript,
        saved_at_epoch_seconds: resolve_optional_epoch_seconds(
            document.saved_at,
            document.saved_at_epoch_seconds,
        ),
        workout_recap_text: document.workout_recap_text,
        workout_recap_provider: document.workout_recap_provider,
        workout_recap_model: document.workout_recap_model,
        workout_recap_generated_at_epoch_seconds: resolve_optional_epoch_seconds(
            document.workout_recap_generated_at,
            document.workout_recap_generated_at_epoch_seconds,
        ),
        created_at_epoch_seconds: resolve_required_epoch_seconds(
            document.created_at,
            document.created_at_epoch_seconds,
            "created_at",
        )
        .map_err(WorkoutSummaryError::Repository)?,
        updated_at_epoch_seconds: resolve_required_epoch_seconds(
            document.updated_at,
            document.updated_at_epoch_seconds,
            "updated_at",
        )
        .map_err(WorkoutSummaryError::Repository)?,
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
        hidden_transcript: summary.hidden_transcript.clone(),
        saved_at_epoch_seconds: summary.saved_at_epoch_seconds,
        saved_at: optional_epoch_seconds_to_bson_datetime(
            summary.saved_at_epoch_seconds,
            "saved_at",
        )
        .expect("saved_at should fit BSON DateTime"),
        workout_recap_text: summary.workout_recap_text.clone(),
        workout_recap_provider: summary.workout_recap_provider.clone(),
        workout_recap_model: summary.workout_recap_model.clone(),
        workout_recap_generated_at_epoch_seconds: summary.workout_recap_generated_at_epoch_seconds,
        workout_recap_generated_at: optional_epoch_seconds_to_bson_datetime(
            summary.workout_recap_generated_at_epoch_seconds,
            "workout_recap_generated_at",
        )
        .expect("workout_recap_generated_at should fit BSON DateTime"),
        created_at_epoch_seconds: Some(summary.created_at_epoch_seconds),
        created_at: optional_epoch_seconds_to_bson_datetime(
            Some(summary.created_at_epoch_seconds),
            "created_at",
        )
        .expect("created_at should fit BSON DateTime"),
        updated_at_epoch_seconds: Some(summary.updated_at_epoch_seconds),
        updated_at: optional_epoch_seconds_to_bson_datetime(
            Some(summary.updated_at_epoch_seconds),
            "updated_at",
        )
        .expect("updated_at should fit BSON DateTime"),
    }
}

fn map_message_to_document(message: ConversationMessage) -> ConversationMessageDocument {
    ConversationMessageDocument {
        id: message.id,
        role: match message.role {
            MessageRole::User => "user".to_string(),
            MessageRole::Coach => "coach".to_string(),
            MessageRole::Tool => "tool".to_string(),
        },
        content: message.content,
        tool_call: message.tool_call,
        created_at_epoch_seconds: Some(message.created_at_epoch_seconds),
        created_at: optional_epoch_seconds_to_bson_datetime(
            Some(message.created_at_epoch_seconds),
            "created_at",
        )
        .expect("created_at should fit BSON DateTime"),
    }
}

fn map_message_to_domain(
    message: ConversationMessageDocument,
) -> Result<ConversationMessage, WorkoutSummaryError> {
    let role = match message.role.as_str() {
        "user" => MessageRole::User,
        "coach" => MessageRole::Coach,
        "tool" => MessageRole::Tool,
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
        tool_call: message.tool_call,
        created_at_epoch_seconds: resolve_required_epoch_seconds(
            message.created_at,
            message.created_at_epoch_seconds,
            "created_at",
        )
        .map_err(WorkoutSummaryError::Repository)?,
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
