use std::time::{SystemTime, UNIX_EPOCH};

use mongodb::{
    bson::{doc, oid::ObjectId, to_document, DateTime, Document},
    Client,
};

use super::{
    document::{
        default_availability_document, AiAgentsDocument, AvailabilityDayDocument,
        AvailabilityDocument, CyclingDocument, IntervalsDocument,
        IntervalsPollBootstrapIntervalsDocument, IntervalsPollBootstrapUser,
        IntervalsPollBootstrapUserDocument, OptionsDocument, SettingsDocument, WahooDocument,
    },
    mapping::{
        map_document_availability_to_domain, map_document_to_domain,
        map_domain_availability_to_document,
    },
    repository::{
        bootstrap_intervals_updated_at, build_intervals_poll_bootstrap_filter, has_non_empty,
        is_bootstrap_active_intervals, MongoUserSettingsRepository,
    },
};
use crate::domain::settings::{
    AvailabilityDay, AvailabilitySettings, UserSettingsRepository, Weekday,
};

#[test]
fn ai_agents_document_debug_redacts_api_keys() {
    let document = AiAgentsDocument {
        openai_api_key: Some("sk-openai-secret".to_string()),
        gemini_api_key: Some("sk-gemini-secret".to_string()),
        openrouter_api_key: Some("sk-openrouter-secret".to_string()),
        deepseek_api_key: Some("sk-deepseek-secret".to_string()),
        zai_api_key: Some("sk-zai-secret".to_string()),
        openai_compatible_api_key: Some("sk-compat-secret".to_string()),
        openai_compatible_base_url: Some("http://127.0.0.1:8080/v1".to_string()),
        selected_provider: Some("openai".to_string()),
        selected_model: Some("gpt-4o-mini".to_string()),
        ..AiAgentsDocument::default()
    };

    let debug = format!("{document:?}");
    assert!(debug.contains("<redacted:"));
    assert!(!debug.contains("sk-openai-secret"));
    assert!(!debug.contains("sk-gemini-secret"));
    assert!(!debug.contains("sk-openrouter-secret"));
    assert!(!debug.contains("sk-deepseek-secret"));
    assert!(!debug.contains("sk-zai-secret"));
    assert!(!debug.contains("sk-compat-secret"));
    assert!(debug.contains("gpt-4o-mini"));
}

#[test]
fn settings_document_deserializes_missing_availability_with_full_week_default() {
    let document = serde_json::json!({
        "user_id": "user-1",
        "ai_agents": {},
        "intervals": {},
        "options": {},
        "cycling": {},
        "created_at_epoch_seconds": 1,
        "updated_at_epoch_seconds": 1
    });

    let parsed: SettingsDocument = serde_json::from_value(document).unwrap();

    assert!(!parsed.availability.configured);
    assert_eq!(parsed.availability.days.len(), 7);
    assert!(parsed.availability.days.iter().all(|day| !day.available));
    assert!(!parsed.wahoo.connected);
}

#[test]
fn wahoo_document_debug_redacts_tokens() {
    let document = SettingsDocument {
        wahoo: WahooDocument {
            access_token: Some("wahoo-access-token".to_string()),
            refresh_token: Some("wahoo-refresh-token".to_string()),
            expires_at_epoch_seconds: Some(123),
            expires_at: Some(DateTime::from_millis(123_000)),
            user_id: Some(60_462),
            updated_at_epoch_seconds: Some(456),
            updated_at: Some(DateTime::from_millis(456_000)),
            connected: true,
        },
        ..build_settings_document("user-1", 1)
    };

    let debug = format!("{document:?}");

    assert!(debug.contains("<redacted:"));
    assert!(!debug.contains("wahoo-access-token"));
    assert!(!debug.contains("wahoo-refresh-token"));
    assert!(debug.contains("expires_at_epoch_seconds"));
    assert!(debug.contains("expires_at"));
    assert!(debug.contains("updated_at"));
}

#[test]
fn cycling_document_debug_includes_datetime_mirror() {
    let document = SettingsDocument {
        cycling: CyclingDocument {
            last_zone_update_epoch_seconds: Some(789),
            last_zone_update_at: Some(DateTime::from_millis(789_000)),
            ..CyclingDocument::default()
        },
        ..build_settings_document("user-1", 1)
    };

    let debug = format!("{:?}", document.cycling);

    assert!(debug.contains("last_zone_update_epoch_seconds"));
    assert!(debug.contains("last_zone_update_at"));
}

