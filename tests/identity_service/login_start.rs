use std::sync::{Arc, Mutex};

use crate::shared::test_service;

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

#[test]
fn assign_roles_test_helper_still_assigns_admin() {
    let roles = aiwattcoach::domain::identity::assign_roles(
        "admin@example.com",
        &["admin@example.com".to_string()],
    );
    assert!(roles.contains(&aiwattcoach::domain::identity::Role::Admin));
}
