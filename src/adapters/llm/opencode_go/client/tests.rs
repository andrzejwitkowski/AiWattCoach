use super::*;

fn config(model: &str) -> LlmProviderConfig {
    LlmProviderConfig {
        provider: crate::domain::llm::LlmProvider::OpenCodeGo,
        model: model.to_string(),
        api_key: "test-key".to_string(),
        base_url: None,
    }
}

#[test]
fn responses_text_maps_metadata_and_usage() {
    let response = map_response(
        Protocol::Responses,
        &config("gpt-5.6-luna"),
        json!({
            "id": "resp-1",
            "model": "gpt-5.6-luna",
            "status": "completed",
            "output_text": "OK",
            "output": [{"type":"message","content":[{"type":"output_text","text":"OK"}]}],
            "usage": {"input_tokens": 7, "output_tokens": 2, "total_tokens": 9}
        }),
    )
    .unwrap();

    assert_eq!(response.assistant_text(), Some("OK"));
    assert_eq!(response.provider_request_id.as_deref(), Some("resp-1"));
    assert_eq!(response.model, "gpt-5.6-luna");
    assert_eq!(response.usage.input_tokens, Some(7));
    assert_eq!(response.usage.output_tokens, Some(2));
    assert_eq!(response.usage.total_tokens, Some(9));
    assert_eq!(response.finish_reason, Some(LlmFinishReason::Stop));
}

#[test]
fn responses_output_text_is_valid_without_expanded_output_items() {
    let response = map_response(
        Protocol::Responses,
        &config("gpt-5.6-luna"),
        json!({
            "id": "resp-text-only",
            "model": "gpt-5.6-luna",
            "status": "completed",
            "output_text": "OK"
        }),
    )
    .unwrap();
    assert_eq!(response.assistant_text(), Some("OK"));
}

#[test]
fn responses_function_call_is_not_treated_as_empty_and_round_trips() {
    let response = map_response(
        Protocol::Responses,
        &config("gpt-5.6-luna"),
        json!({
            "id": "resp-call",
            "model": "gpt-5.6-luna",
            "status": "completed",
            "output": [{"type":"function_call","call_id":"call-1","name":"lookup","arguments":"{\"id\":1}"}]
        }),
    )
    .unwrap();
    assert_eq!(response.tool_calls()[0].id, "call-1");
    assert_eq!(response.tool_calls()[0].name, "lookup");
    assert_eq!(response.finish_reason, Some(LlmFinishReason::ToolCalls));

    let request = LlmChatRequest {
        conversation: vec![response.message, LlmChatMessage::tool("call-1", "found")],
        ..Default::default()
    };
    let body = map_request(Protocol::Responses, &config("gpt-5.6-luna"), &request).unwrap();
    assert_eq!(body["input"][0]["type"], "function_call");
    assert_eq!(body["input"][1]["type"], "function_call_output");
    assert_eq!(body["input"][1]["call_id"], "call-1");
}

#[test]
fn responses_rejects_empty_output() {
    let error = map_response(
        Protocol::Responses,
        &config("gpt-5.6-luna"),
        json!({"output": [], "status": "completed"}),
    )
    .unwrap_err();
    assert!(matches!(error, LlmError::InvalidResponse(_)));
}

#[test]
fn messages_tool_calls_preserve_provider_content_for_next_round() {
    let response = map_response(
        Protocol::Messages,
        &config("minimax-m3"),
        json!({
            "id": "msg-1",
            "model": "minimax-m3",
            "content": [{"type":"tool_use","id":"tool-1","name":"lookup","input":{"id":1}}],
            "stop_reason": "tool_use"
        }),
    )
    .unwrap();
    let request = LlmChatRequest {
        conversation: vec![response.message, LlmChatMessage::tool("tool-1", "found")],
        ..Default::default()
    };
    let body = map_request(Protocol::Messages, &config("minimax-m3"), &request).unwrap();
    assert_eq!(body["messages"][0]["role"], "assistant");
    assert_eq!(body["messages"][0]["content"][0]["id"], "tool-1");
    assert_eq!(body["messages"][1]["content"][0]["tool_use_id"], "tool-1");
}

#[test]
fn messages_group_parallel_tool_results_in_one_user_message() {
    let request = LlmChatRequest {
        conversation: vec![
            LlmChatMessage::assistant_with_tool_calls(
                "",
                vec![
                    LlmToolCall {
                        id: "tool-1".to_string(),
                        name: "lookup-one".to_string(),
                        arguments_json: "{}".to_string(),
                    },
                    LlmToolCall {
                        id: "tool-2".to_string(),
                        name: "lookup-two".to_string(),
                        arguments_json: "{}".to_string(),
                    },
                ],
            ),
            LlmChatMessage::tool("tool-1", "first result"),
            LlmChatMessage::tool("tool-2", "second result"),
        ],
        ..Default::default()
    };

    let body = map_request(Protocol::Messages, &config("minimax-m3"), &request).unwrap();
    let messages = body["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[1]["role"], "user");
    assert_eq!(messages[1]["content"].as_array().unwrap().len(), 2);
    assert_eq!(messages[1]["content"][0]["tool_use_id"], "tool-1");
    assert_eq!(messages[1]["content"][1]["tool_use_id"], "tool-2");
}

#[test]
fn request_body_logging_requires_full_debug_logging() {
    let payload = json!({"prompt": "athlete training context"});

    assert!(request_body_for_log(&payload, false).is_none());
    assert_eq!(
        request_body_for_log(&payload, true).as_deref(),
        Some(r#"{"prompt":"athlete training context"}"#)
    );
}

#[test]
fn messages_reject_invalid_tool_arguments() {
    let request = LlmChatRequest {
        conversation: vec![LlmChatMessage::assistant_with_tool_calls(
            "",
            vec![LlmToolCall {
                id: "tool-1".to_string(),
                name: "lookup".to_string(),
                arguments_json: "not-json".to_string(),
            }],
        )],
        ..Default::default()
    };

    let error = map_request(Protocol::Messages, &config("minimax-m3"), &request).unwrap_err();
    assert!(matches!(error, LlmError::InvalidResponse(_)));
}

#[test]
fn responses_failed_status_is_invalid_response() {
    let error = map_response(
        Protocol::Responses,
        &config("gpt-5.6-luna"),
        json!({
            "id": "resp-failed",
            "model": "gpt-5.6-luna",
            "status": "failed",
            "output_text": "partial output",
            "error": {"message": "provider failed"}
        }),
    )
    .unwrap_err();

    assert!(matches!(error, LlmError::InvalidResponse(_)));
}
