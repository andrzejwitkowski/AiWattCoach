use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use serde_json::Value;
use tower::util::ServiceExt;

use crate::shared::{
    get_json, session_cookie, settings_test_app, settings_test_app_with_services,
    MockLlmChatService, TestIdentityServiceWithSession, TestLlmConfigProvider, TestSettingsService,
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

    let intervals = body.get("intervals").unwrap();
    assert!(!intervals.get("connected").unwrap().as_bool().unwrap());

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
async fn update_intervals_clears_credentials_when_blank_values_are_sent() {
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

    assert!(!intervals.get("apiKeySet").unwrap().as_bool().unwrap());
    assert!(intervals.get("apiKey").is_none_or(|value| value.is_null()));
    assert!(intervals.get("athleteId").is_some_and(Value::is_null));
    assert!(!intervals.get("connected").unwrap().as_bool().unwrap());
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
async fn update_intervals_keeps_connection_active_when_complete_credentials_are_saved() {
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

    assert!(intervals.get("connected").unwrap().as_bool().unwrap());
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
async fn update_availability_derives_not_configured_when_all_days_are_unavailable() {
    let app = settings_test_app(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::default(),
    )
    .await;

    let body = serde_json::json!({
        "days": [
            { "weekday": "mon", "available": false, "maxDurationMinutes": null },
            { "weekday": "tue", "available": false, "maxDurationMinutes": null },
            { "weekday": "wed", "available": false, "maxDurationMinutes": null },
            { "weekday": "thu", "available": false, "maxDurationMinutes": null },
            { "weekday": "fri", "available": false, "maxDurationMinutes": null },
            { "weekday": "sat", "available": false, "maxDurationMinutes": null },
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
    assert!(!availability.get("configured").unwrap().as_bool().unwrap());
}

#[tokio::test]
async fn update_availability_rejects_invalid_duration_step() {
    let app = settings_test_app(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::default(),
    )
    .await;

    let body = serde_json::json!({
        "days": [
            { "weekday": "mon", "available": true, "maxDurationMinutes": 45 },
            { "weekday": "tue", "available": false, "maxDurationMinutes": null },
            { "weekday": "wed", "available": false, "maxDurationMinutes": null },
            { "weekday": "thu", "available": false, "maxDurationMinutes": null },
            { "weekday": "fri", "available": false, "maxDurationMinutes": null },
            { "weekday": "sat", "available": false, "maxDurationMinutes": null },
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

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body: Value = get_json(response).await;
    assert_eq!(
        body.get("message").and_then(Value::as_str),
        Some("availability duration 45 is invalid; expected one of [30, 60, 90, 120, 150, 180, 210, 240, 270, 300]")
    );
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

#[tokio::test]
async fn update_ai_agents_supports_openrouter_provider_and_model() {
    let app = settings_test_app(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::default(),
    )
    .await;

    let body = serde_json::json!({
        "openrouterApiKey": "or-key-123456",
        "selectedProvider": "openrouter",
        "selectedModel": "openai/gpt-4o-mini"
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

    assert!(ai_agents
        .get("openrouterApiKeySet")
        .unwrap()
        .as_bool()
        .unwrap());
    let openrouter_key = ai_agents.get("openrouterApiKey").unwrap().as_str().unwrap();
    assert!(openrouter_key.starts_with("***..."));
    assert_eq!(
        ai_agents.get("selectedProvider").unwrap().as_str().unwrap(),
        "openrouter"
    );
    assert_eq!(
        ai_agents.get("selectedModel").unwrap().as_str().unwrap(),
        "openai/gpt-4o-mini"
    );
    assert_eq!(
        ai_agents.get("openrouterApiKey").unwrap().as_str().unwrap(),
        "***...3456"
    );
}

#[tokio::test]
async fn update_ai_agents_requires_model_when_provider_changes() {
    let mut settings = UserSettings::new_defaults("user-1".to_string(), 1000);
    settings.ai_agents.selected_provider = Some(aiwattcoach::domain::llm::LlmProvider::OpenAi);
    settings.ai_agents.selected_model = Some("gpt-4o-mini".to_string());
    settings.ai_agents.openrouter_api_key = Some("or-saved-key-3456".to_string());

    let app = settings_test_app(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::with_settings(settings),
    )
    .await;

    let body = serde_json::json!({
        "selectedProvider": "openrouter"
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

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let response_body: Value = get_json(response).await;
    assert_eq!(
        response_body.get("message").unwrap().as_str().unwrap(),
        "selectedModel must not be empty"
    );
}

#[tokio::test]
async fn update_ai_agents_rejects_invalid_provider() {
    let app = settings_test_app(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::default(),
    )
    .await;

    let body = serde_json::json!({
        "selectedProvider": "unknown"
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

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn update_ai_agents_explicitly_clears_provider_model_and_openrouter_key() {
    let mut existing_settings = UserSettings::new_defaults("user-1".to_string(), 1000);
    existing_settings.ai_agents.openrouter_api_key = Some("or-key-123456".to_string());
    existing_settings.ai_agents.selected_provider =
        Some(aiwattcoach::domain::llm::LlmProvider::OpenRouter);
    existing_settings.ai_agents.selected_model = Some("openai/gpt-4o-mini".to_string());

    let app = settings_test_app(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::with_settings(existing_settings),
    )
    .await;

    let body = serde_json::json!({
        "openrouterApiKey": null,
        "selectedProvider": "   ",
        "selectedModel": null
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

    assert!(!ai_agents
        .get("openrouterApiKeySet")
        .unwrap()
        .as_bool()
        .unwrap());
    assert!(ai_agents
        .get("openrouterApiKey")
        .is_none_or(|value| value.is_null()));
    assert!(ai_agents
        .get("selectedProvider")
        .is_none_or(|value| value.is_null()));
    assert!(ai_agents
        .get("selectedModel")
        .is_none_or(|value| value.is_null()));
}

#[tokio::test]
async fn test_ai_agents_connection_returns_ok_for_valid_provider_settings() {
    let app = settings_test_app_with_services(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::default(),
        None,
        Some(std::sync::Arc::new(MockLlmChatService::returning_ok())),
        Some(std::sync::Arc::new(TestLlmConfigProvider)),
    )
    .await;

    let body = serde_json::json!({
        "openrouterApiKey": "or-key-123456",
        "selectedProvider": "openrouter",
        "selectedModel": "openai/gpt-4o-mini"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/settings/ai-agents/test")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let response_body: Value = get_json(response).await;
    assert!(response_body.get("connected").unwrap().as_bool().unwrap());
    assert_eq!(
        response_body.get("message").unwrap().as_str().unwrap(),
        "Connection successful."
    );
}

#[tokio::test]
async fn test_ai_agents_connection_returns_unauthorized_for_missing_auth() {
    let app = settings_test_app_with_services(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::default(),
        None,
        Some(std::sync::Arc::new(MockLlmChatService::returning_ok())),
        Some(std::sync::Arc::new(TestLlmConfigProvider)),
    )
    .await;

    let body = serde_json::json!({
        "openrouterApiKey": "or-key-123456",
        "selectedProvider": "openrouter",
        "selectedModel": "openai/gpt-4o-mini"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/settings/ai-agents/test")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_ai_agents_connection_explicit_clear_does_not_fall_back_to_saved_values() {
    let mut existing_settings = UserSettings::new_defaults("user-1".to_string(), 1000);
    existing_settings.ai_agents.openrouter_api_key = Some("or-key-123456".to_string());
    existing_settings.ai_agents.selected_provider =
        Some(aiwattcoach::domain::llm::LlmProvider::OpenRouter);
    existing_settings.ai_agents.selected_model = Some("openai/gpt-4o-mini".to_string());

    let app = settings_test_app_with_services(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::with_settings(existing_settings),
        None,
        Some(std::sync::Arc::new(MockLlmChatService::returning_ok())),
        Some(std::sync::Arc::new(TestLlmConfigProvider)),
    )
    .await;

    let body = serde_json::json!({
        "selectedProvider": null,
        "selectedModel": "   "
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/settings/ai-agents/test")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let response_body: Value = get_json(response).await;
    assert!(!response_body.get("connected").unwrap().as_bool().unwrap());
    assert_eq!(
        response_body.get("message").unwrap().as_str().unwrap(),
        "Provider, model, and matching API key are required."
    );
    assert!(!response_body
        .get("usedSavedProvider")
        .unwrap()
        .as_bool()
        .unwrap());
    assert!(!response_body
        .get("usedSavedModel")
        .unwrap()
        .as_bool()
        .unwrap());
    assert!(!response_body
        .get("usedSavedApiKey")
        .unwrap()
        .as_bool()
        .unwrap());
}

#[tokio::test]
async fn test_ai_agents_connection_returns_bad_request_for_provider_error() {
    let app = settings_test_app_with_services(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::default(),
        None,
        Some(std::sync::Arc::new(MockLlmChatService::returning_err(
            aiwattcoach::domain::llm::LlmError::ProviderRejected("invalid model".to_string()),
        ))),
        Some(std::sync::Arc::new(TestLlmConfigProvider)),
    )
    .await;

    let body = serde_json::json!({
        "openaiApiKey": "sk-test-key",
        "selectedProvider": "openai",
        "selectedModel": "bad-model"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/settings/ai-agents/test")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_ai_agents_connection_reuses_model_validation_rules() {
    let app = settings_test_app_with_services(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::default(),
        None,
        Some(std::sync::Arc::new(MockLlmChatService::returning_ok())),
        Some(std::sync::Arc::new(TestLlmConfigProvider)),
    )
    .await;

    let body = serde_json::json!({
        "openaiApiKey": "sk-test-key",
        "selectedProvider": "openai",
        "selectedModel": "x".repeat(201)
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/settings/ai-agents/test")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let response_body: Value = get_json(response).await;
    assert_eq!(
        response_body.get("message").unwrap().as_str().unwrap(),
        "selectedModel must be 200 characters or fewer"
    );
    assert!(!response_body
        .get("usedSavedApiKey")
        .unwrap()
        .as_bool()
        .unwrap());
    assert!(!response_body
        .get("usedSavedProvider")
        .unwrap()
        .as_bool()
        .unwrap());
    assert!(!response_body
        .get("usedSavedModel")
        .unwrap()
        .as_bool()
        .unwrap());
}

#[tokio::test]
async fn test_ai_agents_connection_returns_service_unavailable_for_timeout() {
    let app = settings_test_app_with_services(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::default(),
        None,
        Some(std::sync::Arc::new(MockLlmChatService::returning_err(
            aiwattcoach::domain::llm::LlmError::Transport(
                "LLM request timed out after 180 seconds".to_string(),
            ),
        ))),
        Some(std::sync::Arc::new(TestLlmConfigProvider)),
    )
    .await;

    let body = serde_json::json!({
        "openaiApiKey": "sk-test-key",
        "selectedProvider": "openai",
        "selectedModel": "o1-mini"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/settings/ai-agents/test")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

    let response_body: Value = get_json(response).await;
    assert_eq!(
        response_body.get("message").unwrap().as_str().unwrap(),
        "LLM request timed out after 180 seconds"
    );
}

#[tokio::test]
async fn test_ai_agents_connection_returns_service_unavailable_for_invalid_response_errors() {
    let app = settings_test_app_with_services(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::default(),
        None,
        Some(std::sync::Arc::new(MockLlmChatService::returning_err(
            aiwattcoach::domain::llm::LlmError::InvalidResponse(
                "provider returned malformed payload".to_string(),
            ),
        ))),
        Some(std::sync::Arc::new(TestLlmConfigProvider)),
    )
    .await;

    let body = serde_json::json!({
        "openaiApiKey": "sk-test-key",
        "selectedProvider": "openai",
        "selectedModel": "gpt-5.4"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/settings/ai-agents/test")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn test_ai_agents_connection_returns_bad_request_when_provider_changes_without_model() {
    let mut existing_settings = UserSettings::new_defaults("user-1".to_string(), 1000);
    existing_settings.ai_agents = aiwattcoach::domain::settings::AiAgentsConfig {
        openai_api_key: Some("sk-existing-openai".to_string()),
        gemini_api_key: None,
        openrouter_api_key: Some("or-existing-openrouter".to_string()),
        selected_provider: Some(aiwattcoach::domain::llm::LlmProvider::OpenAi),
        selected_model: Some("gpt-4o-mini".to_string()),
    };
    let app = settings_test_app_with_services(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::with_settings(existing_settings),
        None,
        Some(std::sync::Arc::new(MockLlmChatService::returning_ok())),
        Some(std::sync::Arc::new(TestLlmConfigProvider)),
    )
    .await;

    let body = serde_json::json!({
        "selectedProvider": "openrouter"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/settings/ai-agents/test")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let response_body: Value = get_json(response).await;
    assert_eq!(
        response_body.get("message").unwrap().as_str().unwrap(),
        "Provider, model, and matching API key are required."
    );
    assert!(!response_body
        .get("usedSavedProvider")
        .unwrap()
        .as_bool()
        .unwrap());
    assert!(!response_body
        .get("usedSavedModel")
        .unwrap()
        .as_bool()
        .unwrap());
}

#[tokio::test]
async fn test_ai_agents_connection_requires_authentication() {
    let app = settings_test_app_with_services(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::default(),
        None,
        Some(std::sync::Arc::new(MockLlmChatService::returning_ok())),
        Some(std::sync::Arc::new(TestLlmConfigProvider)),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/settings/ai-agents/test")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

use aiwattcoach::domain::settings::UserSettings;
