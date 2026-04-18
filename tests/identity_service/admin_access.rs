use std::sync::{Arc, Mutex};

use aiwattcoach::domain::identity::{
    AppUser, AuthSession, IdentityError, IdentityService, IdentityServiceConfig,
    IdentityServiceDependencies, SessionRepository, UserRepository,
};

use crate::shared::{
    test_service, InMemoryLoginStates, InMemorySessions, InMemoryUsers, InMemoryWhitelist,
    TestClock, TestGoogleOAuthAdapter, TestIdGenerator,
};

#[tokio::test]
async fn require_admin_rejects_non_admin_user() {
    let users = InMemoryUsers::default();
    let sessions = InMemorySessions::default();
    let login_states = InMemoryLoginStates {
        items: Arc::new(Mutex::new(Vec::new())),
    };
    let whitelist = InMemoryWhitelist::default();

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
        IdentityServiceDependencies {
            users,
            sessions,
            login_states,
            whitelist,
            google_oauth: TestGoogleOAuthAdapter,
            clock: TestClock,
            ids: TestIdGenerator,
        },
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
