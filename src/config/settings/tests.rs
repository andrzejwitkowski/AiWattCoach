use std::collections::BTreeMap;

use super::Settings;

fn base_values() -> BTreeMap<String, String> {
    BTreeMap::from([
        ("APP_NAME".to_string(), "AiWattCoach".to_string()),
        ("SERVER_HOST".to_string(), "127.0.0.1".to_string()),
        ("SERVER_PORT".to_string(), "3002".to_string()),
        (
            "MONGODB_URI".to_string(),
            "mongodb://localhost:27017".to_string(),
        ),
        ("MONGODB_DATABASE".to_string(), "aiwattcoach".to_string()),
        (
            "GOOGLE_OAUTH_CLIENT_ID".to_string(),
            "client-id.apps.googleusercontent.com".to_string(),
        ),
        (
            "GOOGLE_OAUTH_CLIENT_SECRET".to_string(),
            "super-secret".to_string(),
        ),
        (
            "GOOGLE_OAUTH_REDIRECT_URL".to_string(),
            "http://localhost:3002/api/auth/google/callback".to_string(),
        ),
        (
            "SESSION_COOKIE_NAME".to_string(),
            "aiwattcoach_session".to_string(),
        ),
        ("SESSION_COOKIE_SAME_SITE".to_string(), "lax".to_string()),
        ("SESSION_TTL_HOURS".to_string(), "24".to_string()),
        ("SESSION_COOKIE_SECURE".to_string(), "false".to_string()),
        ("ADMIN_EMAILS".to_string(), "".to_string()),
    ])
}

#[test]
fn client_log_ingestion_defaults_to_disabled() {
    let settings = Settings::from_map(&base_values()).expect("settings should parse");

    assert!(!settings.client_log_ingestion_enabled);
}

#[test]
fn client_log_ingestion_can_be_enabled_explicitly() {
    let mut values = base_values();
    values.insert(
        "ENABLE_CLIENT_LOG_INGESTION".to_string(),
        "true".to_string(),
    );

    let settings = Settings::from_map(&values).expect("settings should parse");

    assert!(settings.client_log_ingestion_enabled);
}

#[test]
fn dev_auth_can_supply_google_oauth_defaults() {
    let mut values = base_values();
    values.remove("GOOGLE_OAUTH_CLIENT_ID");
    values.remove("GOOGLE_OAUTH_CLIENT_SECRET");
    values.remove("GOOGLE_OAUTH_REDIRECT_URL");
    values.insert("DEV_AUTH_ENABLED".to_string(), "true".to_string());

    let settings = Settings::from_map(&values).expect("settings should parse");

    assert!(settings.auth.dev.enabled);
    assert_eq!(settings.auth.google.client_id, "dev-google-client-id");
    assert_eq!(settings.auth.dev.email, "dev@aiwattcoach.local");
}

#[test]
fn dev_intervals_can_be_enabled_explicitly() {
    let mut values = base_values();
    values.insert("DEV_INTERVALS_ENABLED".to_string(), "true".to_string());

    let settings = Settings::from_map(&values).expect("settings should parse");

    assert!(settings.dev_intervals_enabled);
}
