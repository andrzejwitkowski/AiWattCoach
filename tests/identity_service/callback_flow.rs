use std::sync::{Arc, Mutex};

use aiwattcoach::domain::identity::{
    AppUser, GoogleLoginOutcome, IdentityError, IdentityService, IdentityServiceConfig,
    IdentityServiceDependencies, LoginState, SessionRepository, UserRepository, WhitelistEntry,
    WhitelistRepository,
};

use crate::shared::{
    test_service, InMemoryLoginStates, InMemorySessions, InMemoryUsers, InMemoryWhitelist,
    TestClock, TestGoogleOAuthAdapter, TestIdGenerator,
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

    service
        .whitelist
        .save(WhitelistEntry::new(
            "admin@example.com".to_string(),
            true,
            90,
            90,
        ))
        .await
        .unwrap();

    let result = service
        .handle_google_callback("state-1", "oauth-code")
        .await
        .unwrap();

    let GoogleLoginOutcome::SignedIn(result) = result else {
        panic!("expected signed in outcome");
    };

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

    service
        .whitelist
        .save(WhitelistEntry::new(
            "admin@example.com".to_string(),
            true,
            90,
            90,
        ))
        .await
        .unwrap();

    let result = service
        .handle_google_callback("state-1", "oauth-code")
        .await
        .unwrap();

    let GoogleLoginOutcome::SignedIn(result) = result else {
        panic!("expected signed in outcome");
    };

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
async fn join_whitelist_refreshes_existing_entry_timestamp() {
    let service = test_service(Arc::new(Mutex::new(Vec::new())), Vec::new());

    service
        .whitelist
        .save(WhitelistEntry::new(
            "athlete@example.com".to_string(),
            false,
            90,
            90,
        ))
        .await
        .unwrap();

    let entry = service
        .join_whitelist("athlete@example.com".to_string())
        .await
        .unwrap();

    assert_eq!(entry.created_at_epoch_seconds, 90);
    assert_eq!(entry.updated_at_epoch_seconds, 100);
    assert_eq!(entry.email, "athlete@example.com");
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
    let whitelist = InMemoryWhitelist::default();
    whitelist
        .save(WhitelistEntry::new(
            "admin@example.com".to_string(),
            true,
            90,
            90,
        ))
        .await
        .unwrap();
    let service = IdentityService::new(
        IdentityServiceDependencies {
            users,
            sessions,
            login_states: states,
            whitelist,
            google_oauth: TestGoogleOAuthAdapter,
            clock: TestClock,
            ids: TestIdGenerator,
        },
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

    service
        .whitelist
        .save(WhitelistEntry::new(
            "admin@example.com".to_string(),
            true,
            90,
            90,
        ))
        .await
        .unwrap();

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

    service
        .whitelist
        .save(WhitelistEntry::new(
            "admin@example.com".to_string(),
            true,
            90,
            90,
        ))
        .await
        .unwrap();

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
    let whitelist = InMemoryWhitelist::default();
    whitelist
        .save(WhitelistEntry::new(
            "admin@example.com".to_string(),
            true,
            90,
            90,
        ))
        .await
        .unwrap();
    let service = IdentityService::new(
        IdentityServiceDependencies {
            users,
            sessions,
            login_states: states,
            whitelist,
            google_oauth: TestGoogleOAuthAdapter,
            clock: TestClock,
            ids: TestIdGenerator,
        },
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

#[tokio::test]
async fn handle_google_callback_adds_missing_user_to_whitelist_and_returns_pending_approval() {
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

    assert_eq!(
        result,
        GoogleLoginOutcome::PendingApproval {
            redirect_to: "/?auth=pending-approval&returnTo=%2Fapp".to_string()
        }
    );
    let whitelist_entry = service
        .whitelist
        .find_by_normalized_email("admin@example.com")
        .await
        .unwrap()
        .unwrap();
    assert!(!whitelist_entry.allowed);
    assert!(service
        .sessions
        .find_by_id("session-1")
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn handle_google_callback_refreshes_existing_pending_whitelist_entry_timestamp() {
    let login_states = Arc::new(Mutex::new(vec![LoginState::new(
        "state-1".to_string(),
        Some("/app".to_string()),
        200,
        100,
    )]));
    let service = test_service(login_states, vec!["admin@example.com".to_string()]);
    service
        .whitelist
        .save(WhitelistEntry::new(
            "Admin@Example.com".to_string(),
            false,
            10,
            10,
        ))
        .await
        .unwrap();

    let result = service
        .handle_google_callback("state-1", "oauth-code")
        .await
        .unwrap();

    assert_eq!(
        result,
        GoogleLoginOutcome::PendingApproval {
            redirect_to: "/?auth=pending-approval&returnTo=%2Fapp".to_string()
        }
    );
    let whitelist_entry = service
        .whitelist
        .find_by_normalized_email("admin@example.com")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(whitelist_entry.email, "Admin@Example.com");
    assert_eq!(whitelist_entry.created_at_epoch_seconds, 10);
    assert_eq!(whitelist_entry.updated_at_epoch_seconds, 100);
}

#[tokio::test]
async fn handle_google_callback_allows_new_user_when_whitelist_entry_is_approved() {
    let login_states = Arc::new(Mutex::new(vec![LoginState::new(
        "state-1".to_string(),
        Some("/app".to_string()),
        200,
        100,
    )]));
    let service = test_service(login_states, vec!["admin@example.com".to_string()]);
    service
        .whitelist
        .save(WhitelistEntry::new(
            "admin@example.com".to_string(),
            true,
            10,
            20,
        ))
        .await
        .unwrap();

    let result = service
        .handle_google_callback("state-1", "oauth-code")
        .await
        .unwrap();

    let GoogleLoginOutcome::SignedIn(result) = result else {
        panic!("expected signed in outcome");
    };
    assert_eq!(result.user.email, "admin@example.com");
    assert_eq!(result.session.id, "session-1");
}

#[tokio::test]
async fn handle_google_callback_keeps_deep_link_in_pending_approval_redirect() {
    let login_states = Arc::new(Mutex::new(vec![LoginState::new(
        "state-1".to_string(),
        Some("/settings?tab=security#billing".to_string()),
        200,
        100,
    )]));
    let service = test_service(login_states, vec![]);

    let result = service
        .handle_google_callback("state-1", "oauth-code")
        .await
        .unwrap();

    assert_eq!(
        result,
        GoogleLoginOutcome::PendingApproval {
            redirect_to: "/?auth=pending-approval&returnTo=%2Fsettings%3Ftab%3Dsecurity%23billing"
                .to_string()
        }
    );
}
