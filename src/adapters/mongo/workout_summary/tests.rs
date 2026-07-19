use mongodb::bson::{doc, from_document, Bson, DateTime};

use super::{
    document::{CoachQuestionDocument, ConversationMessageDocument, WorkoutSummaryDocument},
    lookup::{
        current_workout_id_filter, document_identity_filter, document_is_locked,
        editable_document_identity_filter, legacy_event_id_filter, with_message_append_filter,
    },
    mapping::{map_document_to_domain, map_domain_to_document, map_message_to_domain},
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
        provider_transcript: Vec::new(),
        saved_at_epoch_seconds: None,
        saved_at: None,
        workout_recap_text: None,
        workout_recap_provider: None,
        workout_recap_model: None,
        workout_recap_generated_at_epoch_seconds: None,
        workout_recap_generated_at: None,
        created_at_epoch_seconds: Some(1),
        created_at: None,
        updated_at_epoch_seconds: Some(1),
        updated_at: None,
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
fn workout_summary_document_reads_datetime_fields_without_legacy_epoch() {
    let document = WorkoutSummaryDocument {
        id: None,
        summary_id: "summary-1".to_string(),
        user_id: "user-1".to_string(),
        workout_id: "workout-1".to_string(),
        rpe: Some(6),
        messages: vec![ConversationMessageDocument {
            id: "message-1".to_string(),
            role: "user".to_string(),
            content: "hello".to_string(),
            tool_call: None,
            questions: Vec::new(),
            created_at_epoch_seconds: None,
            created_at: Some(DateTime::from_millis(1_700_000_000_000)),
            image_url: None,
        }],
        provider_transcript: Vec::new(),
        saved_at_epoch_seconds: None,
        saved_at: Some(DateTime::from_millis(1_700_000_010_000)),
        workout_recap_text: None,
        workout_recap_provider: None,
        workout_recap_model: None,
        workout_recap_generated_at_epoch_seconds: None,
        workout_recap_generated_at: Some(DateTime::from_millis(1_700_000_020_000)),
        created_at_epoch_seconds: None,
        created_at: Some(DateTime::from_millis(1_700_000_030_000)),
        updated_at_epoch_seconds: None,
        updated_at: Some(DateTime::from_millis(1_700_000_040_000)),
    };

    let summary = map_document_to_domain(document).expect("datetime-backed document should map");

    assert_eq!(summary.messages[0].created_at_epoch_seconds, 1_700_000_000);
    assert_eq!(summary.saved_at_epoch_seconds, Some(1_700_000_010));
    assert_eq!(
        summary.workout_recap_generated_at_epoch_seconds,
        Some(1_700_000_020)
    );
    assert_eq!(summary.created_at_epoch_seconds, 1_700_000_030);
    assert_eq!(summary.updated_at_epoch_seconds, 1_700_000_040);
}

#[test]
fn map_domain_to_document_includes_recap_fields() {
    let summary = WorkoutSummary {
        id: "summary-1".to_string(),
        user_id: "user-1".to_string(),
        workout_id: "workout-1".to_string(),
        rpe: Some(6),
        messages: Vec::new(),
        provider_transcript: Vec::new(),
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
fn current_workout_filter_matches_workout_id() {
    assert_eq!(
        current_workout_id_filter("user-1", "workout-1"),
        doc! {
            "user_id": "user-1",
            "workout_id": "workout-1",
        }
    );
}

#[test]
fn legacy_event_filter_matches_event_id() {
    assert_eq!(
        legacy_event_id_filter("user-1", "workout-1"),
        doc! {
            "user_id": "user-1",
            "event_id": "workout-1",
        }
    );
}

#[test]
fn document_identity_filter_prefers_object_id() {
    let id = mongodb::bson::oid::ObjectId::parse_str("507f1f77bcf86cd799439011").unwrap();
    let document = WorkoutSummaryDocument {
        id: Some(id),
        summary_id: "summary-1".to_string(),
        user_id: "user-1".to_string(),
        workout_id: "workout-1".to_string(),
        rpe: None,
        messages: Vec::new(),
        provider_transcript: Vec::new(),
        saved_at_epoch_seconds: None,
        saved_at: None,
        workout_recap_text: None,
        workout_recap_provider: None,
        workout_recap_model: None,
        workout_recap_generated_at_epoch_seconds: None,
        workout_recap_generated_at: None,
        created_at_epoch_seconds: Some(1),
        created_at: None,
        updated_at_epoch_seconds: Some(1),
        updated_at: None,
    };

    assert_eq!(document_identity_filter(&document), doc! { "_id": id });
}

#[test]
fn document_identity_filter_falls_back_to_summary_and_user() {
    let document = WorkoutSummaryDocument {
        id: None,
        summary_id: "summary-1".to_string(),
        user_id: "user-1".to_string(),
        workout_id: "workout-1".to_string(),
        rpe: None,
        messages: Vec::new(),
        provider_transcript: Vec::new(),
        saved_at_epoch_seconds: None,
        saved_at: None,
        workout_recap_text: None,
        workout_recap_provider: None,
        workout_recap_model: None,
        workout_recap_generated_at_epoch_seconds: None,
        workout_recap_generated_at: None,
        created_at_epoch_seconds: Some(1),
        created_at: None,
        updated_at_epoch_seconds: Some(1),
        updated_at: None,
    };

    assert_eq!(
        document_identity_filter(&document),
        doc! {
            "summary_id": "summary-1",
            "user_id": "user-1",
        }
    );
}

#[test]
fn document_is_locked_when_saved_at_datetime_exists_without_legacy_epoch() {
    let document = WorkoutSummaryDocument {
        id: None,
        summary_id: "summary-1".to_string(),
        user_id: "user-1".to_string(),
        workout_id: "workout-1".to_string(),
        rpe: None,
        messages: Vec::new(),
        provider_transcript: Vec::new(),
        saved_at_epoch_seconds: None,
        saved_at: Some(DateTime::from_millis(1_700_000_000_000)),
        workout_recap_text: None,
        workout_recap_provider: None,
        workout_recap_model: None,
        workout_recap_generated_at_epoch_seconds: None,
        workout_recap_generated_at: None,
        created_at_epoch_seconds: Some(1),
        created_at: None,
        updated_at_epoch_seconds: Some(1),
        updated_at: None,
    };

    assert!(document_is_locked(&document));
}

#[test]
fn editable_document_identity_filter_requires_datetime_mirror_to_be_null() {
    let document = WorkoutSummaryDocument {
        id: None,
        summary_id: "summary-1".to_string(),
        user_id: "user-1".to_string(),
        workout_id: "workout-1".to_string(),
        rpe: None,
        messages: Vec::new(),
        provider_transcript: Vec::new(),
        saved_at_epoch_seconds: None,
        saved_at: None,
        workout_recap_text: None,
        workout_recap_provider: None,
        workout_recap_model: None,
        workout_recap_generated_at_epoch_seconds: None,
        workout_recap_generated_at: None,
        created_at_epoch_seconds: Some(1),
        created_at: None,
        updated_at_epoch_seconds: Some(1),
        updated_at: None,
    };

    let filter = editable_document_identity_filter(&document);

    assert_eq!(filter.get("saved_at_epoch_seconds"), Some(&Bson::Null));
    assert_eq!(filter.get("saved_at"), Some(&Bson::Null));
}

#[test]
fn message_append_filter_requires_datetime_mirror_to_be_null() {
    let filter = with_message_append_filter(doc! { "summary_id": "summary-1" }, "message-1");

    assert_eq!(filter.get("saved_at_epoch_seconds"), Some(&Bson::Null));
    assert_eq!(filter.get("saved_at"), Some(&Bson::Null));
    assert_eq!(
        filter.get_document("messages.id").unwrap(),
        &doc! { "$ne": "message-1" }
    );
}

#[test]
fn map_message_to_domain_round_trips_questions() {
    let document = ConversationMessageDocument {
        id: "msg-1".to_string(),
        role: "coach".to_string(),
        content: "Legs were the limiter.".to_string(),
        tool_call: None,
        questions: vec![CoachQuestionDocument {
            id: "question-1".to_string(),
            question: "What limited you most?".to_string(),
            answers: vec![
                "Legs".to_string(),
                "Breathing".to_string(),
                "Fueling".to_string(),
            ],
            free_text_label: Some("Add detail".to_string()),
        }],
        created_at_epoch_seconds: Some(1_700_000_000),
        created_at: None,
        image_url: None,
    };

    let message = map_message_to_domain(document).expect("coach message with questions should map");

    assert_eq!(message.questions.len(), 1);
    assert_eq!(message.questions[0].id, "question-1");
    assert_eq!(message.questions[0].question, "What limited you most?");
    assert_eq!(
        message.questions[0].answers,
        vec!["Legs", "Breathing", "Fueling"]
    );
    assert_eq!(
        message.questions[0].free_text_label.as_deref(),
        Some("Add detail")
    );
}

#[test]
fn map_message_to_domain_defaults_missing_questions_to_empty() {
    use mongodb::bson::from_document;

    let document: ConversationMessageDocument = from_document(doc! {
        "id": "msg-old",
        "role": "coach",
        "content": "Old message without questions field.",
        "created_at_epoch_seconds": 1_700_000_000_i64,
    })
    .expect("legacy document without questions field should deserialize");

    let message =
        map_message_to_domain(document).expect("legacy message without questions should map");

    assert!(message.questions.is_empty());
}
