use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use aiwattcoach::domain::identity::{
    assign_roles, AppUser, AuthSession, BoxFuture, Clock, GoogleIdentity, GoogleOAuthPort,
    IdGenerator, IdentityError, IdentityService, IdentityServiceConfig, LoginState,
    LoginStateRepository, SessionRepository, UserRepository,
};

#[tokio::test]
async fn begin_google_login_persists_state_before_returning_redirect() {
    let login_states = Arc::new(Mutex::new(Vec::new()));
    let service = test_service(login_states.clone(), Vec::new());

    let result = service
        .begin_google_login(Some("/settings".to_string()))
        .await
        .unwrap();

    let states = login_states.lock().unwrap();
    assert_eq!(states.len(), 1);
    assert_eq!(states[0].id, result.state);
    assert_eq!(
        result.redirect_url,
        format!(
            "https://accounts.google.com/o/oauth2/v2/auth?state={}",
            result.state
        )
    );
}

#[tokio::test]
async fn begin_google_login_drops_unsafe_return_to_values() {
    let login_states = Arc::new(Mutex::new(Vec::new()));
    let service = test_service(login_states.clone(), Vec::new());

    let result = service
        .begin_google_login(Some("https://evil.example".to_string()))
        .await
        .unwrap();

    let states = login_states.lock().unwrap();
    assert_eq!(states.len(), 1);
    assert_eq!(states[0].return_to, None);
    assert_eq!(result.state, "login-state-1");
}

#[tokio::test]
async fn begin_google_login_drops_control_character_return_to_values() {
    let login_states = Arc::new(Mutex::new(Vec::new()));
    let service = test_service(login_states.clone(), Vec::new());

    let result = service
        .begin_google_login(Some("/settings%0d%0aX-Test: injected".to_string()))
        .await
        .unwrap();

    let states = login_states.lock().unwrap();
    assert_eq!(states.len(), 1);
    assert_eq!(states[0].return_to, None);
    assert_eq!(result.state, "login-state-1");
}

#[tokio::test]
async fn handle_google_callback_creates_new_user_and_session() {
    let login_states = Arc::new(Mutex::new(vec![LoginState::new(
        "state-1".to_string(),
        Some("/app".to_string()),
        200,
        100,
    )]));

    let service = test_service(login_states, vec!["admin@example.com".to_string()]);

    let result = service
        .handle_google_callback("state-1", "oauth-code")
        .await
        .unwrap();

    assert_eq!(result.redirect_to, "/app");
    assert_eq!(result.user.email, "admin@example.com");
    assert!(result.user.is_admin());
    assert_eq!(result.user.id, "user-1");
    assert_eq!(result.session.id, "session-1");
    assert_eq!(result.session.user_id, result.user.id);
}

#[tokio::test]
async fn handle_google_callback_rejects_missing_state() {
    let service = test_service(Arc::new(Mutex::new(Vec::new())), Vec::new());

    let error = service
        .handle_google_callback("missing-state", "oauth-code")
        .await
        .unwrap_err();

    assert_eq!(error, IdentityError::InvalidLoginState);
}

#[tokio::test]
async fn handle_google_callback_consumes_login_state_before_side_effects() {
    let login_states = Arc::new(Mutex::new(vec![LoginState::new(
        "state-1".to_string(),
        Some("/app".to_string()),
        200,
        100,
    )]));
    let service = test_service(login_states.clone(), vec!["admin@example.com".to_string()]);

    let _ = service
        .handle_google_callback("state-1", "oauth-code")
        .await
        .unwrap();

    let states = login_states.lock().unwrap();
    assert!(states.is_empty());
}

#[tokio::test]
async fn logout_deletes_session() {
    let service = test_service(Arc::new(Mutex::new(Vec::new())), Vec::new());
    let session = AuthSession::new("session-1".to_string(), "user-1".to_string(), 200, 100);

    service.sessions.save(session.clone()).await.unwrap();
    service.logout(&session.id).await.unwrap();

    let loaded = service.sessions.find_by_id(&session.id).await.unwrap();
    assert!(loaded.is_none());
}

#[tokio::test]
async fn get_current_user_returns_none_for_unknown_session() {
    let service = test_service(Arc::new(Mutex::new(Vec::new())), Vec::new());

    let user = service.get_current_user("missing-session").await.unwrap();

    assert!(user.is_none());
}

