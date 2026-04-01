use futures::TryStreamExt;
use mongodb::{
    bson::{doc, oid::ObjectId},
    options::IndexOptions,
    Collection, IndexModel,
};
use serde::{Deserialize, Serialize};

use crate::{
    adapters::mongo::error::is_duplicate_key_error,
    domain::workout_summary::{
        BoxFuture, ConversationMessage, MessageRole, WorkoutSummary, WorkoutSummaryError,
        WorkoutSummaryRepository,
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
            .create_indexes([IndexModel::builder()
                .keys(doc! { "user_id": 1, "workout_id": 1 })
                .options(
                    IndexOptions::builder()
                        .name("workout_summaries_user_workout_unique".to_string())
                        .unique(true)
                        .build(),
                )
                .build()])
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
            let document = collection
                .find_one(doc! { "user_id": &user_id, "workout_id": &workout_id })
                .await
                .map_err(|error| WorkoutSummaryError::Repository(error.to_string()))?;
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
            let documents = collection
                .find(doc! { "user_id": &user_id, "workout_id": { "$in": workout_ids } })
                .await
                .map_err(|error| WorkoutSummaryError::Repository(error.to_string()))?;
            let documents = documents
                .try_collect::<Vec<_>>()
                .await
                .map_err(|error| WorkoutSummaryError::Repository(error.to_string()))?;

            documents
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
            let result = collection
                .update_one(
                    doc! {
                        "user_id": &user_id,
                        "workout_id": &workout_id,
                        "saved_at_epoch_seconds": null,
                    },
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
                let existing = collection
                    .find_one(doc! { "user_id": &user_id, "workout_id": &workout_id })
                    .await
                    .map_err(|error| WorkoutSummaryError::Repository(error.to_string()))?;

                return match existing {
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
            let result = collection
                .update_one(
                    doc! {
                        "user_id": &user_id,
                        "workout_id": &workout_id,
                        "saved_at_epoch_seconds": null,
                    },
                    doc! {
                        "$push": { "messages": mongodb::bson::to_bson(&message).map_err(|error| WorkoutSummaryError::Repository(error.to_string()))? },
                        "$set": { "updated_at_epoch_seconds": updated_at_epoch_seconds },
                    },
                )
                .await
                .map_err(|error| WorkoutSummaryError::Repository(error.to_string()))?;

            if result.matched_count == 0 {
                let existing = collection
                    .find_one(doc! { "user_id": &user_id, "workout_id": &workout_id })
                    .await
                    .map_err(|error| WorkoutSummaryError::Repository(error.to_string()))?;

                return match existing {
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
            let result = collection
                .update_one(
                    doc! { "user_id": &user_id, "workout_id": &workout_id },
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

    fn find_message_by_id(
        &self,
        user_id: &str,
        workout_id: &str,
        message_id: &str,
    ) -> BoxFuture<Result<Option<ConversationMessage>, WorkoutSummaryError>> {
        let collection = self.collection.clone();
        let user_id = user_id.to_string();
        let workout_id = workout_id.to_string();
        let message_id = message_id.to_string();
        Box::pin(async move {
            let document = collection
                .find_one(doc! { "user_id": &user_id, "workout_id": &workout_id })
                .await
                .map_err(|error| WorkoutSummaryError::Repository(error.to_string()))?;

            let summary = document.map(map_document_to_domain).transpose()?;
            Ok(summary.and_then(|summary| {
                summary
                    .messages
                    .into_iter()
                    .find(|message| message.id == message_id)
            }))
        })
    }
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
mod tests {
    use mongodb::bson::{doc, from_document, Bson};

    use super::{map_document_to_domain, ConversationMessageDocument, WorkoutSummaryDocument};
    use crate::domain::workout_summary::WorkoutSummaryError;

    #[test]
    fn map_document_to_domain_rejects_out_of_range_rpe() {
        let error = map_document_to_domain(WorkoutSummaryDocument {
            id: None,
            summary_id: "summary-1".to_string(),
            user_id: "user-1".to_string(),
            workout_id: "workout-1".to_string(),
            rpe: Some(300),
            messages: Vec::<ConversationMessageDocument>::new(),
            saved_at_epoch_seconds: None,
            created_at_epoch_seconds: 1,
            updated_at_epoch_seconds: 1,
        })
        .expect_err("out-of-range rpe should fail");

        assert_eq!(
            error,
            WorkoutSummaryError::Repository("invalid workout summary rpe: 300".to_string())
        );
    }

    #[test]
    fn workout_summary_document_accepts_legacy_event_id_field() {
        let document: WorkoutSummaryDocument = from_document(doc! {
            "summary_id": "summary-1",
            "user_id": "user-1",
            "event_id": "workout-legacy",
            "rpe": 6,
            "messages": [],
            "saved_at_epoch_seconds": Bson::Null,
            "created_at_epoch_seconds": 1,
            "updated_at_epoch_seconds": 1,
        })
        .expect("legacy event_id should deserialize");

        assert_eq!(document.workout_id, "workout-legacy");
    }
}
