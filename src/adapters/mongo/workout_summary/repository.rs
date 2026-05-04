use mongodb::{
    bson::{doc, to_bson, Document},
    Collection,
};

use super::{
    super::{error::is_duplicate_key_error, time::optional_epoch_seconds_to_bson_datetime},
    document::{WorkoutSummaryDocument, WorkoutSummaryMessageLookupDocument},
    lookup::{
        document_identity_filter, document_is_locked, editable_document_identity_filter,
        find_preferred_document, find_preferred_documents, find_preferred_message_lookup_document,
        with_message_append_filter,
    },
    mapping::{
        map_document_to_domain, map_domain_to_document, map_message_to_document,
        map_message_to_domain,
    },
    MongoWorkoutSummaryRepository,
};
use crate::domain::{
    llm::LlmChatMessage,
    workout_summary::{
        BoxFuture, ConversationMessage, WorkoutRecap, WorkoutSummary, WorkoutSummaryError,
        WorkoutSummaryRepository,
    },
};

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
            find_preferred_documents(&collection, &user_id, &workout_ids)
                .await?
                .into_iter()
                .map(map_document_to_domain)
                .collect()
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
            let document = load_document_or_not_found(&collection, &user_id, &workout_id).await?;
            if document_is_locked(&document) {
                return Err(WorkoutSummaryError::Locked);
            }

            let mut set = updated_at_fields(updated_at_epoch_seconds)?;
            set.insert("rpe", i32::from(rpe));

            let result = collection
                .update_one(
                    editable_document_identity_filter(&document),
                    doc! { "$set": set },
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
            let document = load_document_or_not_found(&collection, &user_id, &workout_id).await?;
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

            let set = updated_at_fields(updated_at_epoch_seconds)?;
            let message_bson = to_bson(&message)
                .map_err(|error| WorkoutSummaryError::Repository(error.to_string()))?;

            let result = collection
                .update_one(
                    with_message_append_filter(document_identity_filter(&document), &message.id),
                    doc! {
                        "$push": { "messages": message_bson },
                        "$set": set,
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
            let document = load_document_or_not_found(&collection, &user_id, &workout_id).await?;

            let mut set = updated_at_fields(updated_at_epoch_seconds)?;
            set.insert("saved_at_epoch_seconds", saved_at_epoch_seconds);
            set.insert(
                "saved_at",
                optional_epoch_seconds_to_bson_datetime(saved_at_epoch_seconds, "saved_at")
                    .map_err(WorkoutSummaryError::Repository)?,
            );

            let result = collection
                .update_one(document_identity_filter(&document), doc! { "$set": set })
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
            let document = load_document_or_not_found(&collection, &user_id, &workout_id).await?;

            let mut set = updated_at_fields(updated_at_epoch_seconds)?;
            set.insert(
                "hidden_transcript",
                to_bson(&hidden_transcript)
                    .map_err(|error| WorkoutSummaryError::Repository(error.to_string()))?,
            );

            let result = collection
                .update_one(
                    doc! {
                        "$and": [
                            document_identity_filter(&document),
                            { "user_id": &user_id },
                            { "updated_at_epoch_seconds": expected_updated_at_epoch_seconds },
                        ]
                    },
                    doc! { "$set": set },
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
            let document = load_document_or_not_found(&collection, &user_id, &workout_id).await?;

            let mut set = updated_at_fields(updated_at_epoch_seconds)?;
            set.insert("workout_recap_text", recap.text);
            set.insert("workout_recap_provider", recap.provider);
            set.insert("workout_recap_model", recap.model);
            set.insert(
                "workout_recap_generated_at_epoch_seconds",
                recap.generated_at_epoch_seconds,
            );
            set.insert(
                "workout_recap_generated_at",
                optional_epoch_seconds_to_bson_datetime(
                    Some(recap.generated_at_epoch_seconds),
                    "workout_recap_generated_at",
                )
                .map_err(WorkoutSummaryError::Repository)?,
            );

            let result = collection
                .update_one(document_identity_filter(&document), doc! { "$set": set })
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

async fn load_document_or_not_found(
    collection: &Collection<WorkoutSummaryDocument>,
    user_id: &str,
    workout_id: &str,
) -> Result<WorkoutSummaryDocument, WorkoutSummaryError> {
    find_preferred_document(collection, user_id, workout_id)
        .await?
        .ok_or(WorkoutSummaryError::NotFound)
}

fn updated_at_fields(updated_at_epoch_seconds: i64) -> Result<Document, WorkoutSummaryError> {
    Ok(doc! {
        "updated_at_epoch_seconds": updated_at_epoch_seconds,
        "updated_at": optional_epoch_seconds_to_bson_datetime(
            Some(updated_at_epoch_seconds),
            "updated_at",
        )
        .map_err(WorkoutSummaryError::Repository)?,
    })
}
