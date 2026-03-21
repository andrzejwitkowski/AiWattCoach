use std::{collections::BTreeMap, env, error::Error, fmt, io::ErrorKind};

use crate::domain::identity::MAX_BSON_EPOCH_SECONDS;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub app_name: String,
    pub server: ServerSettings,
    pub mongo: MongoSettings,
    pub auth: AuthSettings,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerSettings {
    pub host: String,
    pub port: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MongoSettings {
    pub uri: String,
    pub database: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthSettings {
    pub google: GoogleOAuthSettings,
    pub session: SessionSettings,
    pub admin_emails: Vec<String>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct GoogleOAuthSettings {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_url: String,
}

impl fmt::Debug for GoogleOAuthSettings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GoogleOAuthSettings")
            .field("client_id", &self.client_id)
            .field("client_secret", &"<redacted>")
            .field("redirect_url", &self.redirect_url)
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionSettings {
    pub cookie_name: String,
    pub same_site: String,
    pub ttl_hours: u64,
    pub secure: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsError {
    message: String,
}

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
            "SESSION_COOKIE_SAME_SITE",
            "SESSION_TTL_HOURS",
            "SESSION_COOKIE_SECURE",
            "ADMIN_EMAILS",
        ];
        let mut values = BTreeMap::new();
        for key in keys {
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

        Self::from_map(&values)
    }

    pub fn from_map(values: &BTreeMap<String, String>) -> Result<Self, SettingsError> {
        Ok(Self {
            app_name: required(values, "APP_NAME")?,
            server: ServerSettings {
                host: required(values, "SERVER_HOST")?,
                port: required(values, "SERVER_PORT")?
                    .parse()
                    .map_err(|_| SettingsError::new("SERVER_PORT must be a valid u16"))?,
            },
            mongo: MongoSettings {
                uri: required(values, "MONGODB_URI")?,
                database: required(values, "MONGODB_DATABASE")?,
            },
            auth: AuthSettings {
                google: GoogleOAuthSettings {
                    client_id: required(values, "GOOGLE_OAUTH_CLIENT_ID")?,
                    client_secret: required(values, "GOOGLE_OAUTH_CLIENT_SECRET")?,
                    redirect_url: required(values, "GOOGLE_OAUTH_REDIRECT_URL")?,
                },
                session: SessionSettings {
                    cookie_name: parse_cookie_name(
                        required(values, "SESSION_COOKIE_NAME")?.as_str(),
                    )?,
                    same_site: parse_same_site_setting(
                        required(values, "SESSION_COOKIE_SAME_SITE")?.as_str(),
                    )?,
                    ttl_hours: parse_session_ttl_hours(
                        required(values, "SESSION_TTL_HOURS")?.as_str(),
                    )?,
                    secure: parse_bool_setting(
                        required(values, "SESSION_COOKIE_SECURE")?.as_str(),
                        "SESSION_COOKIE_SECURE",
                    )?,
                },
                admin_emails: parse_admin_emails(values.get("ADMIN_EMAILS")),
            },
        })
        .and_then(validate_session_cookie_settings)
    }

    pub fn test_defaults() -> Self {
        Self {
            app_name: "AiWattCoach".to_string(),
            server: ServerSettings {
                host: "127.0.0.1".to_string(),
                port: 3002,
            },
            mongo: MongoSettings {
                uri: "mongodb://localhost:27017".to_string(),
                database: "aiwattcoach".to_string(),
            },
            auth: AuthSettings {
                google: GoogleOAuthSettings {
                    client_id: "local-google-client-id".to_string(),
                    client_secret: "local-google-client-secret".to_string(),
                    redirect_url: "http://localhost:3002/api/auth/google/callback".to_string(),
                },
                session: SessionSettings {
                    cookie_name: "aiwattcoach_session".to_string(),
                    same_site: "lax".to_string(),
                    ttl_hours: 24,
                    secure: false,
                },
                admin_emails: Vec::new(),
            },
        }
    }
}

impl ServerSettings {
    pub fn address(&self) -> String {
        if self.host.contains(':') && !self.host.starts_with('[') {
            format!("[{}]:{}", self.host, self.port)
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }
}

impl SettingsError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for SettingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl Error for SettingsError {}

fn required(values: &BTreeMap<String, String>, key: &str) -> Result<String, SettingsError> {
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

fn parse_admin_emails(raw_value: Option<&String>) -> Vec<String> {
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

fn parse_session_ttl_hours(raw_value: &str) -> Result<u64, SettingsError> {
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

fn parse_bool_setting(raw_value: &str, key: &str) -> Result<bool, SettingsError> {
    match raw_value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(SettingsError::new(format!("{key} must be true or false"))),
    }
}

fn parse_same_site_setting(raw_value: &str) -> Result<String, SettingsError> {
    let normalized = raw_value.trim().to_ascii_lowercase();

    match normalized.as_str() {
        "lax" | "strict" | "none" => Ok(normalized),
        _ => Err(SettingsError::new(
            "SESSION_COOKIE_SAME_SITE must be lax, strict, or none",
        )),
    }
}

fn validate_session_cookie_settings(settings: Settings) -> Result<Settings, SettingsError> {
    if settings.auth.session.same_site == "none" && !settings.auth.session.secure {
        return Err(SettingsError::new(
            "SESSION_COOKIE_SECURE must be true when SESSION_COOKIE_SAME_SITE is none",
        ));
    }

    Ok(settings)
}

fn parse_cookie_name(raw_value: &str) -> Result<String, SettingsError> {
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
