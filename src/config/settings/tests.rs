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
fn wahoo_oauth_settings_are_optional_by_default() {
    let settings = Settings::from_map(&base_values()).expect("settings should parse");

    assert!(settings.auth.wahoo.is_none());
}

#[test]
fn wahoo_oauth_settings_parse_when_all_values_are_present() {
    let mut values = base_values();
    values.insert(
        "WAHOO_OAUTH_CLIENT_ID".to_string(),
        "wahoo-client-id".to_string(),
    );
    values.insert(
        "WAHOO_OAUTH_CLIENT_SECRET".to_string(),
        "wahoo-client-secret".to_string(),
    );
    values.insert(
        "WAHOO_OAUTH_REDIRECT_URL".to_string(),
        "http://localhost:3002/api/wahoo/callback".to_string(),
    );

    let settings = Settings::from_map(&values).expect("settings should parse");
    let wahoo = settings.auth.wahoo.expect("wahoo settings should exist");

    assert_eq!(wahoo.client_id, "wahoo-client-id");
    assert_eq!(wahoo.client_secret, "wahoo-client-secret");
    assert_eq!(
        wahoo.redirect_url,
        "http://localhost:3002/api/wahoo/callback"
    );
    assert_eq!(
        wahoo.authorize_url,
        "https://api.wahooligan.com/oauth/authorize"
    );
    assert_eq!(wahoo.token_url, "https://api.wahooligan.com/oauth/token");
    assert_eq!(
        wahoo.scope,
        "email user_read user_write power_zones_read power_zones_write workouts_read workouts_write plans_read plans_write routes_read routes_write offline_data"
    );
    assert_eq!(wahoo.webhook_token, None);
}

#[test]
fn wahoo_oauth_settings_allow_optional_endpoint_overrides() {
    let mut values = base_values();
    values.insert(
        "WAHOO_OAUTH_CLIENT_ID".to_string(),
        "wahoo-client-id".to_string(),
    );
    values.insert(
        "WAHOO_OAUTH_CLIENT_SECRET".to_string(),
        "wahoo-client-secret".to_string(),
    );
    values.insert(
        "WAHOO_OAUTH_REDIRECT_URL".to_string(),
        "http://localhost:3002/api/wahoo/callback".to_string(),
    );
    values.insert(
        "WAHOO_OAUTH_AUTHORIZE_URL".to_string(),
        "https://example.test/oauth/authorize".to_string(),
    );
    values.insert(
        "WAHOO_OAUTH_TOKEN_URL".to_string(),
        "https://example.test/oauth/token".to_string(),
    );
    values.insert(
        "WAHOO_OAUTH_SCOPE".to_string(),
        "email offline_data custom_scope".to_string(),
    );
    values.insert(
        "WAHOO_WEBHOOK_TOKEN".to_string(),
        "secret-token".to_string(),
    );

    let settings = Settings::from_map(&values).expect("settings should parse");
    let wahoo = settings.auth.wahoo.expect("wahoo settings should exist");

    assert_eq!(wahoo.authorize_url, "https://example.test/oauth/authorize");
    assert_eq!(wahoo.token_url, "https://example.test/oauth/token");
    assert_eq!(wahoo.scope, "email offline_data custom_scope");
    assert_eq!(wahoo.webhook_token.as_deref(), Some("secret-token"));
}

#[test]
fn wahoo_oauth_settings_require_all_values_together() {
    let mut values = base_values();
    values.insert(
        "WAHOO_OAUTH_CLIENT_ID".to_string(),
        "wahoo-client-id".to_string(),
    );

    let error = Settings::from_map(&values).expect_err("settings should fail");

    assert_eq!(
        error.to_string(),
        "WAHOO_OAUTH_CLIENT_ID, WAHOO_OAUTH_CLIENT_SECRET, and WAHOO_OAUTH_REDIRECT_URL must be set together"
    );
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

#[test]
fn dev_llm_coach_defaults_to_disabled() {
    let settings = Settings::from_map(&base_values()).expect("settings should parse");

    assert!(!settings.dev_llm_coach_enabled);
}

#[test]
fn dev_llm_coach_can_be_enabled_explicitly() {
    let mut values = base_values();
    values.insert("DEV_LLM_COACH_ENABLED".to_string(), "true".to_string());

    let settings = Settings::from_map(&values).expect("settings should parse");

    assert!(settings.dev_llm_coach_enabled);
}

#[test]
fn legacy_time_stream_cleanup_defaults_to_disabled() {
    let settings = Settings::from_map(&base_values()).expect("settings should parse");

    assert!(!settings.legacy_time_stream_cleanup_enabled);
}

#[test]
fn legacy_time_stream_cleanup_can_be_enabled_explicitly() {
    let mut values = base_values();
    values.insert(
        "ENABLE_LEGACY_TIME_STREAM_CLEANUP".to_string(),
        "true".to_string(),
    );

    let settings = Settings::from_map(&values).expect("settings should parse");

    assert!(settings.legacy_time_stream_cleanup_enabled);
}
