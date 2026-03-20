use std::collections::BTreeMap;

#[cfg(unix)]
use std::{
    env,
    ffi::OsString,
    os::unix::ffi::OsStringExt,
    sync::{Mutex, OnceLock},
};

use aiwattcoach::Settings;

#[test]
fn settings_load_required_values_from_map() {
    let settings = Settings::from_map(&BTreeMap::from([
        ("APP_NAME".to_string(), "AiWattCoach".to_string()),
        ("SERVER_HOST".to_string(), "127.0.0.1".to_string()),
        ("SERVER_PORT".to_string(), "3000".to_string()),
        (
            "MONGODB_URI".to_string(),
            "mongodb://localhost:27017".to_string(),
        ),
        ("MONGODB_DATABASE".to_string(), "aiwattcoach".to_string()),
    ]))
    .unwrap();

    assert_eq!(settings.app_name, "AiWattCoach");
    assert_eq!(settings.server.host, "127.0.0.1");
    assert_eq!(settings.server.port, 3000);
    assert_eq!(settings.mongo.uri, "mongodb://localhost:27017");
    assert_eq!(settings.mongo.database, "aiwattcoach");
}

#[test]
fn settings_reject_empty_required_values() {
    let error = Settings::from_map(&BTreeMap::from([
        ("APP_NAME".to_string(), "   ".to_string()),
        ("SERVER_HOST".to_string(), "127.0.0.1".to_string()),
        ("SERVER_PORT".to_string(), "3000".to_string()),
        (
            "MONGODB_URI".to_string(),
            "mongodb://localhost:27017".to_string(),
        ),
        ("MONGODB_DATABASE".to_string(), "aiwattcoach".to_string()),
    ]))
    .unwrap_err();

    assert_eq!(error.to_string(), "Setting APP_NAME must not be empty");
}

#[test]
fn settings_reject_invalid_server_port() {
    let error = Settings::from_map(&BTreeMap::from([
        ("APP_NAME".to_string(), "AiWattCoach".to_string()),
        ("SERVER_HOST".to_string(), "127.0.0.1".to_string()),
        ("SERVER_PORT".to_string(), "70000".to_string()),
        (
            "MONGODB_URI".to_string(),
            "mongodb://localhost:27017".to_string(),
        ),
        ("MONGODB_DATABASE".to_string(), "aiwattcoach".to_string()),
    ]))
    .unwrap_err();

    assert_eq!(error.to_string(), "SERVER_PORT must be a valid u16");
}

#[test]
fn settings_trim_required_values() {
    let settings = Settings::from_map(&BTreeMap::from([
        ("APP_NAME".to_string(), "  AiWattCoach  ".to_string()),
        ("SERVER_HOST".to_string(), " 127.0.0.1 ".to_string()),
        ("SERVER_PORT".to_string(), "3000".to_string()),
        (
            "MONGODB_URI".to_string(),
            " mongodb://localhost:27017 ".to_string(),
        ),
        ("MONGODB_DATABASE".to_string(), " aiwattcoach ".to_string()),
    ]))
    .unwrap();

    assert_eq!(settings.app_name, "AiWattCoach");
    assert_eq!(settings.server.host, "127.0.0.1");
    assert_eq!(settings.mongo.uri, "mongodb://localhost:27017");
    assert_eq!(settings.mongo.database, "aiwattcoach");
}

#[test]
fn server_settings_wrap_ipv6_hosts_in_brackets() {
    let mut settings = Settings::test_defaults();
    settings.server.host = "::1".to_string();

    assert_eq!(settings.server.address(), "[::1]:3000");
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
    ];
    let original_values = keys
        .iter()
        .map(|key| (key, env::var_os(key)))
        .collect::<Vec<_>>();

    env::set_var("APP_NAME", "AiWattCoach");
    env::set_var("SERVER_HOST", "127.0.0.1");
    env::set_var("SERVER_PORT", "3000");
    env::set_var("MONGODB_DATABASE", "aiwattcoach");
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
