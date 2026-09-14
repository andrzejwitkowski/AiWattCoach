use aiwattcoach::domain::settings::UserSettings;
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use serde_json::Value;
use tower::util::ServiceExt;

use crate::shared::{
    get_json, session_cookie, settings_test_app, TestIdentityServiceWithSession,
    TestSettingsService,
};

#[tokio::test]
async fn get_settings_requires_authentication() {
    let app = settings_test_app(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::default(),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/settings")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn get_settings_returns_default_settings_for_authenticated_user() {
    let app = settings_test_app(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::default(),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/settings")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = get_json(response).await;
    assert!(body.get("aiAgents").is_some());
    assert!(body.get("intervals").is_some());
    assert!(body.get("options").is_some());
    assert!(body.get("availability").is_some());
    assert!(body.get("cycling").is_some());

    let ai_agents = body.get("aiAgents").unwrap();
    assert!(!ai_agents.get("openaiApiKeySet").unwrap().as_bool().unwrap());
    assert!(!ai_agents.get("geminiApiKeySet").unwrap().as_bool().unwrap());
    assert!(!ai_agents
        .get("deepseekApiKeySet")
        .unwrap()
        .as_bool()
        .unwrap());
    assert!(!ai_agents.get("zaiApiKeySet").unwrap().as_bool().unwrap());
    assert!(!ai_agents
        .get("openaiCompatibleApiKeySet")
        .unwrap()
        .as_bool()
        .unwrap());
    assert!(ai_agents.get("openaiCompatibleBaseUrl").unwrap().is_null());

    let intervals = body.get("intervals").unwrap();
    assert!(!intervals.get("connected").unwrap().as_bool().unwrap());

    let wahoo = body.get("wahoo").unwrap();
    assert!(!wahoo.get("available").unwrap().as_bool().unwrap());
    assert!(!wahoo.get("connected").unwrap().as_bool().unwrap());

    let options = body.get("options").unwrap();
    assert!(!options
        .get("analyzeWithoutHeartRate")
        .unwrap()
        .as_bool()
        .unwrap());

    let availability = body.get("availability").unwrap();
    assert!(!availability.get("configured").unwrap().as_bool().unwrap());
    assert_eq!(
        availability.get("days").unwrap().as_array().unwrap().len(),
        7
    );
}

#[tokio::test]
async fn update_intervals_saves_athlete_id() {
    let app = settings_test_app(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::default(),
    )
    .await;

    let body = serde_json::json!({
        "apiKey": "intervals-api-key-xyz",
        "athleteId": "i12345678"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/settings/intervals")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let response_body: Value = get_json(response).await;
    let intervals = response_body.get("intervals").unwrap();

    assert_eq!(
        intervals.get("athleteId").unwrap().as_str().unwrap(),
        "i12345678"
    );
    assert!(intervals.get("apiKeySet").unwrap().as_bool().unwrap());
}

#[tokio::test]
async fn update_intervals_preserves_saved_credentials_when_fields_are_missing() {
    let mut settings = UserSettings::new_defaults("user-1".to_string(), 1000);
    settings.intervals.api_key = Some("saved-api-key".to_string());
    settings.intervals.athlete_id = Some("saved-athlete-id".to_string());
    settings.intervals.connected = true;

    let app = settings_test_app(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::with_settings(settings),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/settings/intervals")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let response_body: Value = get_json(response).await;
    let intervals = response_body.get("intervals").unwrap();

    assert!(intervals.get("apiKeySet").unwrap().as_bool().unwrap());
    assert_eq!(
        intervals.get("athleteId").unwrap().as_str().unwrap(),
        "saved-athlete-id"
    );
    assert!(intervals.get("connected").unwrap().as_bool().unwrap());
}

#[tokio::test]
async fn update_intervals_keeps_saved_credentials_disconnected_until_retested() {
    let mut settings = UserSettings::new_defaults("user-1".to_string(), 1000);
    settings.intervals.api_key = Some("saved-api-key".to_string());
    settings.intervals.athlete_id = Some("saved-athlete-id".to_string());
    settings.intervals.connected = false;

    let app = settings_test_app(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::with_settings(settings),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/settings/intervals")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let response_body: Value = get_json(response).await;
    let intervals = response_body.get("intervals").unwrap();

    assert!(intervals.get("apiKeySet").unwrap().as_bool().unwrap());
    assert_eq!(
        intervals.get("athleteId").unwrap().as_str().unwrap(),
        "saved-athlete-id"
    );
    assert!(!intervals.get("connected").unwrap().as_bool().unwrap());
}

#[tokio::test]
async fn update_intervals_does_not_activate_incomplete_saved_credentials() {
    let mut settings = UserSettings::new_defaults("user-1".to_string(), 1000);
    settings.intervals.api_key = Some("saved-api-key".to_string());
    settings.intervals.athlete_id = None;
    settings.intervals.connected = true;

    let app = settings_test_app(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::with_settings(settings),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/settings/intervals")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let response_body: Value = get_json(response).await;
    let intervals = response_body.get("intervals").unwrap();

    assert!(intervals.get("apiKeySet").unwrap().as_bool().unwrap());
    assert!(intervals.get("athleteId").is_some_and(Value::is_null));
    assert!(!intervals.get("connected").unwrap().as_bool().unwrap());
}

#[tokio::test]
async fn update_options_sets_analyze_without_heart_rate() {
    let app = settings_test_app(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::default(),
    )
    .await;

    let body = serde_json::json!({
        "analyzeWithoutHeartRate": true
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/settings/options")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let response_body: Value = get_json(response).await;
    let options = response_body.get("options").unwrap();

    assert!(options
        .get("analyzeWithoutHeartRate")
        .unwrap()
        .as_bool()
        .unwrap());
}

#[tokio::test]
async fn update_availability_saves_explicit_week_structure() {
    let app = settings_test_app(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::default(),
    )
    .await;

    let body = serde_json::json!({
        "days": [
            { "weekday": "mon", "available": true, "maxDurationMinutes": 60 },
            { "weekday": "tue", "available": false, "maxDurationMinutes": null },
            { "weekday": "wed", "available": true, "maxDurationMinutes": 90 },
            { "weekday": "thu", "available": false, "maxDurationMinutes": null },
            { "weekday": "fri", "available": true, "maxDurationMinutes": 120 },
            { "weekday": "sat", "available": true, "maxDurationMinutes": 180 },
            { "weekday": "sun", "available": false, "maxDurationMinutes": null }
        ]
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/settings/availability")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let response_body: Value = get_json(response).await;
    let availability = response_body.get("availability").unwrap();
    assert!(availability.get("configured").unwrap().as_bool().unwrap());
    let days = availability.get("days").unwrap().as_array().unwrap();
    assert_eq!(days.len(), 7);
    assert_eq!(days[0].get("weekday").unwrap().as_str().unwrap(), "mon");
    assert_eq!(
        days[0].get("maxDurationMinutes").unwrap().as_u64().unwrap(),
        60
    );
    assert!(days[1]
        .get("maxDurationMinutes")
        .is_some_and(Value::is_null));
}

#[tokio::test]
async fn update_availability_requires_days_payload() {
    let app = settings_test_app(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::default(),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/settings/availability")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn update_cycling_saves_biometrics() {
    let app = settings_test_app(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::default(),
    )
    .await;

    let body = serde_json::json!({
        "fullName": "Alex Rivier",
        "age": 28,
        "heightCm": 182,
        "weightKg": 74.0,
        "ftpWatts": 280,
        "hrMaxBpm": 192,
        "vo2Max": 58.0
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/settings/cycling")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let response_body: Value = get_json(response).await;
    let cycling = response_body.get("cycling").unwrap();

    assert_eq!(
        cycling.get("fullName").unwrap().as_str().unwrap(),
        "Alex Rivier"
    );
    assert_eq!(cycling.get("age").unwrap().as_i64().unwrap(), 28);
    assert_eq!(cycling.get("heightCm").unwrap().as_i64().unwrap(), 182);
    assert_eq!(cycling.get("weightKg").unwrap().as_f64().unwrap(), 74.0);
    assert_eq!(cycling.get("ftpWatts").unwrap().as_i64().unwrap(), 280);
    assert_eq!(cycling.get("hrMaxBpm").unwrap().as_i64().unwrap(), 192);
    assert_eq!(cycling.get("vo2Max").unwrap().as_f64().unwrap(), 58.0);
}

#[tokio::test]
async fn update_cycling_trims_and_clears_full_name() {
    let mut settings = UserSettings::new_defaults("user-1".to_string(), 1000);
    settings.cycling.full_name = Some("Saved Name".to_string());

    let app = settings_test_app(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::with_settings(settings),
    )
    .await;

    let trimmed_body = serde_json::json!({
        "fullName": "  Alex Rivier  "
    });

    let trimmed_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/settings/cycling")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&trimmed_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(trimmed_response.status(), StatusCode::OK);

    let trimmed_response_body: Value = get_json(trimmed_response).await;
    assert_eq!(
        trimmed_response_body
            .get("cycling")
            .unwrap()
            .get("fullName")
            .unwrap()
            .as_str()
            .unwrap(),
        "Alex Rivier"
    );

    let clear_body = serde_json::json!({
        "fullName": "   "
    });

    let clear_response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/settings/cycling")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&clear_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(clear_response.status(), StatusCode::OK);

    let clear_response_body: Value = get_json(clear_response).await;
    assert!(clear_response_body
        .get("cycling")
        .unwrap()
        .get("fullName")
        .is_some_and(Value::is_null));
}

#[tokio::test]
async fn update_cycling_saves_training_context_profile_fields() {
    let app = settings_test_app(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::default(),
    )
    .await;

    let body = serde_json::json!({
        "athletePrompt": "  Climbing specialist preparing for stage races.  ",
        "medications": "  Iron supplement  ",
        "athleteNotes": "  Responds poorly to back-to-back VO2 days.  "
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/settings/cycling")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let response_body: Value = get_json(response).await;
    let cycling = response_body.get("cycling").unwrap();

    assert_eq!(
        cycling.get("athletePrompt").unwrap().as_str().unwrap(),
        "Climbing specialist preparing for stage races."
    );
    assert_eq!(
        cycling.get("medications").unwrap().as_str().unwrap(),
        "Iron supplement"
    );
    assert_eq!(
        cycling.get("athleteNotes").unwrap().as_str().unwrap(),
        "Responds poorly to back-to-back VO2 days."
    );
}

#[tokio::test]
async fn update_cycling_clears_training_context_profile_fields() {
    let mut settings = UserSettings::new_defaults("user-1".to_string(), 1000);
    settings.cycling.athlete_prompt = Some("saved athlete prompt".to_string());
    settings.cycling.medications = Some("saved medication".to_string());
    settings.cycling.athlete_notes = Some("saved athlete note".to_string());

    let app = settings_test_app(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::with_settings(settings),
    )
    .await;

    let body = serde_json::json!({
        "athletePrompt": "   ",
        "medications": null,
        "athleteNotes": ""
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/settings/cycling")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let response_body: Value = get_json(response).await;
    let cycling = response_body.get("cycling").unwrap();

    assert!(cycling.get("athletePrompt").is_some_and(Value::is_null));
    assert!(cycling.get("medications").is_some_and(Value::is_null));
    assert!(cycling.get("athleteNotes").is_some_and(Value::is_null));
}
