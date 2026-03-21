use std::sync::Arc;

use mongodb::Client;

use crate::domain::identity::IdentityUseCases;

#[derive(Clone)]
pub struct AppState {
    pub app_name: String,
    pub mongo_database: String,
    pub mongo_client: Client,
    pub identity_service: Option<Arc<dyn IdentityUseCases>>,
    pub session_cookie_name: String,
    pub secure_session_cookie: bool,
    pub session_ttl_hours: u64,
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
            identity_service: None,
            session_cookie_name: "aiwattcoach_session".to_string(),
            secure_session_cookie: false,
            session_ttl_hours: 24,
        }
    }

    pub fn with_identity_service(
        mut self,
        identity_service: Arc<dyn IdentityUseCases>,
        session_cookie_name: impl Into<String>,
        secure_session_cookie: bool,
        session_ttl_hours: u64,
    ) -> Self {
        self.identity_service = Some(identity_service);
        self.session_cookie_name = session_cookie_name.into();
        self.secure_session_cookie = secure_session_cookie;
        self.session_ttl_hours = session_ttl_hours;
        self
    }
}