#[tokio::test]
async fn get_current_user_treats_boundary_expiry_as_expired() {
    let service = test_service(Arc::new(Mutex::new(Vec::new())), Vec::new());
    service
        .sessions
        .save(AuthSession::new(
            "session-1".to_string(),
            "user-1".to_string(),
            100,
            90,
        ))
        .await
        .unwrap();

    let user = service.get_current_user("session-1").await.unwrap();

    assert!(user.is_none());
    let session = service.sessions.find_by_id("session-1").await.unwrap();
    assert!(session.is_none());
}

#[tokio::test]
async fn require_admin_rejects_non_admin_user() {
    let users = InMemoryUsers::default();
    let sessions = InMemorySessions::default();
    let login_states = InMemoryLoginStates {
        items: Arc::new(Mutex::new(Vec::new())),
    };

    let user = AppUser::new(
        "user-1".to_string(),
        "google-subject-1".to_string(),
        "athlete@example.com".to_string(),
        vec![aiwattcoach::domain::identity::Role::User],
        Some("Athlete".to_string()),
        None,
        true,
    );

    users.save(user.clone()).await.unwrap();
    sessions
        .save(AuthSession::new(
            "session-1".to_string(),
            user.id.clone(),
            200,
            100,
        ))
        .await
        .unwrap();

    let service = IdentityService::new(
        users,
        sessions,
        login_states,
        TestGoogleOAuthAdapter,
        TestClock,
        TestIdGenerator,
        IdentityServiceConfig::new(Vec::new(), 24),
    );

    let error = service.require_admin("session-1").await.unwrap_err();

    assert_eq!(error, IdentityError::Forbidden);
}

#[tokio::test]
async fn handle_google_callback_rejects_overflowing_session_ttl() {
    let login_states = Arc::new(Mutex::new(vec![LoginState::new(
        "state-1".to_string(),
        Some("/app".to_string()),
        200,
        100,
    )]));
    let users = InMemoryUsers::default();
    let sessions = InMemorySessions::default();
    let states = InMemoryLoginStates {
        items: login_states,
    };
    let service = IdentityService::new(
        users,
        sessions,
        states,
        TestGoogleOAuthAdapter,
        TestClock,
        TestIdGenerator,
        IdentityServiceConfig::new(Vec::new(), (i64::MAX as u64) / 3600 + 1),
    );

    let error = service
        .handle_google_callback("state-1", "oauth-code")
        .await
        .unwrap_err();

    assert!(
        matches!(error, IdentityError::External(message) if message.contains("SESSION_TTL_HOURS"))
    );
}

#[derive(Clone)]
struct TestGoogleOAuthAdapter;

impl GoogleOAuthPort for TestGoogleOAuthAdapter {
    fn build_authorize_url(&self, state: &str) -> Result<String, IdentityError> {
        Ok(format!(
            "https://accounts.google.com/o/oauth2/v2/auth?state={state}"
        ))
    }

    fn exchange_code_for_identity(
        &self,
        code: &str,
    ) -> BoxFuture<Result<GoogleIdentity, IdentityError>> {
        let code = code.to_string();
        Box::pin(async move {
            if code == "unverified" {
                return GoogleIdentity::new(
                    "google-subject-1",
                    "athlete@example.com",
                    false,
                    Some("Athlete".to_string()),
                    None,
                );
            }

            GoogleIdentity::new(
                "google-subject-1",
                "admin@example.com",
                true,
                Some("Admin Athlete".to_string()),
                Some("https://example.com/avatar.png".to_string()),
            )
        })
    }
}

#[derive(Clone)]
struct TestClock;

impl Clock for TestClock {
    fn now_epoch_seconds(&self) -> i64 {
        100
    }
}

#[derive(Clone)]
struct TestIdGenerator;

impl IdGenerator for TestIdGenerator {
    fn new_id(&self, prefix: &str) -> String {
        format!("{prefix}-1")
    }
}

#[derive(Clone, Default)]
struct InMemoryUsers {
    by_google_subject: Arc<Mutex<BTreeMap<String, AppUser>>>,
}

impl UserRepository for InMemoryUsers {
    fn find_by_id(&self, user_id: &str) -> BoxFuture<Result<Option<AppUser>, IdentityError>> {
        let id = user_id.to_string();
        let data = self.by_google_subject.clone();
        Box::pin(async move {
            Ok(data
                .lock()
                .unwrap()
                .values()
                .find(|user| user.id == id)
                .cloned())
        })
    }

