use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use serde_json::Value;
use tower::util::ServiceExt;

use crate::{
    app::intervals_test_app,
    fixtures::{get_json, sample_event, session_cookie},
    identity_fakes::{SessionMappedIdentityService, TestIdentityServiceWithSession},
    intervals_fakes::{ScopedIntervalsService, TestIntervalsService},
};

#[tokio::test]
async fn create_event_returns_201() {
    let app = intervals_test_app(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::default(),
    )
    .await;

    let request_body = serde_json::json!({
        "category": "WORKOUT",
        "startDateLocal": "2026-03-25",
        "name": "Sweet Spot",
        "description": "mid-week",
        "indoor": true,
        "color": "green",
        "workoutDoc": "- 15min 88%\n- 5min 55%"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/intervals/events")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let body: Value = get_json(response).await;
    assert_eq!(body.get("name").unwrap().as_str().unwrap(), "Sweet Spot");
    assert_eq!(
        body.get("eventDefinition")
            .unwrap()
            .get("intervals")
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

#[tokio::test]
async fn create_event_rejects_ambiguous_file_upload_payload() {
    let app = intervals_test_app(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::default(),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/intervals/events")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "category": "WORKOUT",
                        "startDateLocal": "2026-03-25",
                        "fileUpload": {
                            "filename": "workout.zwo",
                            "fileContents": "<xml/>",
                            "fileContentsBase64": "PHhtbC8+"
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_event_rejects_empty_file_upload_payload() {
    let app = intervals_test_app(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::default(),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/intervals/events")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "category": "WORKOUT",
                        "startDateLocal": "2026-03-25",
                        "fileUpload": {
                            "filename": "workout.zwo",
                            "fileContents": "   "
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_event_is_scoped_to_authenticated_user() {
    let service = ScopedIntervalsService::with_user_events([
        (
            "user-1",
            vec![sample_event(
                301,
                "User One Existing",
                Some("- 5min 55%".to_string()),
            )],
        ),
        (
            "user-2",
            vec![sample_event(
                401,
                "User Two Existing",
                Some("- 3x3min 120%".to_string()),
            )],
        ),
    ]);
    let app = intervals_test_app(
        SessionMappedIdentityService::with_users([
            ("session-1", "user-1", "athlete1@example.com"),
            ("session-2", "user-2", "athlete2@example.com"),
        ]),
        service,
    )
    .await;

    let request_body = serde_json::json!({
        "category": "WORKOUT",
        "startDateLocal": "2026-03-26",
        "name": "Created For User One",
        "indoor": true,
        "workoutDoc": "- 2x15min 90%"
    });

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/intervals/events")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_response.status(), StatusCode::CREATED);

    let list_user_1 = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/intervals/events?oldest=2026-03-01&newest=2026-03-31")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let list_user_2 = app
        .oneshot(
            Request::builder()
                .uri("/api/intervals/events?oldest=2026-03-01&newest=2026-03-31")
                .header(header::COOKIE, session_cookie("session-2"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let user_1_body: Value = get_json(list_user_1).await;
    let user_2_body: Value = get_json(list_user_2).await;

    assert_eq!(user_1_body.as_array().unwrap().len(), 2);
    assert_eq!(user_2_body.as_array().unwrap().len(), 1);
    assert_eq!(
        user_2_body.as_array().unwrap()[0]
            .get("id")
            .unwrap()
            .as_i64(),
        Some(401)
    );
}

#[tokio::test]
async fn update_event_returns_200() {
    let app = intervals_test_app(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::with_events(vec![sample_event(30, "Old", None)]),
    )
    .await;

    let request_body = serde_json::json!({
        "name": "Updated Workout",
        "indoor": false,
        "workoutDoc": "- 2x20min 90%"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/intervals/events/30")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = get_json(response).await;
    assert_eq!(
        body.get("name").unwrap().as_str().unwrap(),
        "Updated Workout"
    );
}

#[tokio::test]
async fn update_event_is_scoped_to_authenticated_user() {
    let app = intervals_test_app(
        SessionMappedIdentityService::with_users([
            ("session-1", "user-1", "athlete1@example.com"),
            ("session-2", "user-2", "athlete2@example.com"),
        ]),
        ScopedIntervalsService::with_user_events([
            (
                "user-1",
                vec![sample_event(
                    601,
                    "User One Workout",
                    Some("- 5min 55%".to_string()),
                )],
            ),
            (
                "user-2",
                vec![sample_event(
                    602,
                    "User Two Workout",
                    Some("- 4x4min 120%".to_string()),
                )],
            ),
        ]),
    )
    .await;

    let request_body = serde_json::json!({
        "name": "Hijack Attempt",
        "workoutDoc": "- 99min 999w"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/intervals/events/602")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn create_event_rejects_invalid_category() {
    let app = intervals_test_app(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::default(),
    )
    .await;

    let request_body = serde_json::json!({
        "category": "INVALID",
        "startDateLocal": "2026-03-25",
        "name": "Bad",
        "indoor": true
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/intervals/events")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn delete_event_returns_204() {
    let app = intervals_test_app(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::with_events(vec![sample_event(40, "Delete Me", None)]),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/intervals/events/40")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn delete_event_is_scoped_to_authenticated_user() {
    let app = intervals_test_app(
        SessionMappedIdentityService::with_users([
            ("session-1", "user-1", "athlete1@example.com"),
            ("session-2", "user-2", "athlete2@example.com"),
        ]),
        ScopedIntervalsService::with_user_events([
            (
                "user-1",
                vec![sample_event(
                    701,
                    "User One Workout",
                    Some("- 5min 55%".to_string()),
                )],
            ),
            (
                "user-2",
                vec![sample_event(
                    702,
                    "User Two Workout",
                    Some("- 3x3min 120%".to_string()),
                )],
            ),
        ]),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/intervals/events/702")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
