use std::time::Duration;

use mongodb::{bson::doc, Client};
use tokio::time::timeout;

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
