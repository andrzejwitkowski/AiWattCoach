use std::{collections::BTreeMap, env, error::Error, fmt, io::ErrorKind};

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
        })
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
