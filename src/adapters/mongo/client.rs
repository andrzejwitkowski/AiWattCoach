use std::time::Duration;

use mongodb::{bson::doc, Client};
use tokio::time::timeout;

const BOOTSTRAP_COLLECTION: &str = "_bootstrap";

#[derive(Debug)]
pub enum MongoConnectionError {
    Timeout {
        database: String,
    },
    Query {
        database: String,
        source: mongodb::error::Error,
    },
}

impl std::fmt::Display for MongoConnectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Timeout { database } => {
                write!(f, "Timed out while pinging Mongo database '{database}'")
            }
            Self::Query { database, source } => {
                write!(f, "Failed to ping Mongo database '{database}': {source}")
            }
        }
    }
}

impl std::error::Error for MongoConnectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Timeout { .. } => None,
            Self::Query { source, .. } => Some(source),
        }
    }
}

pub async fn create_client(uri: &str) -> Result<Client, mongodb::error::Error> {
    Client::with_uri_str(uri).await
}

pub async fn verify_connection(
    client: &Client,
    database: &str,
    timeout_duration: Duration,
) -> Result<(), MongoConnectionError> {
    let ping_result = timeout(
        timeout_duration,
        client.database(database).run_command(doc! { "ping": 1 }),
    )
    .await
    .map_err(|_| MongoConnectionError::Timeout {
        database: database.to_string(),
    })?;

    ping_result
        .map(|_| ())
        .map_err(|source| MongoConnectionError::Query {
            database: database.to_string(),
            source,
        })
}

pub async fn ensure_database_exists(
    client: &Client,
    database: &str,
) -> Result<(), mongodb::error::Error> {
    let database_names = client.list_database_names().await?;

    if database_needs_bootstrap(&database_names, database) {
        client
            .database(database)
            .create_collection(BOOTSTRAP_COLLECTION)
            .await?;
    }

    Ok(())
}

fn database_needs_bootstrap(database_names: &[String], database: &str) -> bool {
    !database_names.iter().any(|name| name == database)
}

#[cfg(test)]
mod tests {
    use super::database_needs_bootstrap;

    #[test]
    fn database_needs_bootstrap_when_database_name_is_missing() {
        assert!(database_needs_bootstrap(
            &["admin".to_string(), "config".to_string()],
            "aiwattcoach_test"
        ));
    }

    #[test]
    fn database_does_not_need_bootstrap_when_database_already_exists() {
        assert!(!database_needs_bootstrap(
            &["admin".to_string(), "aiwattcoach_test".to_string()],
            "aiwattcoach_test"
        ));
    }
}
