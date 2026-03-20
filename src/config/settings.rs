use std::{collections::BTreeMap, env, error::Error, fmt};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub app_name: String,
    pub server: ServerSettings,
    pub mongo: MongoSettings,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsError {
    message: String,
}

impl Settings {
    pub fn from_env() -> Result<Self, SettingsError> {
        let keys = [
            "APP_NAME",
            "SERVER_HOST",
            "SERVER_PORT",
            "MONGODB_URI",
            "MONGODB_DATABASE",
        ];
        let values = keys
            .into_iter()
            .filter_map(|key| env::var(key).ok().map(|value| (key.to_string(), value)))
            .collect::<BTreeMap<_, _>>();

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
        })
    }

    pub fn test_defaults() -> Self {
        Self {
            app_name: "AiWattCoach".to_string(),
            server: ServerSettings {
                host: "127.0.0.1".to_string(),
                port: 3000,
            },
            mongo: MongoSettings {
                uri: "mongodb://localhost:27017".to_string(),
                database: "aiwattcoach".to_string(),
            },
        }
    }
}

impl ServerSettings {
    pub fn address(&self) -> String {
        format!("{}:{}", self.host, self.port)
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
    values
        .get(key)
        .cloned()
        .ok_or_else(|| SettingsError::new(format!("Missing required setting: {key}")))
}
