use aiwattcoach::domain::identity::{
    AppUser, AuthSession, GoogleLoginStart, GoogleLoginSuccess, IdentityError, IdentityUseCases,
    Role,
};

use super::app::BoxFuture;

#[derive(Clone)]
pub(crate) struct AdminIdentityErrorService {
    error: IdentityError,
}

impl AdminIdentityErrorService {
    pub(crate) fn new(error: IdentityError) -> Self {
        Self { error }
    }
}

impl IdentityUseCases for AdminIdentityErrorService {
    fn begin_google_login(
        &self,
        _return_to: Option<String>,
    ) -> BoxFuture<Result<GoogleLoginStart, IdentityError>> {
        Box::pin(async {
            Ok(GoogleLoginStart {
                state: "state-1".to_string(),
                redirect_url: "https://accounts.google.com/o/oauth2/v2/auth?state=state-1"
                    .to_string(),
            })
        })
    }

    fn handle_google_callback(
        &self,
        _state: &str,
        _code: &str,
    ) -> BoxFuture<Result<GoogleLoginSuccess, IdentityError>> {
        Box::pin(async { Err(IdentityError::External("not used in test".to_string())) })
    }

    fn get_current_user(
        &self,
        _session_id: &str,
    ) -> BoxFuture<Result<Option<AppUser>, IdentityError>> {
        Box::pin(async { Ok(None) })
    }

    fn logout(&self, _session_id: &str) -> BoxFuture<Result<(), IdentityError>> {
        Box::pin(async { Ok(()) })
    }

    fn require_admin(&self, _session_id: &str) -> BoxFuture<Result<AppUser, IdentityError>> {
        let error = self.error.clone();
        Box::pin(async move { Err(error) })
    }
}

#[derive(Clone, Default)]
pub(crate) struct TestIdentityServiceWithSession {
    pub(crate) session_id: String,
    pub(crate) user_id: String,
    pub(crate) roles: Vec<Role>,
}

impl IdentityUseCases for TestIdentityServiceWithSession {
    fn begin_google_login(
        &self,
        _return_to: Option<String>,
    ) -> BoxFuture<Result<GoogleLoginStart, IdentityError>> {
        Box::pin(async {
            Ok(GoogleLoginStart {
                state: "state-1".to_string(),
                redirect_url: "https://accounts.google.com/o/oauth2/v2/auth?state=state-1"
                    .to_string(),
            })
        })
    }

    fn handle_google_callback(
        &self,
        _state: &str,
        _code: &str,
    ) -> BoxFuture<Result<GoogleLoginSuccess, IdentityError>> {
        let roles = self.roles.clone();
        let user_id = self.user_id.clone();
        let session_id = self.session_id.clone();
        Box::pin(async move {
            Ok(GoogleLoginSuccess {
                user: AppUser::new(
                    user_id.clone(),
                    "google-subject-1".to_string(),
                    "athlete@example.com".to_string(),
                    roles,
                    Some("Test User".to_string()),
                    None,
                    true,
                ),
                session: AuthSession::new(session_id, user_id, 999999, 100),
                redirect_to: "/app".to_string(),
            })
        })
    }

    fn get_current_user(
        &self,
        session_id: &str,
    ) -> BoxFuture<Result<Option<AppUser>, IdentityError>> {
        let roles = self.roles.clone();
        let user_id = self.user_id.clone();
        let session_id_check = session_id.to_string();
        Box::pin(async move {
            if session_id_check != "session-1" {
                return Ok(None);
            }
            Ok(Some(AppUser::new(
                user_id,
                "google-subject-1".to_string(),
                "athlete@example.com".to_string(),
                roles,
                Some("Test User".to_string()),
                None,
                true,
            )))
        })
    }

    fn logout(&self, _session_id: &str) -> BoxFuture<Result<(), IdentityError>> {
        Box::pin(async { Ok(()) })
    }

    fn require_admin(&self, session_id: &str) -> BoxFuture<Result<AppUser, IdentityError>> {
        let roles = self.roles.clone();
        let user_id = self.user_id.clone();
        let session_id_check = session_id.to_string();
        Box::pin(async move {
            if session_id_check != "session-1" {
                return Err(IdentityError::Unauthenticated);
            }
            if !roles.contains(&Role::Admin) {
                return Err(IdentityError::Forbidden);
            }
            Ok(AppUser::new(
                user_id,
                "google-subject-1".to_string(),
                "admin@example.com".to_string(),
                roles,
                Some("Admin".to_string()),
                None,
                true,
            ))
        })
    }
}
