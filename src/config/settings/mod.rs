mod error;
mod parse;
mod types;

pub use types::{AuthSettings, MongoSettings, ServerSettings, Settings};

use std::{collections::BTreeMap, env, io::ErrorKind};

use error::SettingsError;
use parse::{parse_admin_emails, parse_dev_auth_settings, parse_google_oauth_settings};
use types::{SessionSettings, SettingsParts};

impl Settings {
    pub fn from_env() -> Result<Self, SettingsError> {
        match dotenvy::dotenv() {
            Ok(_) => {}
            Err(dotenvy::Error::Io(error)) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => {
                return Err(SettingsError::new(format!(
                    "Failed to load .env configuration: {error}"
                )))
            }
        }

        Self::from_map(&load_env_values()?)
    }

    pub fn from_map(values: &BTreeMap<String, String>) -> Result<Self, SettingsError> {
        let dev_auth = parse_dev_auth_settings(values)?;
        let parts = SettingsParts::parse(values, dev_auth)?;

        Ok(Self {
            app_name: parts.app_name,
            server: parts.server,
            mongo: parts.mongo,
            auth: AuthSettings {
                google: parse_google_oauth_settings(values, parts.auth.dev.enabled)?,
                dev: parts.auth.dev,
                session: SessionSettings::parse(values)?,
                admin_emails: parse_admin_emails(values.get("ADMIN_EMAILS")),
            },
            dev_intervals_enabled: parse::optional_bool_setting(
                values.get("DEV_INTERVALS_ENABLED"),
                "DEV_INTERVALS_ENABLED",
                false,
            )?,
            client_log_ingestion_enabled: parse::optional_bool_setting(
                values.get("ENABLE_CLIENT_LOG_INGESTION"),
                "ENABLE_CLIENT_LOG_INGESTION",
                false,
            )?,
            legacy_time_stream_cleanup_enabled: parse::optional_bool_setting(
                values.get("ENABLE_LEGACY_TIME_STREAM_CLEANUP"),
                "ENABLE_LEGACY_TIME_STREAM_CLEANUP",
                false,
            )?,
        })
        .and_then(parse::validate_session_cookie_settings)
    }

    pub fn test_defaults() -> Self {
        Self {
            app_name: "AiWattCoach".to_string(),
            server: types::ServerSettings::test_defaults(),
            mongo: types::MongoSettings::test_defaults(),
            auth: AuthSettings::test_defaults(),
            dev_intervals_enabled: false,
            client_log_ingestion_enabled: false,
            legacy_time_stream_cleanup_enabled: false,
        }
    }
}

fn load_env_values() -> Result<BTreeMap<String, String>, SettingsError> {
    const KEYS: [&str; 21] = [
        "APP_NAME",
        "SERVER_HOST",
        "SERVER_PORT",
        "MONGODB_URI",
        "MONGODB_DATABASE",
        "GOOGLE_OAUTH_CLIENT_ID",
        "GOOGLE_OAUTH_CLIENT_SECRET",
        "GOOGLE_OAUTH_REDIRECT_URL",
        "DEV_AUTH_ENABLED",
        "DEV_AUTH_GOOGLE_SUBJECT",
        "DEV_AUTH_EMAIL",
        "DEV_AUTH_DISPLAY_NAME",
        "DEV_AUTH_AVATAR_URL",
        "DEV_INTERVALS_ENABLED",
        "SESSION_COOKIE_NAME",
        "SESSION_COOKIE_SAME_SITE",
        "SESSION_TTL_HOURS",
        "SESSION_COOKIE_SECURE",
        "ADMIN_EMAILS",
        "ENABLE_CLIENT_LOG_INGESTION",
        "ENABLE_LEGACY_TIME_STREAM_CLEANUP",
    ];

    let mut values = BTreeMap::new();

    for key in KEYS {
        match env::var(key) {
            Ok(value) => {
                values.insert(key.to_string(), value);
            }
            Err(env::VarError::NotPresent) => {}
            Err(env::VarError::NotUnicode(_)) => {
                return Err(SettingsError::new(format!(
                    "Environment variable {key} is not valid UTF-8"
                )));
            }
        }
    }

    Ok(values)
}

#[cfg(test)]
mod tests;
