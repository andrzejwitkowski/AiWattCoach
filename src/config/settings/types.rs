use std::{collections::BTreeMap, fmt};

use super::{
    error::SettingsError,
    parse::{parse_cookie_name, parse_same_site_setting, parse_session_ttl_hours, required},
};

#[derive(Clone, PartialEq, Eq)]
pub struct Settings {
    pub app_name: String,
    pub server: ServerSettings,
    pub mongo: MongoSettings,
    pub auth: AuthSettings,
    pub gemini_supervisor_webhook_token: Option<String>,
    pub dev_intervals_enabled: bool,
    pub dev_llm_coach_enabled: bool,
    pub client_log_ingestion_enabled: bool,
    pub legacy_time_stream_cleanup_enabled: bool,
    pub trust_proxy_headers: bool,
}

impl fmt::Debug for Settings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Settings")
            .field("app_name", &self.app_name)
            .field("server", &self.server)
            .field("mongo", &self.mongo)
            .field("auth", &self.auth)
            .field(
                "gemini_supervisor_webhook_token",
                &self
                    .gemini_supervisor_webhook_token
                    .as_ref()
                    .map(|_| "<redacted>"),
            )
            .field("dev_intervals_enabled", &self.dev_intervals_enabled)
            .field("dev_llm_coach_enabled", &self.dev_llm_coach_enabled)
            .field(
                "client_log_ingestion_enabled",
                &self.client_log_ingestion_enabled,
            )
            .field(
                "legacy_time_stream_cleanup_enabled",
                &self.legacy_time_stream_cleanup_enabled,
            )
            .field("trust_proxy_headers", &self.trust_proxy_headers)
            .finish()
    }
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
    pub wahoo: Option<WahooOAuthSettings>,
    pub dev: DevAuthSettings,
    pub session: SessionSettings,
    pub admin_emails: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DevAuthSettings {
    pub enabled: bool,
    pub google_subject: String,
    pub email: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct GoogleOAuthSettings {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_url: String,
}

#[derive(Clone, PartialEq, Eq)]
pub struct WahooOAuthSettings {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_url: String,
    pub authorize_url: String,
    pub token_url: String,
    pub scope: String,
    pub webhook_token: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionSettings {
    pub cookie_name: String,
    pub same_site: String,
    pub ttl_hours: u64,
    pub secure: bool,
}

pub(super) struct SettingsParts {
    pub(super) app_name: String,
    pub(super) server: ServerSettings,
    pub(super) mongo: MongoSettings,
    pub(super) auth: AuthSettingsParts,
}

pub(super) struct AuthSettingsParts {
    pub(super) dev: DevAuthSettings,
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

impl fmt::Debug for WahooOAuthSettings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WahooOAuthSettings")
            .field("client_id", &self.client_id)
            .field("client_secret", &"<redacted>")
            .field("redirect_url", &self.redirect_url)
            .field("authorize_url", &self.authorize_url)
            .field("token_url", &self.token_url)
            .field("scope", &self.scope)
            .field(
                "webhook_token",
                &self.webhook_token.as_ref().map(|_| "<redacted>"),
            )
            .finish()
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

    pub(super) fn test_defaults() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 3002,
        }
    }
}

impl MongoSettings {
    pub(super) fn test_defaults() -> Self {
        Self {
            uri: "mongodb://localhost:27017".to_string(),
            database: "aiwattcoach".to_string(),
        }
    }
}

impl AuthSettings {
    pub(super) fn test_defaults() -> Self {
        Self {
            google: GoogleOAuthSettings {
                client_id: "local-google-client-id".to_string(),
                client_secret: "local-google-client-secret".to_string(),
                redirect_url: "http://localhost:3002/api/auth/google/callback".to_string(),
            },
            wahoo: None,
            dev: DevAuthSettings {
                enabled: false,
                google_subject: "dev-google-subject".to_string(),
                email: "dev@aiwattcoach.local".to_string(),
                display_name: "Dev Athlete".to_string(),
                avatar_url: None,
            },
            session: SessionSettings {
                cookie_name: "aiwattcoach_session".to_string(),
                same_site: "lax".to_string(),
                ttl_hours: 24,
                secure: false,
            },
            admin_emails: Vec::new(),
        }
    }
}

impl SessionSettings {
    pub(super) fn parse(values: &BTreeMap<String, String>) -> Result<Self, SettingsError> {
        Ok(Self {
            cookie_name: parse_cookie_name(required(values, "SESSION_COOKIE_NAME")?.as_str())?,
            same_site: parse_same_site_setting(
                required(values, "SESSION_COOKIE_SAME_SITE")?.as_str(),
            )?,
            ttl_hours: parse_session_ttl_hours(required(values, "SESSION_TTL_HOURS")?.as_str())?,
            secure: super::parse::parse_bool_setting(
                required(values, "SESSION_COOKIE_SECURE")?.as_str(),
                "SESSION_COOKIE_SECURE",
            )?,
        })
    }
}

impl SettingsParts {
    pub(super) fn parse(
        values: &BTreeMap<String, String>,
        dev: DevAuthSettings,
    ) -> Result<Self, SettingsError> {
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
            auth: AuthSettingsParts { dev },
        })
    }
}
