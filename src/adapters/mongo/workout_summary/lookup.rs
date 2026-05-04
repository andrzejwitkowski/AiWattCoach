use std::collections::{BTreeMap, HashSet};

use futures::TryStreamExt;
use mongodb::{
    bson::{doc, Bson, Document},
    Collection,
};

use crate::domain::{
    completed_workouts::{canonical_completed_workout_id, completed_workout_activity_id},
    workout_summary::WorkoutSummaryError,
};

use super::document::{WorkoutSummaryDocument, WorkoutSummaryMessageLookupDocument};

pub(super) async fn find_preferred_document(
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

pub(super) async fn find_preferred_documents(
    collection: &Collection<WorkoutSummaryDocument>,
    user_id: &str,
    workout_ids: &[String],
) -> Result<Vec<WorkoutSummaryDocument>, WorkoutSummaryError> {
    if workout_ids.is_empty() {
        return Ok(Vec::new());
    }

    let current_documents = find_current_documents(collection, user_id, workout_ids).await?;
    let mut preferred_by_storage_workout_id =
        current_documents_by_storage_workout_id(current_documents);

    // Migration semantics: current documents keyed by `workout_id` win first.
    // Legacy documents are fetched by `event_id` only for missing workout ids,
    // and `or_insert` keeps the current `workout_id` match preferred when both exist.
    load_legacy_documents_for_missing_workout_ids(
        collection,
        user_id,
        workout_ids,
        &mut preferred_by_storage_workout_id,
    )
    .await?;

    Ok(remap_documents_to_requested_workout_ids(
        workout_ids,
        preferred_by_storage_workout_id,
    ))
}

pub(super) async fn find_preferred_message_lookup_document(
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

pub(super) fn document_identity_filter(document: &WorkoutSummaryDocument) -> Document {
    match document.id {
        Some(id) => doc! { "_id": id },
        None => doc! {
            "summary_id": &document.summary_id,
            "user_id": &document.user_id,
        },
    }
}

pub(super) fn editable_document_identity_filter(document: &WorkoutSummaryDocument) -> Document {
    let mut filter = document_identity_filter(document);
    filter.insert("saved_at_epoch_seconds", Bson::Null);
    filter.insert("saved_at", Bson::Null);
    filter
}

pub(super) fn document_is_locked(document: &WorkoutSummaryDocument) -> bool {
    document.saved_at.is_some() || document.saved_at_epoch_seconds.is_some()
}

pub(super) fn current_workout_id_filter(user_id: &str, workout_id: &str) -> Document {
    doc! {
        "user_id": user_id,
        "workout_id": workout_id,
    }
}

pub(super) fn legacy_event_id_filter(user_id: &str, workout_id: &str) -> Document {
    doc! {
        "user_id": user_id,
        "event_id": workout_id,
    }
}

pub(super) fn with_message_append_filter(mut filter: Document, message_id: &str) -> Document {
    filter.insert("saved_at_epoch_seconds", Bson::Null);
    filter.insert("saved_at", Bson::Null);
    filter.insert("messages.id", doc! { "$ne": message_id });
    filter
}

async fn find_current_documents(
    collection: &Collection<WorkoutSummaryDocument>,
    user_id: &str,
    workout_ids: &[String],
) -> Result<Vec<WorkoutSummaryDocument>, WorkoutSummaryError> {
    let current_lookup_ids = current_lookup_ids_for_requests(workout_ids);

    collection
        .find(doc! {
            "user_id": user_id,
            "workout_id": { "$in": current_lookup_ids },
        })
        .await
        .map_err(|error| WorkoutSummaryError::Repository(error.to_string()))?
        .try_collect::<Vec<_>>()
        .await
        .map_err(|error| WorkoutSummaryError::Repository(error.to_string()))
}

fn current_lookup_ids_for_requests(workout_ids: &[String]) -> Vec<String> {
    let mut lookup_ids = Vec::new();
    let mut seen = HashSet::new();

    for workout_id in workout_ids {
        for candidate in current_lookup_ids_for_request(workout_id) {
            if seen.insert(candidate.clone()) {
                lookup_ids.push(candidate);
            }
        }
    }

    lookup_ids
}

fn current_documents_by_storage_workout_id(
    current_documents: Vec<WorkoutSummaryDocument>,
) -> BTreeMap<String, WorkoutSummaryDocument> {
    current_documents
        .into_iter()
        .map(|document| (document.workout_id.clone(), document))
        .collect()
}

async fn load_legacy_documents_for_missing_workout_ids(
    collection: &Collection<WorkoutSummaryDocument>,
    user_id: &str,
    workout_ids: &[String],
    preferred_by_storage_workout_id: &mut BTreeMap<String, WorkoutSummaryDocument>,
) -> Result<(), WorkoutSummaryError> {
    let missing_workout_ids = missing_workout_ids(preferred_by_storage_workout_id, workout_ids);
    if missing_workout_ids.is_empty() {
        return Ok(());
    }

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

    Ok(())
}

fn missing_workout_ids(
    preferred_by_storage_workout_id: &BTreeMap<String, WorkoutSummaryDocument>,
    workout_ids: &[String],
) -> Vec<String> {
    let stored_ids = preferred_by_storage_workout_id
        .keys()
        .cloned()
        .collect::<HashSet<_>>();
    let stored_activity_ids = stored_ids
        .iter()
        .map(|id| completed_workout_activity_id(id))
        .collect::<HashSet<_>>();

    workout_ids
        .iter()
        .filter(|workout_id| {
            let activity_id = completed_workout_activity_id(workout_id);
            !stored_ids.contains(workout_id.as_str())
                && !stored_ids.contains(&canonical_completed_workout_id(workout_id))
                && !stored_activity_ids.contains(activity_id)
        })
        .cloned()
        .collect()
}

fn remap_documents_to_requested_workout_ids(
    workout_ids: &[String],
    mut preferred_by_storage_workout_id: BTreeMap<String, WorkoutSummaryDocument>,
) -> Vec<WorkoutSummaryDocument> {
    let mut preferred_by_requested_workout_id = BTreeMap::new();

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

    workout_ids
        .iter()
        .filter_map(|workout_id| preferred_by_requested_workout_id.remove(workout_id))
        .collect()
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
