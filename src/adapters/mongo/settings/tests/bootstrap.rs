use mongodb::bson::{doc, to_document, Document};

use super::super::{
    bootstrap::{
        bootstrap_intervals_updated_at, build_intervals_poll_bootstrap_filter, has_non_empty,
        is_bootstrap_active_intervals, IntervalsPollBootstrapIntervalsDocument,
        IntervalsPollBootstrapUser, IntervalsPollBootstrapUserDocument,
    },
    documents::{IntervalsDocument, SettingsDocument},
    repository::MongoUserSettingsRepository,
};
use super::support::{
    build_settings_document, test_mongo_client_or_skip, unique_test_database_name,
};

#[test]
fn has_non_empty_trims_whitespace() {
    assert!(has_non_empty(Some("value")));
    assert!(has_non_empty(Some(" value ")));
    assert!(!has_non_empty(Some("   ")));
    assert!(!has_non_empty(None));
}

#[test]
fn build_intervals_poll_bootstrap_filter_matches_non_empty_credentials_or_existing_users() {
    let filter = build_intervals_poll_bootstrap_filter(&["user-1".to_string()]);

    assert_eq!(
        filter,
        doc! {
            "$or": [
                {
                    "$and": [
                        { "intervals.api_key": { "$type": "string", "$regex": "\\S" } },
                        { "intervals.athlete_id": { "$type": "string", "$regex": "\\S" } },
                        { "intervals.connected": { "$ne": false } },
                    ]
                },
                { "user_id": { "$in": ["user-1"] } },
            ]
        }
    );
}

#[test]
fn build_intervals_poll_bootstrap_filter_omits_user_id_clause_without_existing_users() {
    let filter = build_intervals_poll_bootstrap_filter(&[]);

    assert_eq!(
        filter,
        doc! {
            "$or": [
                {
                    "$and": [
                        { "intervals.api_key": { "$type": "string", "$regex": "\\S" } },
                        { "intervals.athlete_id": { "$type": "string", "$regex": "\\S" } },
                        { "intervals.connected": { "$ne": false } },
                    ]
                },
            ]
        }
    );
}

#[test]
fn is_bootstrap_active_intervals_requires_complete_connected_credentials() {
    assert!(is_bootstrap_active_intervals(
        &IntervalsPollBootstrapIntervalsDocument {
            api_key: Some("api-key".to_string()),
            athlete_id: Some("athlete-1".to_string()),
            connected: Some(true),
            updated_at_epoch_seconds: Some(10),
        }
    ));
    assert!(is_bootstrap_active_intervals(
        &IntervalsPollBootstrapIntervalsDocument {
            api_key: Some("api-key".to_string()),
            athlete_id: Some("athlete-1".to_string()),
            connected: None,
            updated_at_epoch_seconds: None,
        }
    ));
    assert!(!is_bootstrap_active_intervals(
        &IntervalsPollBootstrapIntervalsDocument {
            api_key: Some(" ".to_string()),
            athlete_id: Some("athlete-1".to_string()),
            connected: Some(true),
            updated_at_epoch_seconds: Some(10),
        }
    ));
    assert!(!is_bootstrap_active_intervals(
        &IntervalsPollBootstrapIntervalsDocument {
            api_key: Some("api-key".to_string()),
            athlete_id: None,
            connected: Some(true),
            updated_at_epoch_seconds: Some(10),
        }
    ));
    assert!(!is_bootstrap_active_intervals(
        &IntervalsPollBootstrapIntervalsDocument {
            api_key: Some("api-key".to_string()),
            athlete_id: Some("athlete-1".to_string()),
            connected: Some(false),
            updated_at_epoch_seconds: Some(10),
        }
    ));
}

#[test]
fn bootstrap_intervals_updated_at_falls_back_to_document_updated_at() {
    let document = IntervalsPollBootstrapUserDocument {
        user_id: "user-1".to_string(),
        updated_at_epoch_seconds: Some(40),
        intervals: Some(IntervalsPollBootstrapIntervalsDocument {
            api_key: Some("api-key".to_string()),
            athlete_id: Some("athlete-1".to_string()),
            connected: Some(true),
            updated_at_epoch_seconds: None,
        }),
    };

    assert_eq!(bootstrap_intervals_updated_at(&document), Some(40));
}

