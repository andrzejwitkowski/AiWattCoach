use mongodb::bson::{doc, from_document, Bson};

use super::{
    legacy_workout_id_filter, legacy_workout_ids_filter, map_document_to_domain,
    map_domain_to_document, ConversationMessageDocument, WorkoutSummaryDocument,
};
use crate::domain::workout_summary::{WorkoutSummary, WorkoutSummaryError};

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
        workout_recap_text: None,
        workout_recap_provider: None,
        workout_recap_model: None,
        workout_recap_generated_at_epoch_seconds: None,
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

#[test]
fn workout_summary_document_defaults_missing_recap_fields_to_none() {
    let document: WorkoutSummaryDocument = from_document(doc! {
        "summary_id": "summary-1",
        "user_id": "user-1",
        "workout_id": "workout-1",
        "rpe": 6,
        "messages": [],
        "saved_at_epoch_seconds": Bson::Null,
        "created_at_epoch_seconds": 1,
        "updated_at_epoch_seconds": 2,
    })
    .expect("legacy documents without recap fields should deserialize");

    let summary = map_document_to_domain(document).expect("legacy document should map");

    assert_eq!(summary.workout_recap_text, None);
    assert_eq!(summary.workout_recap_provider, None);
    assert_eq!(summary.workout_recap_model, None);
    assert_eq!(summary.workout_recap_generated_at_epoch_seconds, None);
}

#[test]
fn map_domain_to_document_includes_recap_fields() {
    let summary = WorkoutSummary {
        id: "summary-1".to_string(),
        user_id: "user-1".to_string(),
        workout_id: "workout-1".to_string(),
        rpe: Some(6),
        messages: Vec::new(),
        saved_at_epoch_seconds: None,
        workout_recap_text: Some("Strong close after a controlled opener.".to_string()),
        workout_recap_provider: Some("openai".to_string()),
        workout_recap_model: Some("gpt-5.4-mini".to_string()),
        workout_recap_generated_at_epoch_seconds: Some(123),
        created_at_epoch_seconds: 1,
        updated_at_epoch_seconds: 2,
    };

    let document = map_domain_to_document(&summary);

    assert_eq!(
        document.workout_recap_text,
        Some("Strong close after a controlled opener.".to_string())
    );
    assert_eq!(document.workout_recap_provider, Some("openai".to_string()));
    assert_eq!(
        document.workout_recap_model,
        Some("gpt-5.4-mini".to_string())
    );
    assert_eq!(document.workout_recap_generated_at_epoch_seconds, Some(123));
}

#[test]
fn legacy_workout_filter_matches_workout_id_or_event_id() {
    assert_eq!(
        legacy_workout_id_filter("user-1", "workout-1"),
        doc! {
            "user_id": "user-1",
            "$or": [
                { "workout_id": "workout-1" },
                { "event_id": "workout-1" },
            ]
        }
    );
}

#[test]
fn legacy_workout_ids_filter_matches_workout_id_or_event_id_lists() {
    assert_eq!(
        legacy_workout_ids_filter(
            "user-1",
            &["workout-1".to_string(), "workout-2".to_string()]
        ),
        doc! {
            "user_id": "user-1",
            "$or": [
                { "workout_id": { "$in": ["workout-1", "workout-2"] } },
                { "event_id": { "$in": ["workout-1", "workout-2"] } },
            ]
        }
    );
}
