use std::collections::HashMap;

use aiwattcoach::domain::identity::{
    AppUser, AuthSession, GoogleLoginOutcome, GoogleLoginStart, GoogleLoginSuccess, IdentityError,
    IdentityUseCases, Role, WhitelistEntry,
};

use crate::test_support::BoxFuture;

#[derive(Clone)]
pub(crate) struct TestIdentityServiceWithSession {
    session_id: String,
    user_id: String,
    email: String,
    roles: Vec<Role>,
    display_name: String,
}

impl Default for TestIdentityServiceWithSession {
    fn default() -> Self {
        Self {
            session_id: "session-1".to_string(),
            user_id: "user-1".to_string(),
            email: "athlete@example.com".to_string(),
            roles: vec![Role::User],
            display_name: "Test User".to_string(),
        }
    }
}

impl TestIdentityServiceWithSession {
    fn build_user(&self) -> AppUser {
        AppUser::new(
            self.user_id.clone(),
            format!("google-subject-{}", self.user_id),
            self.email.clone(),
            self.roles.clone(),
            Some(self.display_name.clone()),
            None,
            true,
        )
    }
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

    fn join_whitelist(&self, email: String) -> BoxFuture<Result<WhitelistEntry, IdentityError>> {
        Box::pin(async move { Ok(WhitelistEntry::new(email, false, 100, 100)) })
    }

    fn handle_google_callback(
        &self,
        _state: &str,
        _code: &str,
    ) -> BoxFuture<Result<GoogleLoginOutcome, IdentityError>> {
        let user_id = self.user_id.clone();
        let session_id = self.session_id.clone();
        let user = self.build_user();
        Box::pin(async move {
            Ok(GoogleLoginOutcome::SignedIn(Box::new(GoogleLoginSuccess {
                user,
                session: AuthSession::new(session_id, user_id, 999999, 100),
                redirect_to: "/app".to_string(),
            })))
        })
    }

    fn get_current_user(
        &self,
        session_id: &str,
    ) -> BoxFuture<Result<Option<AppUser>, IdentityError>> {
        let expected_session_id = self.session_id.clone();
        let user = self.build_user();
        let session_id_check = session_id.to_string();
        Box::pin(async move {
            if session_id_check != expected_session_id {
                return Ok(None);
            }
            Ok(Some(user))
        })
    }

    fn logout(&self, _session_id: &str) -> BoxFuture<Result<(), IdentityError>> {
        Box::pin(async { Ok(()) })
    }

    fn require_admin(&self, session_id: &str) -> BoxFuture<Result<AppUser, IdentityError>> {
        let expected_session_id = self.session_id.clone();
        let roles = self.roles.clone();
        let user_id = self.user_id.clone();
        let session_id_check = session_id.to_string();
        Box::pin(async move {
            if session_id_check != expected_session_id {
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

#[derive(Clone, Default)]
pub(crate) struct SessionMappedIdentityService {
    users_by_session: HashMap<String, AppUser>,
}

impl SessionMappedIdentityService {
    pub(crate) fn with_users<const N: usize>(entries: [(&str, &str, &str); N]) -> Self {
        let users_by_session = entries
            .into_iter()
            .map(|(session_id, user_id, email)| {
                (
                    session_id.to_string(),
                    AppUser::new(
                        user_id.to_string(),
                        format!("google-subject-{user_id}"),
                        email.to_string(),
                        vec![Role::User],
                        Some(format!("User {user_id}")),
                        None,
                        true,
                    ),
                )
            })
            .collect();

        Self { users_by_session }
    }
}

impl IdentityUseCases for SessionMappedIdentityService {
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

    fn join_whitelist(&self, email: String) -> BoxFuture<Result<WhitelistEntry, IdentityError>> {
        Box::pin(async move { Ok(WhitelistEntry::new(email, false, 100, 100)) })
    }

    fn handle_google_callback(
        &self,
        _state: &str,
        _code: &str,
    ) -> BoxFuture<Result<GoogleLoginOutcome, IdentityError>> {
        Box::pin(async { Err(IdentityError::External("not used in test".to_string())) })
    }

    fn get_current_user(
        &self,
        session_id: &str,
    ) -> BoxFuture<Result<Option<AppUser>, IdentityError>> {
        let user = self.users_by_session.get(session_id).cloned();
        Box::pin(async move { Ok(user) })
    }

    fn logout(&self, _session_id: &str) -> BoxFuture<Result<(), IdentityError>> {
        Box::pin(async { Ok(()) })
    }

    fn require_admin(&self, _session_id: &str) -> BoxFuture<Result<AppUser, IdentityError>> {
        Box::pin(async { Err(IdentityError::Forbidden) })
    }
}