#[tokio::test]
async fn list_intervals_poll_bootstrap_users_keeps_existing_poll_users_even_when_disconnected() {
    let Some(client) = test_mongo_client_or_skip().await else {
        return;
    };
    let database_name = unique_test_database_name("user-settings-poll-bootstrap");
    let repository = MongoUserSettingsRepository::new(client.clone(), &database_name);
    let collection = client
        .database(&database_name)
        .collection::<Document>("user_settings");

    collection
        .insert_many([
            to_document(&SettingsDocument {
                intervals: IntervalsDocument {
                    api_key: Some("api-key".to_string()),
                    athlete_id: Some("athlete-1".to_string()),
                    connected: true,
                    updated_at_epoch_seconds: Some(10),
                    updated_at: None,
                },
                ..build_settings_document("connected-user", 10)
            })
            .unwrap(),
            to_document(&SettingsDocument {
                intervals: IntervalsDocument {
                    api_key: Some("legacy-key".to_string()),
                    athlete_id: Some("legacy-athlete".to_string()),
                    connected: true,
                    updated_at_epoch_seconds: Some(20),
                    updated_at: None,
                },
                ..build_settings_document("connected-user-2", 20)
            })
            .unwrap(),
            to_document(&SettingsDocument {
                intervals: IntervalsDocument {
                    api_key: Some("old-key".to_string()),
                    athlete_id: Some("old-athlete".to_string()),
                    connected: false,
                    updated_at_epoch_seconds: Some(30),
                    updated_at: None,
                },
                ..build_settings_document("explicitly-disconnected-user", 30)
            })
            .unwrap(),
            doc! {
                "user_id": "legacy-missing-connected",
                "ai_agents": {},
                "intervals": { "api_key": "legacy-key", "athlete_id": "legacy-athlete" },
                "options": {},
                "availability": { "configured": false, "days": [] },
                "cycling": {},
                "created_at_epoch_seconds": 1,
                "updated_at_epoch_seconds": 40
            },
            doc! {
                "user_id": "poll-only-user",
                "ai_agents": {},
                "intervals": {},
                "options": {},
                "availability": { "configured": false, "days": [] },
                "cycling": {},
                "created_at_epoch_seconds": 1,
                "updated_at_epoch_seconds": 60
            },
            to_document(&SettingsDocument {
                intervals: IntervalsDocument {
                    api_key: Some("   ".to_string()),
                    athlete_id: Some("athlete-2".to_string()),
                    connected: true,
                    updated_at_epoch_seconds: Some(50),
                    updated_at: None,
                },
                ..build_settings_document("invalid-connected-user", 50)
            })
            .unwrap(),
            doc! {
                "user_id": "blank-legacy-user",
                "ai_agents": {},
                "intervals": {
                    "api_key": "   ",
                    "athlete_id": "   ",
                    "connected": true,
                    "updated_at_epoch_seconds": null
                },
                "options": {},
                "availability": { "configured": false, "days": [] },
                "cycling": {},
                "created_at_epoch_seconds": 1,
                "updated_at_epoch_seconds": 55
            },
            to_document(&build_settings_document("disconnected-user", 40)).unwrap(),
        ])
        .await
        .unwrap();

    let users = repository
        .list_intervals_poll_bootstrap_users(&[
            "explicitly-disconnected-user".to_string(),
            "disconnected-user".to_string(),
            "poll-only-user".to_string(),
        ])
        .await
        .unwrap();

    assert_eq!(
        users,
        vec![
            IntervalsPollBootstrapUser {
                user_id: "connected-user".to_string(),
                desired_active: true,
                intervals_updated_at_epoch_seconds: Some(10)
            },
            IntervalsPollBootstrapUser {
                user_id: "connected-user-2".to_string(),
                desired_active: true,
                intervals_updated_at_epoch_seconds: Some(20)
            },
            IntervalsPollBootstrapUser {
                user_id: "disconnected-user".to_string(),
                desired_active: false,
                intervals_updated_at_epoch_seconds: Some(40)
            },
            IntervalsPollBootstrapUser {
                user_id: "explicitly-disconnected-user".to_string(),
                desired_active: false,
                intervals_updated_at_epoch_seconds: Some(30)
            },
            IntervalsPollBootstrapUser {
                user_id: "legacy-missing-connected".to_string(),
                desired_active: true,
                intervals_updated_at_epoch_seconds: Some(40)
            },
            IntervalsPollBootstrapUser {
                user_id: "poll-only-user".to_string(),
                desired_active: false,
                intervals_updated_at_epoch_seconds: Some(60)
            },
        ]
    );
    assert!(!users.iter().any(|user| user.user_id == "blank-legacy-user"));

    client.database(&database_name).drop().await.unwrap();
}
