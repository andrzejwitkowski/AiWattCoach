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
    assert!(body.get("cycling").is_some());

    let ai_agents = body.get("aiAgents").unwrap();
    assert!(!ai_agents.get("openaiApiKeySet").unwrap().as_bool().unwrap());
    assert!(!ai_agents.get("geminiApiKeySet").unwrap().as_bool().unwrap());

    let intervals = body.get("intervals").unwrap();
    assert!(!intervals.get("connected").unwrap().as_bool().unwrap());

    let options = body.get("options").unwrap();
    assert!(!options
        .get("analyzeWithoutHeartRate")
        .unwrap()
        .as_bool()
        .unwrap());
}

#[tokio::test]
async fn get_settings_masks_api_keys() {
    let settings = UserSettings::new_defaults("user-1".to_string(), 1000);
    let mut settings_with_keys = settings;
    settings_with_keys.ai_agents.openai_api_key = Some("sk-verysecretkey1234".to_string());

    let app = settings_test_app(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::with_settings(settings_with_keys),
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
    let ai_agents = body.get("aiAgents").unwrap();

    assert_eq!(
        ai_agents.get("openaiApiKey").unwrap().as_str().unwrap(),
        "***...1234"
    );
    assert!(ai_agents.get("openaiApiKeySet").unwrap().as_bool().unwrap());
    assert!(
        ai_agents.get("geminiApiKey").is_none() || ai_agents.get("geminiApiKey").unwrap().is_null()
    );
}

#[tokio::test]
async fn update_ai_agents_saves_and_returns_updated_settings() {
    let app = settings_test_app(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::default(),
    )
    .await;

    let body = serde_json::json!({
        "openaiApiKey": "sk-new-openai-key",
        "geminiApiKey": "AIza-new-gemini-key"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/settings/ai-agents")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let response_body: Value = get_json(response).await;
    let ai_agents = response_body.get("aiAgents").unwrap();

    let openai_masked = ai_agents.get("openaiApiKey").unwrap().as_str().unwrap();
    let gemini_masked = ai_agents.get("geminiApiKey").unwrap().as_str().unwrap();

    assert!(openai_masked.starts_with("***..."));
    assert!(gemini_masked.starts_with("***..."));
    assert!(!gemini_masked.ends_with("ey-1"));
    assert!(ai_agents.get("openaiApiKeySet").unwrap().as_bool().unwrap());
    assert!(ai_agents.get("geminiApiKeySet").unwrap().as_bool().unwrap());
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
async fn update_intervals_keeps_saved_credentials_when_blank_values_are_sent() {
    let mut settings = UserSettings::new_defaults("user-1".to_string(), 1000);
    settings.intervals.api_key = Some("saved-api-key".to_string());
    settings.intervals.athlete_id = Some("saved-athlete-id".to_string());
    settings.intervals.connected = true;

    let app = settings_test_app(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::with_settings(settings),
    )
    .await;

    let body = serde_json::json!({
        "apiKey": "   ",
        "athleteId": "   "
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

    assert!(intervals.get("apiKeySet").unwrap().as_bool().unwrap());
    assert_eq!(
        intervals.get("athleteId").unwrap().as_str().unwrap(),
        "saved-athlete-id"
    );
    assert!(intervals.get("connected").unwrap().as_bool().unwrap());
}

#[tokio::test]
async fn update_intervals_resets_connected_when_credentials_change() {
    let mut settings = UserSettings::new_defaults("user-1".to_string(), 1000);
    settings.intervals.api_key = Some("saved-api-key".to_string());
    settings.intervals.athlete_id = Some("saved-athlete-id".to_string());
    settings.intervals.connected = true;

    let app = settings_test_app(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::with_settings(settings),
    )
    .await;

    let body = serde_json::json!({
        "apiKey": "updated-api-key"
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

    assert!(!intervals.get("connected").unwrap().as_bool().unwrap());
    assert_eq!(
        intervals.get("athleteId").unwrap().as_str().unwrap(),
        "saved-athlete-id"
    );
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
async fn update_ai_agents_partial_body_preserves_existing_key() {
    let existing_settings = UserSettings::new_defaults("user-1".to_string(), 1000);
    let mut with_existing_keys = existing_settings;
    with_existing_keys.ai_agents.openai_api_key = Some("sk-existing-openai".to_string());
    with_existing_keys.ai_agents.gemini_api_key = Some("AIza-existing-gemini".to_string());

    let app = settings_test_app(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::with_settings(with_existing_keys),
    )
    .await;

    let body = serde_json::json!({
        "openaiApiKey": "sk-new-openai-key"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/settings/ai-agents")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let response_body: Value = get_json(response).await;
    let ai_agents = response_body.get("aiAgents").unwrap();

    assert!(ai_agents.get("openaiApiKeySet").unwrap().as_bool().unwrap());
    assert!(ai_agents.get("geminiApiKeySet").unwrap().as_bool().unwrap());
    let gemini_key = ai_agents.get("geminiApiKey").unwrap().as_str().unwrap();
    assert!(gemini_key.starts_with("***..."));
}

use aiwattcoach::domain::settings::UserSettings;
