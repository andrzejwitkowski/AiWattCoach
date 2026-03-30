use std::sync::{Arc, Mutex};

use aiwattcoach::domain::identity::{
    AppUser, IdentityError, IdentityService, IdentityServiceConfig, LoginState, UserRepository,
};

use crate::shared::{
    test_service, InMemoryLoginStates, InMemorySessions, InMemoryUsers, TestClock,
    TestGoogleOAuthAdapter, TestIdGenerator,
};

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
async fn handle_google_callback_defaults_redirect_to_calendar_when_return_to_missing() {
    let login_states = Arc::new(Mutex::new(vec![LoginState::new(
        "state-1".to_string(),
        None,
        200,
        100,
    )]));
    let service = test_service(login_states, vec!["admin@example.com".to_string()]);

    let result = service
        .handle_google_callback("state-1", "oauth-code")
        .await
        .unwrap();

    assert_eq!(result.redirect_to, "/calendar");
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
