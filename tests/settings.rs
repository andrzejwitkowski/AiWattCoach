use std::collections::BTreeMap;

#[cfg(unix)]
use std::{
    env,
    ffi::OsString,
    os::unix::ffi::OsStringExt,
    sync::{Mutex, OnceLock},
};

use aiwattcoach::Settings;

fn required_settings_map() -> BTreeMap<String, String> {
    BTreeMap::from([
        ("APP_NAME".to_string(), "AiWattCoach".to_string()),
        ("SERVER_HOST".to_string(), "127.0.0.1".to_string()),
        ("SERVER_PORT".to_string(), "3000".to_string()),
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
    ])
}

#[test]
fn settings_load_required_values_from_map() {
    let settings = Settings::from_map(&required_settings_map()).unwrap();

    assert_eq!(settings.app_name, "AiWattCoach");
    assert_eq!(settings.server.host, "127.0.0.1");
    assert_eq!(settings.server.port, 3000);
    assert_eq!(settings.mongo.uri, "mongodb://localhost:27017");
    assert_eq!(settings.mongo.database, "aiwattcoach");
    assert_eq!(
        settings.auth.google.client_id,
        "client-id.apps.googleusercontent.com"
    );
    assert_eq!(
        settings.auth.google.redirect_url,
        "http://localhost:3002/api/auth/google/callback"
    );
    assert_eq!(settings.auth.session.cookie_name, "aiwattcoach_session");
    assert_eq!(settings.auth.session.same_site, "lax");
    assert_eq!(settings.auth.session.ttl_hours, 24);
    assert!(!settings.auth.session.secure);
    assert!(settings.auth.admin_emails.is_empty());
    assert!(!settings.auth.dev.enabled);
    assert!(!settings.dev_intervals_enabled);
    assert!(!settings.dev_llm_coach_enabled);
    assert!(!settings.trust_proxy_headers);
}

#[test]
fn settings_reject_invalid_session_cookie_same_site_value() {
    let mut values = required_settings_map();
    values.insert(
        "SESSION_COOKIE_SAME_SITE".to_string(),
        "strictly".to_string(),
    );

    let error = Settings::from_map(&values).unwrap_err();

    assert_eq!(
        error.to_string(),
        "SESSION_COOKIE_SAME_SITE must be lax, strict, or none"
    );
}

#[test]
fn settings_require_secure_cookie_when_same_site_is_none() {
    let mut values = required_settings_map();
    values.insert("SESSION_COOKIE_SAME_SITE".to_string(), "none".to_string());
    values.insert("SESSION_COOKIE_SECURE".to_string(), "false".to_string());

    let error = Settings::from_map(&values).unwrap_err();

    assert_eq!(
        error.to_string(),
        "SESSION_COOKIE_SECURE must be true when SESSION_COOKIE_SAME_SITE is none"
    );
}

#[test]
fn settings_allow_cross_site_cookie_when_secure_and_same_site_none() {
    let mut values = required_settings_map();
    values.insert("SESSION_COOKIE_SAME_SITE".to_string(), "none".to_string());
    values.insert("SESSION_COOKIE_SECURE".to_string(), "true".to_string());

    let settings = Settings::from_map(&values).unwrap();

    assert_eq!(settings.auth.session.same_site, "none");
    assert!(settings.auth.session.secure);
}

#[test]
fn settings_reject_empty_required_values() {
    let mut values = required_settings_map();
    values.insert("APP_NAME".to_string(), "   ".to_string());

    let error = Settings::from_map(&values).unwrap_err();

    assert_eq!(error.to_string(), "Setting APP_NAME must not be empty");
}

#[test]
fn settings_reject_invalid_server_port() {
    let mut values = required_settings_map();
    values.insert("SERVER_PORT".to_string(), "70000".to_string());

    let error = Settings::from_map(&values).unwrap_err();

    assert_eq!(error.to_string(), "SERVER_PORT must be a valid u16");
}

#[test]
fn settings_reject_invalid_session_ttl_hours() {
    let mut values = required_settings_map();
    values.insert("SESSION_TTL_HOURS".to_string(), "zero".to_string());

    let error = Settings::from_map(&values).unwrap_err();

    assert_eq!(error.to_string(), "SESSION_TTL_HOURS must be a valid u64");
}

#[test]
fn settings_reject_zero_session_ttl_hours() {
    let mut values = required_settings_map();
    values.insert("SESSION_TTL_HOURS".to_string(), "0".to_string());

    let error = Settings::from_map(&values).unwrap_err();

    assert_eq!(
        error.to_string(),
        "SESSION_TTL_HOURS must be greater than 0"
    );
}

