use mongodb::Client;

use crate::config::Settings;

pub async fn create_client(settings: &Settings) -> Result<Client, mongodb::error::Error> {
    Client::with_uri_str(&settings.mongo.uri).await
}
