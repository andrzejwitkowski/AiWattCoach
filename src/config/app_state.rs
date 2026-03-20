use mongodb::Client;

#[derive(Clone)]
pub struct AppState {
    pub app_name: String,
    pub mongo_database: String,
    pub mongo_client: Client,
}

impl AppState {
    pub fn new(
        app_name: impl Into<String>,
        mongo_database: impl Into<String>,
        mongo_client: Client,
    ) -> Self {
        Self {
            app_name: app_name.into(),
            mongo_database: mongo_database.into(),
            mongo_client,
        }
    }
}
