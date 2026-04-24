use std::collections::BTreeMap;

use crate::domain::identity::MAX_BSON_EPOCH_SECONDS;

use super::{
    error::SettingsError,
    types::{DevAuthSettings, GoogleOAuthSettings, Settings, WahooOAuthSettings},
};

pub(super) fn required(
    values: &BTreeMap<String, String>,
    key: &str,
) -> Result<String, SettingsError> {
    let value = values
        .get(key)
        .cloned()
        .ok_or_else(|| SettingsError::new(format!("Missing required setting: {key}")))?;

    let trimmed = value.trim();

    if trimmed.is_empty() {
        return Err(SettingsError::new(format!(
            "Setting {key} must not be empty"
        )));
    }

    Ok(trimmed.to_string())
}

pub(super) fn optional_string_setting(
    values: &BTreeMap<String, String>,
    key: &str,
) -> Option<String> {
    values.get(key).and_then(|value| {
        let trimmed = value.trim();

        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

pub(super) fn parse_google_oauth_settings(
    values: &BTreeMap<String, String>,
    dev_auth_enabled: bool,
) -> Result<GoogleOAuthSettings, SettingsError> {
    if dev_auth_enabled {
        return Ok(GoogleOAuthSettings {
            client_id: optional_string_setting(values, "GOOGLE_OAUTH_CLIENT_ID")
                .unwrap_or_else(|| "dev-google-client-id".to_string()),
            client_secret: optional_string_setting(values, "GOOGLE_OAUTH_CLIENT_SECRET")
                .unwrap_or_else(|| "dev-google-client-secret".to_string()),
            redirect_url: optional_string_setting(values, "GOOGLE_OAUTH_REDIRECT_URL")
                .unwrap_or_else(|| "http://localhost:3002/api/auth/google/callback".to_string()),
        });
    }

    Ok(GoogleOAuthSettings {
        client_id: required(values, "GOOGLE_OAUTH_CLIENT_ID")?,
        client_secret: required(values, "GOOGLE_OAUTH_CLIENT_SECRET")?,
        redirect_url: required(values, "GOOGLE_OAUTH_REDIRECT_URL")?,
    })
}

pub(super) fn parse_wahoo_oauth_settings(
    values: &BTreeMap<String, String>,
) -> Result<Option<WahooOAuthSettings>, SettingsError> {
    const DEFAULT_WAHOO_AUTHORIZE_URL: &str = "https://api.wahooligan.com/oauth/authorize";
    const DEFAULT_WAHOO_TOKEN_URL: &str = "https://api.wahooligan.com/oauth/token";
    const DEFAULT_WAHOO_SCOPE: &str = "email user_read user_write power_zones_read power_zones_write workouts_read workouts_write plans_read plans_write routes_read routes_write offline_data";

    let client_id = optional_string_setting(values, "WAHOO_OAUTH_CLIENT_ID");
    let client_secret = optional_string_setting(values, "WAHOO_OAUTH_CLIENT_SECRET");
    let redirect_url = optional_string_setting(values, "WAHOO_OAUTH_REDIRECT_URL");
    let authorize_url = optional_string_setting(values, "WAHOO_OAUTH_AUTHORIZE_URL")
        .unwrap_or_else(|| DEFAULT_WAHOO_AUTHORIZE_URL.to_string());
    let token_url = optional_string_setting(values, "WAHOO_OAUTH_TOKEN_URL")
        .unwrap_or_else(|| DEFAULT_WAHOO_TOKEN_URL.to_string());
    let scope = optional_string_setting(values, "WAHOO_OAUTH_SCOPE")
        .unwrap_or_else(|| DEFAULT_WAHOO_SCOPE.to_string());

    match (client_id, client_secret, redirect_url) {
        (None, None, None) => Ok(None),
        (Some(client_id), Some(client_secret), Some(redirect_url)) => Ok(Some(WahooOAuthSettings {
            client_id,
            client_secret,
            redirect_url,
            authorize_url,
            token_url,
            scope,
        })),
        _ => Err(SettingsError::new(
            "WAHOO_OAUTH_CLIENT_ID, WAHOO_OAUTH_CLIENT_SECRET, and WAHOO_OAUTH_REDIRECT_URL must be set together",
        )),
    }
}

pub(super) fn parse_dev_auth_settings(
    values: &BTreeMap<String, String>,
) -> Result<DevAuthSettings, SettingsError> {
    Ok(DevAuthSettings {
        enabled: optional_bool_setting(values.get("DEV_AUTH_ENABLED"), "DEV_AUTH_ENABLED", false)?,
        google_subject: optional_string_setting(values, "DEV_AUTH_GOOGLE_SUBJECT")
            .unwrap_or_else(|| "dev-google-subject".to_string()),
        email: optional_string_setting(values, "DEV_AUTH_EMAIL")
            .unwrap_or_else(|| "dev@aiwattcoach.local".to_string()),
        display_name: optional_string_setting(values, "DEV_AUTH_DISPLAY_NAME")
            .unwrap_or_else(|| "Dev Athlete".to_string()),
        avatar_url: optional_string_setting(values, "DEV_AUTH_AVATAR_URL"),
    })
}

pub(super) fn parse_admin_emails(raw_value: Option<&String>) -> Vec<String> {
    raw_value
        .map(|value| {
            value
                .split(',')
                .filter_map(|email| {
                    let normalized = email.trim().to_ascii_lowercase();

                    if normalized.is_empty() {
                        None
                    } else {
                        Some(normalized)
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn parse_session_ttl_hours(raw_value: &str) -> Result<u64, SettingsError> {
    const MAX_SESSION_TTL_HOURS: u64 = MAX_BSON_EPOCH_SECONDS as u64 / 3600;

    let ttl_hours = raw_value
        .parse()
        .map_err(|_| SettingsError::new("SESSION_TTL_HOURS must be a valid u64"))?;

    if ttl_hours == 0 {
        return Err(SettingsError::new(
            "SESSION_TTL_HOURS must be greater than 0",
        ));
    }

    if ttl_hours > MAX_SESSION_TTL_HOURS {
        return Err(SettingsError::new(
            "SESSION_TTL_HOURS exceeds supported range",
        ));
    }

    Ok(ttl_hours)
}

pub(super) fn parse_bool_setting(raw_value: &str, key: &str) -> Result<bool, SettingsError> {
    match raw_value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(SettingsError::new(format!("{key} must be true or false"))),
    }
}

pub(super) fn optional_bool_setting(
    raw_value: Option<&String>,
    key: &str,
    default: bool,
) -> Result<bool, SettingsError> {
    match raw_value {
        Some(value) => parse_bool_setting(value.trim(), key),
        None => Ok(default),
    }
}

pub(super) fn parse_same_site_setting(raw_value: &str) -> Result<String, SettingsError> {
    let normalized = raw_value.trim().to_ascii_lowercase();

    match normalized.as_str() {
        "lax" | "strict" | "none" => Ok(normalized),
        _ => Err(SettingsError::new(
            "SESSION_COOKIE_SAME_SITE must be lax, strict, or none",
        )),
    }
}

pub(super) fn validate_session_cookie_settings(
    settings: Settings,
) -> Result<Settings, SettingsError> {
    if settings.auth.session.same_site == "none" && !settings.auth.session.secure {
        return Err(SettingsError::new(
            "SESSION_COOKIE_SECURE must be true when SESSION_COOKIE_SAME_SITE is none",
        ));
    }

    Ok(settings)
}

pub(super) fn parse_cookie_name(raw_value: &str) -> Result<String, SettingsError> {
    let is_valid = raw_value.bytes().all(|byte| {
        matches!(
            byte,
            b'!' | b'#'
                | b'$'
                | b'%'
                | b'&'
                | b'\''
                | b'*'
                | b'+'
                | b'-'
                | b'.'
                | b'^'
                | b'_'
                | b'`'
                | b'|'
                | b'~'
        ) || byte.is_ascii_alphanumeric()
    });

    if is_valid {
        Ok(raw_value.to_string())
    } else {
        Err(SettingsError::new(
            "SESSION_COOKIE_NAME must be a valid cookie token",
        ))
    }
}
