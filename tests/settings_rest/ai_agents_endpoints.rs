use aiwattcoach::domain::settings::UserSettings;
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use serde_json::Value;
use tower::util::ServiceExt;

use super::shared::*;

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
async fn update_ai_agents_persists_meso_cycle_provider_and_model() {
    let app = settings_test_app(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::default(),
    )
    .await;

    let patch_body = serde_json::json!({
        "openrouterApiKey": "or-key-123456",
        "selectedProvider": "openrouter",
        "selectedModel": "openai/gpt-4o-mini",
        "mesoCycleProvider": "gemini",
        "mesoCycleModel": "gemini-2.5-flash"
    });

    let patch_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/settings/ai-agents")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&patch_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(patch_response.status(), StatusCode::OK);

    let get_response = app
        .oneshot(
            Request::builder()
                .uri("/api/settings")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(get_response.status(), StatusCode::OK);
    let response_body: Value = get_json(get_response).await;
    let ai_agents = response_body.get("aiAgents").unwrap();

    assert_eq!(
        ai_agents
            .get("mesoCycleProvider")
            .unwrap()
            .as_str()
            .unwrap(),
        "gemini"
    );
    assert_eq!(
        ai_agents.get("mesoCycleModel").unwrap().as_str().unwrap(),
        "gemini-2.5-flash"
    );
    assert_eq!(
        ai_agents.get("selectedProvider").unwrap().as_str().unwrap(),
        "openrouter"
    );
}

#[tokio::test]
async fn update_ai_agents_persists_post_workout_model_overrides() {
    let app = settings_test_app(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::default(),
    )
    .await;

    let patch_body = serde_json::json!({
        "openrouterApiKey": "or-key-123456",
        "selectedProvider": "openrouter",
        "selectedModel": "openai/gpt-4o-mini",
        "workoutChatProvider": "deepseek",
        "workoutChatModel": "deepseek-v4-flash",
        "workoutPlanningProvider": "gemini",
        "workoutPlanningModel": "gemini-2.5-pro"
    });

    let patch_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/settings/ai-agents")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&patch_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(patch_response.status(), StatusCode::OK);

    let get_response = app
        .oneshot(
            Request::builder()
                .uri("/api/settings")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(get_response.status(), StatusCode::OK);
    let response_body: Value = get_json(get_response).await;
    let ai_agents = response_body.get("aiAgents").unwrap();

    assert_eq!(
        ai_agents
            .get("workoutChatProvider")
            .unwrap()
            .as_str()
            .unwrap(),
        "deepseek"
    );
    assert_eq!(
        ai_agents.get("workoutChatModel").unwrap().as_str().unwrap(),
        "deepseek-v4-flash"
    );
    assert_eq!(
        ai_agents
            .get("workoutPlanningProvider")
            .unwrap()
            .as_str()
            .unwrap(),
        "gemini"
    );
    assert_eq!(
        ai_agents
            .get("workoutPlanningModel")
            .unwrap()
            .as_str()
            .unwrap(),
        "gemini-2.5-pro"
    );
}

#[tokio::test]
async fn update_ai_agents_persists_plan_quality_evaluator_override_max_loops_and_pass_score() {
    let app = settings_test_app(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::default(),
    )
    .await;

    let patch_body = serde_json::json!({
        "openrouterApiKey": "or-key-123456",
        "selectedProvider": "openrouter",
        "selectedModel": "openai/gpt-4o-mini",
        "planQualityEvaluatorProvider": "gemini",
        "planQualityEvaluatorModel": "gemini-2.5-flash",
        "planQualityMaxLoops": 3,
        "planQualityPassScore": 9
    });

    let patch_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/settings/ai-agents")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&patch_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(patch_response.status(), StatusCode::OK);

    let get_response = app
        .oneshot(
            Request::builder()
                .uri("/api/settings")
                .header(header::COOKIE, session_cookie("session-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(get_response.status(), StatusCode::OK);
    let response_body: Value = get_json(get_response).await;
    let ai_agents = response_body.get("aiAgents").unwrap();

    assert_eq!(
        ai_agents
            .get("planQualityEvaluatorProvider")
            .unwrap()
            .as_str()
            .unwrap(),
        "gemini"
    );
    assert_eq!(
        ai_agents
            .get("planQualityEvaluatorModel")
            .unwrap()
            .as_str()
            .unwrap(),
        "gemini-2.5-flash"
    );
    assert_eq!(
        ai_agents
            .get("planQualityMaxLoops")
            .unwrap()
            .as_u64()
            .unwrap(),
        3
    );
    assert_eq!(
        ai_agents
            .get("planQualityPassScore")
            .unwrap()
            .as_u64()
            .unwrap(),
        9
    );
}

#[tokio::test]
async fn update_ai_agents_rejects_plan_quality_max_loops_out_of_range() {
    let app = settings_test_app(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::default(),
    )
    .await;

    let patch_body = serde_json::json!({
        "openrouterApiKey": "or-key-123456",
        "selectedProvider": "openrouter",
        "selectedModel": "openai/gpt-4o-mini",
        "planQualityMaxLoops": 11
    });

    let patch_response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/settings/ai-agents")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&patch_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(patch_response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn update_ai_agents_rejects_plan_quality_pass_score_out_of_range() {
    let app = settings_test_app(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::default(),
    )
    .await;

    let patch_body = serde_json::json!({
        "openrouterApiKey": "or-key-123456",
        "selectedProvider": "openrouter",
        "selectedModel": "openai/gpt-4o-mini",
        "planQualityPassScore": 11
    });

    let patch_response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/settings/ai-agents")
                .header(header::COOKIE, session_cookie("session-1"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&patch_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(patch_response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn update_ai_agents_supports_openai_compatible_provider_key_and_base_url() {
    let app = settings_test_app(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::default(),
    )
    .await;

    let body = serde_json::json!({
        "openaiCompatibleApiKey": "sk-compat-key-789012",
        "openaiCompatibleBaseUrl": "http://127.0.0.1:11434/v1/",
        "selectedProvider": "openai_compatible",
        "selectedModel": "llama3.2"
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
        .get("openaiCompatibleApiKeySet")
        .unwrap()
        .as_bool()
        .unwrap());
    assert_eq!(
        ai_agents
            .get("openaiCompatibleBaseUrl")
            .unwrap()
            .as_str()
            .unwrap(),
        "http://127.0.0.1:11434/v1"
    );
    let masked = ai_agents
        .get("openaiCompatibleApiKey")
        .unwrap()
        .as_str()
        .unwrap();
    assert!(masked.starts_with("***..."));
    assert_eq!(
        ai_agents.get("selectedProvider").unwrap().as_str().unwrap(),
        "openai_compatible"
    );
    assert_eq!(
        ai_agents.get("selectedModel").unwrap().as_str().unwrap(),
        "llama3.2"
    );
}

#[tokio::test]
async fn update_ai_agents_supports_deepseek_provider_and_model() {
    let app = settings_test_app(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::default(),
    )
    .await;

    let body = serde_json::json!({
        "deepseekApiKey": "sk-ds-key-789012",
        "selectedProvider": "deepseek",
        "selectedModel": "deepseek-v4-flash"
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
        .get("deepseekApiKeySet")
        .unwrap()
        .as_bool()
        .unwrap());
    let deepseek_key = ai_agents.get("deepseekApiKey").unwrap().as_str().unwrap();
    assert!(deepseek_key.starts_with("***..."));
    assert_eq!(
        ai_agents.get("selectedProvider").unwrap().as_str().unwrap(),
        "deepseek"
    );
    assert_eq!(
        ai_agents.get("selectedModel").unwrap().as_str().unwrap(),
        "deepseek-v4-flash"
    );
    assert_eq!(
        ai_agents.get("deepseekApiKey").unwrap().as_str().unwrap(),
        "***...9012"
    );
}

#[tokio::test]
async fn update_ai_agents_supports_zai_provider_and_model() {
    let app = settings_test_app(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::default(),
    )
    .await;

    let body = serde_json::json!({
        "zaiApiKey": "sk-zai-key-789012",
        "selectedProvider": "zai",
        "selectedModel": "glm-5.2"
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

    assert!(ai_agents.get("zaiApiKeySet").unwrap().as_bool().unwrap());
    let zai_key = ai_agents.get("zaiApiKey").unwrap().as_str().unwrap();
    assert!(zai_key.starts_with("***..."));
    assert_eq!(
        ai_agents.get("selectedProvider").unwrap().as_str().unwrap(),
        "zai"
    );
    assert_eq!(
        ai_agents.get("selectedModel").unwrap().as_str().unwrap(),
        "glm-5.2"
    );
    assert_eq!(zai_key, "***...9012");
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
async fn test_ai_agents_connection_returns_ok_for_deepseek_settings() {
    let app = settings_test_app_with_services(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::default(),
        None,
        Some(std::sync::Arc::new(MockLlmChatService::returning_ok())),
        Some(std::sync::Arc::new(TestLlmConfigProvider)),
    )
    .await;

    let body = serde_json::json!({
        "deepseekApiKey": "sk-ds-key-789012",
        "selectedProvider": "deepseek",
        "selectedModel": "deepseek-v4-flash"
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
async fn test_ai_agents_connection_returns_ok_for_zai_settings() {
    let app = settings_test_app_with_services(
        TestIdentityServiceWithSession::default(),
        TestSettingsService::default(),
        None,
        Some(std::sync::Arc::new(MockLlmChatService::returning_ok())),
        Some(std::sync::Arc::new(TestLlmConfigProvider)),
    )
    .await;

    let body = serde_json::json!({
        "zaiApiKey": "sk-zai-key-789012",
        "selectedProvider": "zai",
        "selectedModel": "glm-5.2"
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
        deepseek_api_key: None,
        zai_api_key: None,
        openai_compatible_api_key: None,
        openai_compatible_base_url: None,
        selected_provider: Some(aiwattcoach::domain::llm::LlmProvider::OpenAi),
        selected_model: Some("gpt-4o-mini".to_string()),
        workout_chat_provider: None,
        workout_chat_model: None,
        workout_planning_provider: None,
        workout_planning_model: None,
        meso_cycle_provider: None,
        meso_cycle_model: None,
        plan_quality_evaluator_provider: None,
        plan_quality_evaluator_model: None,
        plan_quality_max_loops: None,
        plan_quality_pass_score: None,
        include_power_image: false,
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