#[test]
fn map_document_to_domain_returns_repository_error_when_required_timestamps_are_missing() {
    let error = map_document_to_domain(SettingsDocument {
        created_at_epoch_seconds: None,
        created_at: None,
        updated_at_epoch_seconds: Some(1),
        updated_at: None,
        ..build_settings_document("user-1", 1)
    })
    .unwrap_err();

    assert_eq!(
        error,
        crate::domain::settings::SettingsError::Repository(
            "missing created_at timestamp".to_string()
        )
    );
}

#[test]
fn map_document_availability_to_domain_falls_back_for_legacy_empty_days() {
    let availability = map_document_availability_to_domain(AvailabilityDocument {
        configured: false,
        days: Vec::new(),
    });

    assert!(!availability.configured);
    assert_eq!(availability.days.len(), 7);
    assert!(availability.days.iter().all(|day| !day.available));
}

#[test]
fn map_document_availability_to_domain_repairs_case_and_missing_days() {
    let availability = map_document_availability_to_domain(AvailabilityDocument {
        configured: true,
        days: vec![
            AvailabilityDayDocument {
                weekday: " MON ".to_string(),
                available: true,
                max_duration_minutes: Some(60),
            },
            AvailabilityDayDocument {
                weekday: Weekday::Tue.as_str().to_string(),
                available: false,
                max_duration_minutes: Some(90),
            },
        ],
    });

    assert!(!availability.is_configured());
    assert_eq!(availability.days.len(), 7);
    assert_eq!(availability.days[0].weekday, Weekday::Mon);
    assert_eq!(availability.days[0].max_duration_minutes, Some(60));
    assert_eq!(availability.days[1].weekday, Weekday::Tue);
    assert_eq!(availability.days[1].max_duration_minutes, None);
    assert!(availability.days[2..].iter().all(|day| !day.available));
}

#[test]
fn map_document_availability_to_domain_sanitizes_invalid_duration_without_resetting_week() {
    let availability = map_document_availability_to_domain(AvailabilityDocument {
        configured: true,
        days: vec![
            AvailabilityDayDocument {
                weekday: Weekday::Mon.as_str().to_string(),
                available: true,
                max_duration_minutes: Some(45),
            },
            AvailabilityDayDocument {
                weekday: Weekday::Tue.as_str().to_string(),
                available: false,
                max_duration_minutes: None,
            },
            AvailabilityDayDocument {
                weekday: Weekday::Wed.as_str().to_string(),
                available: true,
                max_duration_minutes: Some(90),
            },
            AvailabilityDayDocument {
                weekday: Weekday::Thu.as_str().to_string(),
                available: false,
                max_duration_minutes: None,
            },
            AvailabilityDayDocument {
                weekday: Weekday::Fri.as_str().to_string(),
                available: false,
                max_duration_minutes: None,
            },
            AvailabilityDayDocument {
                weekday: Weekday::Sat.as_str().to_string(),
                available: false,
                max_duration_minutes: None,
            },
            AvailabilityDayDocument {
                weekday: Weekday::Sun.as_str().to_string(),
                available: false,
                max_duration_minutes: None,
            },
        ],
    });

    assert!(availability.is_configured());
    assert_eq!(availability.days[0].weekday, Weekday::Mon);
    assert!(!availability.days[0].available);
    assert_eq!(availability.days[0].max_duration_minutes, None);
    assert_eq!(availability.days[2].weekday, Weekday::Wed);
    assert!(availability.days[2].available);
    assert_eq!(availability.days[2].max_duration_minutes, Some(90));
}

#[test]
fn map_document_availability_to_domain_keeps_partial_legacy_week_unconfigured() {
    let availability = map_document_availability_to_domain(AvailabilityDocument {
        configured: true,
        days: vec![
            AvailabilityDayDocument {
                weekday: Weekday::Mon.as_str().to_string(),
                available: true,
                max_duration_minutes: Some(60),
            },
            AvailabilityDayDocument {
                weekday: Weekday::Tue.as_str().to_string(),
                available: false,
                max_duration_minutes: None,
            },
        ],
    });

    assert!(!availability.configured);
    assert!(!availability.is_configured());
    assert!(availability.days[0].available);
    assert_eq!(availability.days[0].max_duration_minutes, Some(60));
}

