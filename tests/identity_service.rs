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
async fn handle_google_callback_rejects_conflicting_subject_and_email_matches() {
    let login_states = Arc::new(Mutex::new(vec![LoginState::new(
        "state-1".to_string(),
        Some("/app".to_string()),
        200,
        100,
    )]));
    let users = InMemoryUsers::default();
    users
        .save(AppUser::new(
            "user-subject".to_string(),
            "google-subject-1".to_string(),
            "old-subject@example.com".to_string(),
            vec![aiwattcoach::domain::identity::Role::User],
            Some("Subject Match".to_string()),
            None,
            true,
        ))
        .await
        .unwrap();
    users
        .save(AppUser::new(
            "user-email".to_string(),
            "google-subject-2".to_string(),
            "admin@example.com".to_string(),
            vec![aiwattcoach::domain::identity::Role::User],
            Some("Email Match".to_string()),
            None,
            true,
        ))
        .await
        .unwrap();
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
        IdentityServiceConfig::new(vec![], 24),
    );

    let error = service
        .handle_google_callback("state-1", "oauth-code")
        .await
        .unwrap_err();

    assert!(
        matches!(error, IdentityError::Repository(message) if message.contains("conflicting google subject/email mapping"))
    );
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
async fn handle_google_callback_consumes_login_state_atomically() {
    let login_states = Arc::new(Mutex::new(vec![LoginState::new(
        "state-1".to_string(),
        Some("/app".to_string()),
        200,
        100,
    )]));
    let service = test_service(login_states, vec![]);

    let first = service
        .handle_google_callback("state-1", "oauth-code")
        .await;
    let second = service
        .handle_google_callback("state-1", "oauth-code")
        .await;

    assert!(first.is_ok());
    assert_eq!(second.unwrap_err(), IdentityError::InvalidLoginState);
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
async fn require_admin_rejects_missing_session_as_unauthenticated() {
    let service = test_service(Arc::new(Mutex::new(Vec::new())), Vec::new());

    let error = service.require_admin("missing-session").await.unwrap_err();

    assert_eq!(error, IdentityError::Unauthenticated);
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

    fn upsert_google_user(
        &self,
        new_user_id: String,
        google_identity: GoogleIdentity,
        roles: Vec<aiwattcoach::domain::identity::Role>,
    ) -> BoxFuture<Result<AppUser, IdentityError>> {
        let data = self.by_google_subject.clone();
        Box::pin(async move {
            let mut users = data.lock().unwrap();
            let existing = users
                .values()
                .find(|user| {
                    user.google_subject == google_identity.subject
                        || user.email_normalized == google_identity.email_normalized
                })
                .cloned();

            let user = AppUser::new(
                existing.map(|user| user.id).unwrap_or(new_user_id),
                google_identity.subject.clone(),
                google_identity.email.clone(),
                roles,
                google_identity.display_name.clone(),
                google_identity.avatar_url.clone(),
                google_identity.email_verified,
            );

            if let Some(previous_subject) = users
                .iter()
                .find(|(_, existing_user)| existing_user.id == user.id)
                .map(|(subject, _)| subject.clone())
            {
                users.remove(&previous_subject);
            }

            users.insert(user.google_subject.clone(), user.clone());
            Ok(user)
        })
    }

    fn save_google_user_for_identity(
        &self,
        new_user_id: String,
        google_identity: GoogleIdentity,
        roles: Vec<aiwattcoach::domain::identity::Role>,
    ) -> BoxFuture<Result<AppUser, IdentityError>> {
        let repository = self.clone();
        Box::pin(async move {
            let by_subject = repository
                .find_by_google_subject(&google_identity.subject)
                .await?;
            let by_email = repository
                .find_by_normalized_email(&google_identity.email_normalized)
                .await?;

            match (by_subject, by_email) {
                (Some(subject_user), Some(email_user)) if subject_user.id != email_user.id => {
                    Err(IdentityError::Repository(
                        "conflicting google subject/email mapping".to_string(),
                    ))
                }
                (Some(_), _) => {
                    repository
                        .upsert_google_user(new_user_id, google_identity, roles)
                        .await
                }
                (None, Some(email_user)) => {
                    if email_user.google_subject != google_identity.subject {
                        return Err(IdentityError::Repository(
                            "conflicting google subject/email mapping".to_string(),
                        ));
                    }

                    repository
                        .save(AppUser::new(
                            email_user.id,
                            google_identity.subject,
                            google_identity.email,
                            roles,
                            google_identity.display_name,
                            google_identity.avatar_url,
                            google_identity.email_verified,
                        ))
                        .await
                }
                (None, None) => {
                    repository
                        .upsert_google_user(new_user_id, google_identity, roles)
                        .await
                }
            }
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

    fn consume(&self, state_id: &str) -> BoxFuture<Result<Option<LoginState>, IdentityError>> {
        let id = state_id.to_string();
        let data = self.items.clone();
        Box::pin(async move {
            let mut items = data.lock().unwrap();
            let index = items.iter().position(|state| state.id == id);
            Ok(index.map(|position| items.remove(position)))
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
