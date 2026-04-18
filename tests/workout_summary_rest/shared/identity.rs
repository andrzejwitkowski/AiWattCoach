use aiwattcoach::domain::identity::{
    AppUser, GoogleLoginOutcome, GoogleLoginStart, IdentityError, IdentityUseCases, Role,
    WhitelistEntry,
};

type BoxFuture<T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + 'static>>;

#[derive(Clone, Default)]
pub(crate) struct TestIdentityServiceWithSession {
    pub(crate) session_id: String,
    pub(crate) user_id: String,
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
        Box::pin(async { Err(IdentityError::External("not used in test".to_string())) })
    }

    fn get_current_user(
        &self,
        session_id: &str,
    ) -> BoxFuture<Result<Option<AppUser>, IdentityError>> {
        let user_id = self.user_id.clone();
        let expected_session_id = if self.session_id.is_empty() {
            "session-1".to_string()
        } else {
            self.session_id.clone()
        };
        let session_id = session_id.to_string();
        Box::pin(async move {
            if session_id != expected_session_id {
                return Ok(None);
            }

            Ok(Some(AppUser::new(
                if user_id.is_empty() {
                    "user-1".to_string()
                } else {
                    user_id
                },
                "google-subject-1".to_string(),
                "athlete@example.com".to_string(),
                vec![Role::User],
                Some("Test User".to_string()),
                None,
                true,
            )))
        })
    }

    fn logout(&self, _session_id: &str) -> BoxFuture<Result<(), IdentityError>> {
        Box::pin(async { Ok(()) })
    }

    fn require_admin(&self, _session_id: &str) -> BoxFuture<Result<AppUser, IdentityError>> {
        Box::pin(async { Err(IdentityError::Forbidden) })
    }
}
