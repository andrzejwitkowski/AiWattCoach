use std::time::{SystemTime, UNIX_EPOCH};

use mongodb::{
    bson::{doc, oid::ObjectId, DateTime},
    Client,
};

use super::super::documents::{
    default_availability_document, AiAgentsDocument, CyclingDocument, IntervalsDocument,
    OptionsDocument, SettingsDocument, WahooDocument,
};

pub(super) fn build_settings_document(
    user_id: &str,
    updated_at_epoch_seconds: i64,
) -> SettingsDocument {
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

pub(super) async fn test_mongo_client_or_skip() -> Option<Client> {
    let mongo_uri = test_mongo_uri();
    let mut options = match mongodb::options::ClientOptions::parse(&mongo_uri).await {
        Ok(options) => options,
        Err(error) => {
            if std::env::var("REQUIRE_MONGO_IN_CI").as_deref() == Ok("true") {
                panic!("mongo settings test requires Mongo in CI: {error}");
            }
            eprintln!(
                "skipping mongo settings test: failed to parse client options for {mongo_uri}: {error}"
            );
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

pub(super) fn unique_test_database_name(prefix: &str) -> String {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{prefix}-{unique}")
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
