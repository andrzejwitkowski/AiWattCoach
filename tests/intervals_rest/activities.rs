use aiwattcoach::domain::intervals::UploadedActivities;
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use serde_json::Value;
use tower::util::ServiceExt;

use crate::{
    app::intervals_test_app,
    fixtures::{get_json, sample_activity, session_cookie},
    identity_fakes::TestIdentityServiceWithSession,
    intervals_fakes::TestIntervalsService,
};

#[tokio::test]
async fn list_activities_returns_activities_for_authenticated_user() {
    let app = intervals_test_app(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::with_activities(vec![sample_activity("i11", "Morning Ride")]),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/intervals/activities?oldest=2026-03-01&newest=2026-03-31")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = get_json(response).await;
    let activity = &body.as_array().unwrap()[0];
    assert_eq!(activity.get("id").unwrap().as_str(), Some("i11"));
    assert_eq!(
        activity
            .get("metrics")
            .unwrap()
            .get("normalizedPowerWatts")
            .unwrap()
            .as_i64(),
        Some(238)
    );
}

#[tokio::test]
async fn get_activity_returns_detailed_activity() {
    let app = intervals_test_app(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::with_activities(vec![sample_activity("i21", "Detailed Ride")]),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/intervals/activities/i21")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = get_json(response).await;
    assert_eq!(body.get("name").unwrap().as_str(), Some("Detailed Ride"));
    assert_eq!(
        body.get("details")
            .unwrap()
            .get("intervals")
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn create_activity_returns_201_and_uploaded_activities() {
    let app = intervals_test_app(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::with_uploaded_activities(UploadedActivities {
            created: true,
            activity_ids: vec!["i31".to_string()],
            activities: vec![sample_activity("i31", "Uploaded Ride")],
        }),
    )
    .await;

    let request_body = serde_json::json!({
        "filename": "ride.fit",
        "fileContentsBase64": "AQID",
        "name": "Uploaded Ride",
        "pairedEventId": 9
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/intervals/activities")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let body: Value = get_json(response).await;
    assert_eq!(body.get("created").unwrap().as_bool(), Some(true));
    assert_eq!(
        body.get("activityIds").unwrap().as_array().unwrap()[0].as_str(),
        Some("i31")
    );
}

#[tokio::test]
async fn create_activity_returns_200_when_duplicate_upload_is_detected() {
    let app = intervals_test_app(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::with_uploaded_activities(UploadedActivities {
            created: false,
            activity_ids: vec!["i31".to_string()],
            activities: vec![sample_activity("i31", "Existing Ride")],
        }),
    )
    .await;

    let request_body = serde_json::json!({
        "filename": "ride.fit",
        "fileContentsBase64": "AQID",
        "name": "Existing Ride"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/intervals/activities")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = get_json(response).await;
    assert_eq!(body.get("created").unwrap().as_bool(), Some(false));
    assert_eq!(
        body.get("activityIds")
            .unwrap()
            .as_array()
            .unwrap()
            .first()
            .unwrap()
            .as_str(),
        Some("i31")
    );
}

#[tokio::test]
async fn create_activity_rejects_invalid_base64_payload() {
    let app = intervals_test_app(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::default(),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/intervals/activities")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "filename": "ride.fit",
                        "fileContentsBase64": "AA=A"
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
async fn update_activity_returns_200() {
    let app = intervals_test_app(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::with_activities(vec![sample_activity("i41", "Old Ride")]),
    )
    .await;

    let request_body = serde_json::json!({
        "name": "Updated Ride",
        "activityType": "VirtualRide",
        "trainer": true
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/intervals/activities/i41")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = get_json(response).await;
    assert_eq!(body.get("name").unwrap().as_str(), Some("Updated Ride"));
    assert_eq!(body.get("trainer").unwrap().as_bool(), Some(true));
}

#[tokio::test]
async fn delete_activity_returns_204() {
    let app = intervals_test_app(
        TestIdentityServiceWithSession::default(),
        TestIntervalsService::with_activities(vec![sample_activity("i51", "Delete Me")]),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/intervals/activities/i51")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}