#[test]
fn settings_reject_oversized_session_ttl_hours() {
    let mut values = required_settings_map();
    values.insert(
        "SESSION_TTL_HOURS".to_string(),
        (i64::MAX as u64 / 1000 / 3600 + 1).to_string(),
    );

    let error = Settings::from_map(&values).unwrap_err();

    assert_eq!(
        error.to_string(),
        "SESSION_TTL_HOURS exceeds supported range"
    );
}

#[test]
fn google_oauth_settings_debug_redacts_client_secret() {
    let settings = Settings::from_map(&required_settings_map()).unwrap();

    let debug = format!("{:?}", settings.auth.google);

    assert!(debug.contains("<redacted>"));
    assert!(!debug.contains("super-secret"));
}

#[test]
fn settings_reject_invalid_session_cookie_secure_value() {
    let mut values = required_settings_map();
    values.insert("SESSION_COOKIE_SECURE".to_string(), "yes".to_string());

    let error = Settings::from_map(&values).unwrap_err();

    assert_eq!(
        error.to_string(),
        "SESSION_COOKIE_SECURE must be true or false"
    );
}

#[test]
fn settings_reject_invalid_session_cookie_name() {
    let mut values = required_settings_map();
    values.insert("SESSION_COOKIE_NAME".to_string(), "bad name".to_string());

    let error = Settings::from_map(&values).unwrap_err();

    assert_eq!(
        error.to_string(),
        "SESSION_COOKIE_NAME must be a valid cookie token"
    );
}

#[test]
fn settings_require_google_client_id() {
    let mut values = required_settings_map();
    values.remove("GOOGLE_OAUTH_CLIENT_ID");

    let error = Settings::from_map(&values).unwrap_err();

    assert_eq!(
        error.to_string(),
        "Missing required setting: GOOGLE_OAUTH_CLIENT_ID"
    );
}

#[test]
fn settings_allow_dev_auth_without_google_oauth_credentials() {
    let mut values = required_settings_map();
    values.remove("GOOGLE_OAUTH_CLIENT_ID");
    values.remove("GOOGLE_OAUTH_CLIENT_SECRET");
    values.remove("GOOGLE_OAUTH_REDIRECT_URL");
    values.insert("DEV_AUTH_ENABLED".to_string(), "true".to_string());
    values.insert(
        "DEV_AUTH_EMAIL".to_string(),
        "coach@example.com".to_string(),
    );

    let settings = Settings::from_map(&values).unwrap();

    assert!(settings.auth.dev.enabled);
    assert_eq!(settings.auth.dev.email, "coach@example.com");
    assert_eq!(settings.auth.google.client_id, "dev-google-client-id");
}

#[test]
fn settings_allow_dev_intervals_toggle() {
    let mut values = required_settings_map();
    values.insert("DEV_INTERVALS_ENABLED".to_string(), "true".to_string());

    let settings = Settings::from_map(&values).unwrap();

    assert!(settings.dev_intervals_enabled);
}

#[test]
fn settings_allow_dev_llm_coach_toggle() {
    let mut values = required_settings_map();
    values.insert("DEV_LLM_COACH_ENABLED".to_string(), "true".to_string());

    let settings = Settings::from_map(&values).unwrap();

    assert!(settings.dev_llm_coach_enabled);
}

#[test]
fn settings_allow_trust_proxy_headers_toggle() {
    let mut values = required_settings_map();
    values.insert("TRUST_PROXY_HEADERS".to_string(), "true".to_string());

    let settings = Settings::from_map(&values).unwrap();

    assert!(settings.trust_proxy_headers);
}

#[test]
fn settings_trim_required_values() {
    let mut values = required_settings_map();
    values.insert("APP_NAME".to_string(), "  AiWattCoach  ".to_string());
    values.insert("SERVER_HOST".to_string(), " 127.0.0.1 ".to_string());
    values.insert(
        "MONGODB_URI".to_string(),
        " mongodb://localhost:27017 ".to_string(),
    );
    values.insert("MONGODB_DATABASE".to_string(), " aiwattcoach ".to_string());
    values.insert(
        "GOOGLE_OAUTH_CLIENT_SECRET".to_string(),
        "  super-secret  ".to_string(),
    );
    values.insert(
        "SESSION_COOKIE_NAME".to_string(),
        "  aiwattcoach_session  ".to_string(),
    );

    let settings = Settings::from_map(&values).unwrap();

    assert_eq!(settings.app_name, "AiWattCoach");
    assert_eq!(settings.server.host, "127.0.0.1");
    assert_eq!(settings.mongo.uri, "mongodb://localhost:27017");
    assert_eq!(settings.mongo.database, "aiwattcoach");
    assert_eq!(settings.auth.google.client_secret, "super-secret");
    assert_eq!(settings.auth.session.cookie_name, "aiwattcoach_session");
    assert!(!settings.auth.session.secure);
}