#[test]
fn map_document_availability_to_domain_treats_duplicate_weekdays_as_unconfigured() {
    let availability = map_document_availability_to_domain(AvailabilityDocument {
        configured: true,
        days: vec![
            AvailabilityDayDocument {
                weekday: Weekday::Mon.as_str().to_string(),
                available: true,
                max_duration_minutes: Some(60),
            },
            AvailabilityDayDocument {
                weekday: Weekday::Mon.as_str().to_string(),
                available: false,
                max_duration_minutes: None,
            },
            AvailabilityDayDocument {
                weekday: Weekday::Tue.as_str().to_string(),
                available: false,
                max_duration_minutes: None,
            },
            AvailabilityDayDocument {
                weekday: Weekday::Wed.as_str().to_string(),
                available: false,
                max_duration_minutes: None,
            },
            AvailabilityDayDocument {
                weekday: Weekday::Thu.as_str().to_string(),
                available: false,
                max_duration_minutes: None,
            },
            AvailabilityDayDocument {
                weekday: Weekday::Fri.as_str().to_string(),
                available: false,
                max_duration_minutes: None,
            },
            AvailabilityDayDocument {
                weekday: Weekday::Sat.as_str().to_string(),
                available: false,
                max_duration_minutes: None,
            },
            AvailabilityDayDocument {
                weekday: Weekday::Sun.as_str().to_string(),
                available: false,
                max_duration_minutes: None,
            },
        ],
    });

    assert!(!availability.configured);
    assert!(!availability.is_configured());
}

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
                "intervals": {
                    "api_key": "legacy-key",
                    "athlete_id": "legacy-athlete"
                },
                "options": {},
                "availability": {
                    "configured": false,
                    "days": []
                },
                "cycling": {},
                "created_at_epoch_seconds": 1,
                "updated_at_epoch_seconds": 40
            },
            doc! {
                "user_id": "poll-only-user",
                "ai_agents": {},
                "intervals": {},
                "options": {},
                "availability": {
                    "configured": false,
                    "days": []
                },
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
                "availability": {
                    "configured": false,
                    "days": []
                },
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
                intervals_updated_at_epoch_seconds: Some(10),
            },
            IntervalsPollBootstrapUser {
                user_id: "connected-user-2".to_string(),
                desired_active: true,
                intervals_updated_at_epoch_seconds: Some(20),
            },
            IntervalsPollBootstrapUser {
                user_id: "disconnected-user".to_string(),
                desired_active: false,
                intervals_updated_at_epoch_seconds: Some(40),
            },
            IntervalsPollBootstrapUser {
                user_id: "explicitly-disconnected-user".to_string(),
                desired_active: false,
                intervals_updated_at_epoch_seconds: Some(30),
            },
            IntervalsPollBootstrapUser {
                user_id: "legacy-missing-connected".to_string(),
                desired_active: true,
                intervals_updated_at_epoch_seconds: Some(40),
            },
            IntervalsPollBootstrapUser {
                user_id: "poll-only-user".to_string(),
                desired_active: false,
                intervals_updated_at_epoch_seconds: Some(60),
            },
        ]
    );
    assert!(!users.iter().any(|user| user.user_id == "blank-legacy-user"));

    client.database(&database_name).drop().await.unwrap();
}