    fn find_by_google_subject(
        &self,
        google_subject: &str,
    ) -> BoxFuture<Result<Option<AppUser>, IdentityError>> {
        let key = google_subject.to_string();
        let data = self.by_google_subject.clone();
        Box::pin(async move { Ok(data.lock().unwrap().get(&key).cloned()) })
    }

    fn find_by_normalized_email(
        &self,
        normalized_email: &str,
    ) -> BoxFuture<Result<Option<AppUser>, IdentityError>> {
        let email = normalized_email.to_string();
        let data = self.by_google_subject.clone();
        Box::pin(async move {
            Ok(data
                .lock()
                .unwrap()
                .values()
                .find(|user| user.email_normalized == email)
                .cloned())
        })
    }

    fn save(&self, user: AppUser) -> BoxFuture<Result<AppUser, IdentityError>> {
        let data = self.by_google_subject.clone();
        Box::pin(async move {
            data.lock()
                .unwrap()
                .insert(user.google_subject.clone(), user.clone());
            Ok(user)
        })
    }
}

#[derive(Clone, Default)]
struct InMemorySessions {
    items: Arc<Mutex<BTreeMap<String, AuthSession>>>,
}

impl SessionRepository for InMemorySessions {
    fn find_by_id(
        &self,
        session_id: &str,
    ) -> BoxFuture<Result<Option<AuthSession>, IdentityError>> {
        let id = session_id.to_string();
        let data = self.items.clone();
        Box::pin(async move { Ok(data.lock().unwrap().get(&id).cloned()) })
    }

    fn save(&self, session: AuthSession) -> BoxFuture<Result<AuthSession, IdentityError>> {
        let data = self.items.clone();
        Box::pin(async move {
            data.lock()
                .unwrap()
                .insert(session.id.clone(), session.clone());
            Ok(session)
        })
    }

    fn delete(&self, session_id: &str) -> BoxFuture<Result<(), IdentityError>> {
        let id = session_id.to_string();
        let data = self.items.clone();
        Box::pin(async move {
            data.lock().unwrap().remove(&id);
            Ok(())
        })
    }
}

#[derive(Clone)]
struct InMemoryLoginStates {
    items: Arc<Mutex<Vec<LoginState>>>,
}

impl LoginStateRepository for InMemoryLoginStates {
    fn create(&self, login_state: LoginState) -> BoxFuture<Result<LoginState, IdentityError>> {
        let data = self.items.clone();
        Box::pin(async move {
            data.lock().unwrap().push(login_state.clone());
            Ok(login_state)
        })
    }

    fn find_by_id(&self, state_id: &str) -> BoxFuture<Result<Option<LoginState>, IdentityError>> {
        let id = state_id.to_string();
        let data = self.items.clone();
        Box::pin(async move {
            Ok(data
                .lock()
                .unwrap()
                .iter()
                .find(|state| state.id == id)
                .cloned())
        })
    }

    fn delete(&self, state_id: &str) -> BoxFuture<Result<(), IdentityError>> {
        let id = state_id.to_string();
        let data = self.items.clone();
        Box::pin(async move {
            data.lock().unwrap().retain(|state| state.id != id);
            Ok(())
        })
    }
}

struct TestIdentityService {
    inner: IdentityService<
        InMemoryUsers,
        InMemorySessions,
        InMemoryLoginStates,
        TestGoogleOAuthAdapter,
        TestClock,
        TestIdGenerator,
    >,
    sessions: InMemorySessions,
}

fn test_service(
    login_states: Arc<Mutex<Vec<LoginState>>>,
    admin_emails: Vec<String>,
) -> TestIdentityService {
    let users = InMemoryUsers::default();
    let sessions = InMemorySessions::default();
    let states = InMemoryLoginStates {
        items: login_states,
    };

    let service = IdentityService::new(
        users,
        sessions.clone(),
        states,
        TestGoogleOAuthAdapter,
        TestClock,
        TestIdGenerator,
        IdentityServiceConfig::new(admin_emails, 24),
    );

    TestIdentityService {
        inner: service,
        sessions,
    }
}

impl std::ops::Deref for TestIdentityService {
    type Target = IdentityService<
        InMemoryUsers,
        InMemorySessions,
        InMemoryLoginStates,
        TestGoogleOAuthAdapter,
        TestClock,
        TestIdGenerator,
    >;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

#[test]
fn assign_roles_test_helper_still_assigns_admin() {
    let roles = assign_roles("admin@example.com", &["admin@example.com".to_string()]);
    assert!(roles.contains(&aiwattcoach::domain::identity::Role::Admin));
}
