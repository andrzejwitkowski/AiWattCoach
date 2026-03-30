use std::sync::{Arc, Mutex};

use aiwattcoach::domain::identity::{AuthSession, SessionRepository};

use crate::shared::test_service;

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
