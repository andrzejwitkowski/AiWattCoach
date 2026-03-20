use mongodb::Client;

use super::Settings;

#[derive(Clone)]
pub struct AppState {
    pub settings: Settings,
    pub mongo_client: Option<Client>,
}

impl AppState {
    pub fn new(settings: Settings, mongo_client: Client) -> Self {
        Self {
            settings,
            mongo_client: Some(mongo_client),
        }
    }

    pub fn without_mongo(settings: Settings) -> Self {
        Self {
            settings,
            mongo_client: None,
        }
    }
}