#[tokio::test]
async fn update_availability_updates_only_target_user_document() {
    let Some(client) = test_mongo_client_or_skip().await else {
        return;
    };
    let database_name = unique_test_database_name("user-settings-availability");
    let repository = MongoUserSettingsRepository::new(client.clone(), &database_name);
    let collection = client
        .database(&database_name)
        .collection::<SettingsDocument>("user_settings");

    let user_1_id = "user-availability-target";
    let user_2_id = "user-availability-untouched";

    collection
        .insert_many([
            build_settings_document(user_1_id, 10),
            build_settings_document(user_2_id, 20),
        ])
        .await
        .unwrap();

    let availability = AvailabilitySettings {
        configured: true,
        days: vec![
            AvailabilityDay {
                weekday: Weekday::Mon,
                available: true,
                max_duration_minutes: Some(60),
            },
            AvailabilityDay {
                weekday: Weekday::Tue,
                available: false,
                max_duration_minutes: None,
            },
            AvailabilityDay {
                weekday: Weekday::Wed,
                available: true,
                max_duration_minutes: Some(90),
            },
            AvailabilityDay {
                weekday: Weekday::Thu,
                available: false,
                max_duration_minutes: None,
            },
            AvailabilityDay {
                weekday: Weekday::Fri,
                available: true,
                max_duration_minutes: Some(120),
            },
            AvailabilityDay {
                weekday: Weekday::Sat,
                available: false,
                max_duration_minutes: None,
            },
            AvailabilityDay {
                weekday: Weekday::Sun,
                available: false,
                max_duration_minutes: None,
            },
        ],
    };
    let updated_at = 123_456;

    repository
        .update_availability(user_1_id, availability.clone(), updated_at)
        .await
        .unwrap();

    let updated = collection
        .find_one(doc! { "user_id": user_1_id })
        .await
        .unwrap()
        .unwrap();
    let untouched = collection
        .find_one(doc! { "user_id": user_2_id })
        .await
        .unwrap()
        .unwrap();

    let expected_availability = map_domain_availability_to_document(&availability);

    assert_eq!(
        updated.availability.configured,
        expected_availability.configured
    );
    assert_eq!(
        updated.availability.days.len(),
        expected_availability.days.len()
    );
    assert_eq!(
        updated.availability.days[0].weekday,
        expected_availability.days[0].weekday
    );
    assert_eq!(
        updated.availability.days[0].available,
        expected_availability.days[0].available
    );
    assert_eq!(
        updated.availability.days[0].max_duration_minutes,
        expected_availability.days[0].max_duration_minutes
    );
    assert_eq!(
        updated.availability.days[2].weekday,
        expected_availability.days[2].weekday
    );
    assert_eq!(
        updated.availability.days[2].available,
        expected_availability.days[2].available
    );
    assert_eq!(
        updated.availability.days[2].max_duration_minutes,
        expected_availability.days[2].max_duration_minutes
    );
    assert_eq!(updated.updated_at_epoch_seconds, Some(updated_at));

    let default_availability = default_availability_document();

    assert_eq!(untouched.user_id, user_2_id);
    assert_eq!(untouched.updated_at_epoch_seconds, Some(20));
    assert_eq!(
        untouched.availability.configured,
        default_availability.configured
    );
    assert_eq!(
        untouched.availability.days.len(),
        default_availability.days.len()
    );
    assert!(untouched.availability.days.iter().all(|day| !day.available));
    assert_eq!(untouched.availability.days[0].weekday, "mon");
    assert_eq!(untouched.availability.days[6].weekday, "sun");

    client.database(&database_name).drop().await.unwrap();
}

fn build_settings_document(user_id: &str, updated_at_epoch_seconds: i64) -> SettingsDocument {
    SettingsDocument {
        id: Some(ObjectId::new()),
        user_id: user_id.to_string(),
        ai_agents: AiAgentsDocument::default(),
        intervals: IntervalsDocument::default(),
        wahoo: WahooDocument::default(),
        options: OptionsDocument::default(),
        availability: default_availability_document(),
        cycling: CyclingDocument::default(),
        created_at_epoch_seconds: Some(1),
        created_at: None,
        updated_at_epoch_seconds: Some(updated_at_epoch_seconds),
        updated_at: None,
    }
}

async fn test_mongo_client_or_skip() -> Option<Client> {
    let mongo_uri = test_mongo_uri();
    let mut options = match mongodb::options::ClientOptions::parse(&mongo_uri).await {
        Ok(options) => options,
        Err(error) => {
            if std::env::var("REQUIRE_MONGO_IN_CI").as_deref() == Ok("true") {
                panic!("mongo settings test requires Mongo in CI: {error}");
            }
            eprintln!("skipping mongo settings test: failed to parse client options for {mongo_uri}: {error}");
            return None;
        }
    };
    options.server_selection_timeout = Some(std::time::Duration::from_secs(1));
    let client = match Client::with_options(options) {
        Ok(client) => client,
        Err(error) => {
            if std::env::var("REQUIRE_MONGO_IN_CI").as_deref() == Ok("true") {
                panic!("mongo settings test requires Mongo in CI: {error}");
            }
            eprintln!(
                "skipping mongo settings test: failed to create client for {mongo_uri}: {error}"
            );
            return None;
        }
    };

    match client
        .database("admin")
        .run_command(doc! { "ping": 1 })
        .await
    {
        Ok(_) => Some(client),
        Err(error) => {
            if std::env::var("REQUIRE_MONGO_IN_CI").as_deref() == Ok("true") {
                panic!("mongo settings test requires Mongo in CI: {error}");
            }
            eprintln!(
                "skipping mongo settings test: failed to connect to Mongo at {mongo_uri}: {error}"
            );
            None
        }
    }
}