#[test]
fn settings_load_custom_server_port_from_map() {
    let mut values = required_settings_map();
    values.insert("APP_NAME".to_string(), "AiWattCoach TEST".to_string());
    values.insert("SERVER_HOST".to_string(), "0.0.0.0".to_string());
    values.insert("SERVER_PORT".to_string(), "3002".to_string());
    values.insert(
        "MONGODB_URI".to_string(),
        "mongodb://mongodb-sandbox:27017/?directConnection=true".to_string(),
    );
    values.insert("MONGODB_DATABASE".to_string(), "default".to_string());

    let settings = Settings::from_map(&values).unwrap();

    assert_eq!(settings.server.port, 3002);
    assert_eq!(settings.server.address(), "0.0.0.0:3002");
}

#[test]
fn settings_parse_and_normalize_admin_emails() {
    let mut values = required_settings_map();
    values.insert(
        "ADMIN_EMAILS".to_string(),
        " Admin@One.com,admin@two.com ,, ADMIN@THREE.COM ".to_string(),
    );

    let settings = Settings::from_map(&values).unwrap();

    assert_eq!(
        settings.auth.admin_emails,
        vec![
            "admin@one.com".to_string(),
            "admin@two.com".to_string(),
            "admin@three.com".to_string()
        ]
    );
}

#[test]
fn test_defaults_keep_local_runtime_on_port_3002() {
    let settings = Settings::test_defaults();

    assert_eq!(settings.server.port, 3002);
    assert_eq!(settings.server.address(), "127.0.0.1:3002");
    assert_eq!(settings.auth.session.ttl_hours, 24);
    assert_eq!(settings.auth.session.cookie_name, "aiwattcoach_session");
    assert!(!settings.auth.session.secure);
}

#[test]
fn server_settings_wrap_ipv6_hosts_in_brackets() {
    let mut settings = Settings::test_defaults();
    settings.server.host = "::1".to_string();

    assert_eq!(settings.server.address(), "[::1]:3002");
}

#[cfg(unix)]
#[test]
fn settings_from_env_rejects_non_unicode_values() {
    let _guard = env_lock().lock().unwrap();

    let keys = [
        "APP_NAME",
        "SERVER_HOST",
        "SERVER_PORT",
        "MONGODB_URI",
        "MONGODB_DATABASE",
        "GOOGLE_OAUTH_CLIENT_ID",
        "GOOGLE_OAUTH_CLIENT_SECRET",
        "GOOGLE_OAUTH_REDIRECT_URL",
        "SESSION_COOKIE_NAME",
        "SESSION_TTL_HOURS",
        "SESSION_COOKIE_SECURE",
        "ADMIN_EMAILS",
    ];
    let original_values = keys
        .iter()
        .map(|key| (key, env::var_os(key)))
        .collect::<Vec<_>>();

    env::set_var("APP_NAME", "AiWattCoach");
    env::set_var("SERVER_HOST", "127.0.0.1");
    env::set_var("SERVER_PORT", "3000");
    env::set_var("MONGODB_DATABASE", "aiwattcoach");
    env::set_var(
        "GOOGLE_OAUTH_CLIENT_ID",
        "client-id.apps.googleusercontent.com",
    );
    env::set_var("GOOGLE_OAUTH_CLIENT_SECRET", "super-secret");
    env::set_var(
        "GOOGLE_OAUTH_REDIRECT_URL",
        "http://localhost:3002/api/auth/google/callback",
    );
    env::set_var("SESSION_COOKIE_NAME", "aiwattcoach_session");
    env::set_var("SESSION_TTL_HOURS", "24");
    env::set_var("SESSION_COOKIE_SECURE", "false");
    env::set_var("MONGODB_URI", OsString::from_vec(vec![0xFF]));

    let error = Settings::from_env().unwrap_err();

    for (key, value) in original_values {
        match value {
            Some(value) => env::set_var(key, value),
            None => env::remove_var(key),
        }
    }

    assert_eq!(
        error.to_string(),
        "Environment variable MONGODB_URI is not valid UTF-8"
    );
}

#[cfg(unix)]
fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}