fn test_mongo_uri() -> String {
    std::env::var("MONGODB_URI")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "mongodb://localhost:27017".to_string())
}

fn unique_test_database_name(prefix: &str) -> String {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{prefix}-{unique}")
}

#[tokio::test]
async fn ensure_indexes_creates_unique_wahoo_user_id_index() {
    let Some(client) = test_mongo_client_or_skip().await else {
        return;
    };
    let database_name = unique_test_database_name("mongo-settings-wahoo-user-id-index");
    let repository = MongoUserSettingsRepository::new(client.clone(), &database_name);

    repository.ensure_indexes().await.unwrap();

    let index_names = client
        .database(&database_name)
        .collection::<mongodb::bson::Document>("user_settings")
        .list_index_names()
        .await
        .unwrap();
    assert!(index_names.contains(&"user_settings_wahoo_user_id_unique".to_string()));

    client.database(&database_name).drop().await.unwrap();
}

#[tokio::test]
async fn find_by_wahoo_user_id_rejects_duplicate_mappings() {
    let Some(client) = test_mongo_client_or_skip().await else {
        return;
    };
    let database_name = unique_test_database_name("mongo-settings-wahoo-user-id-duplicates");
    let repository = MongoUserSettingsRepository::new(client.clone(), &database_name);
    let collection = client
        .database(&database_name)
        .collection::<SettingsDocument>("user_settings");

    collection
        .insert_many([
            SettingsDocument {
                wahoo: WahooDocument {
                    user_id: Some(60_462),
                    refresh_token: Some("refresh-1".to_string()),
                    connected: true,
                    ..WahooDocument::default()
                },
                ..build_settings_document("user-1", 10)
            },
            SettingsDocument {
                wahoo: WahooDocument {
                    user_id: Some(60_462),
                    refresh_token: Some("refresh-2".to_string()),
                    connected: true,
                    ..WahooDocument::default()
                },
                ..build_settings_document("user-2", 20)
            },
        ])
        .await
        .unwrap();

    let error = repository
        .find_by_wahoo_user_id(60_462)
        .await
        .expect_err("duplicate wahoo user id should be rejected");
    assert!(error
        .to_string()
        .contains("multiple users are mapped to Wahoo user id 60462"));

    client.database(&database_name).drop().await.unwrap();
}

#[tokio::test]
async fn backfill_wahoo_user_id_updates_only_wahoo_fields() {
    let Some(client) = test_mongo_client_or_skip().await else {
        return;
    };
    let database_name = unique_test_database_name("mongo-settings-wahoo-user-id-backfill");
    let repository = MongoUserSettingsRepository::new(client.clone(), &database_name);
    let collection = client
        .database(&database_name)
        .collection::<Document>("user_settings");

    collection
        .insert_one(doc! {
            "user_id": "user-1",
            "ai_agents": {},
            "intervals": {
                "api_key": "api-key",
                "athlete_id": "athlete-1",
                "connected": true,
                "updated_at_epoch_seconds": 123,
            },
            "wahoo": {
                "refresh_token": "refresh-token",
                "connected": true,
            },
            "options": {},
            "availability": { "configured": false, "days": [] },
            "cycling": {},
            "created_at_epoch_seconds": 1,
            "updated_at_epoch_seconds": 2,
        })
        .await
        .unwrap();

    repository
        .backfill_wahoo_user_id("user-1", 60_462, 1_700_000_000)
        .await
        .unwrap();

    let updated = collection
        .find_one(doc! { "user_id": "user-1" })
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        updated.get("updated_at_epoch_seconds").unwrap().as_i32(),
        Some(2)
    );
    assert_eq!(
        updated
            .get_document("intervals")
            .unwrap()
            .get("updated_at_epoch_seconds")
            .unwrap()
            .as_i32(),
        Some(123)
    );
    assert_eq!(
        updated
            .get_document("wahoo")
            .unwrap()
            .get_i64("user_id")
            .unwrap(),
        60_462
    );
    assert_eq!(
        updated
            .get_document("wahoo")
            .unwrap()
            .get_i64("updated_at_epoch_seconds")
            .unwrap(),
        1_700_000_000
    );

    client.database(&database_name).drop().await.unwrap();
}
