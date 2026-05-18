use reqwest::StatusCode;
use schemars::{schema_for, JsonSchema};
use serde::Deserialize;

use crate::domain::{
    llm::truncate_logged_body,
    training_plan::TrainingPlanError,
    training_plan_supervisor::{
        BoxFuture, TrainingPlanSupervisorBatchPort, TrainingPlanSupervisorBatchRequest,
        TrainingPlanSupervisorBatchSubmission, TrainingPlanSupervisorDecision,
        TrainingPlanSupervisorReview,
    },
};

use super::{
    batch_dto::{
        GeminiBatchCreateBody, GeminiBatchCreateRequest, GeminiBatchCreateResponse,
        GeminiBatchGenerateContentRequest, GeminiBatchGetResponse, GeminiBatchInlineRequest,
        GeminiBatchInlineRequests, GeminiBatchInputConfig, GeminiBatchRequestMetadata,
        GeminiBatchResultLine,
    },
    dto::{GeminiContent, GeminiGenerationConfig, GeminiTextPart},
};

const DEFAULT_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";

#[derive(Clone)]
pub struct GeminiBatchClient {
    client: reqwest::Client,
    base_url: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
struct SupervisorReplyEnvelope {
    decision: String,
    reason: String,
    #[serde(default)]
    plan: Option<String>,
}

impl GeminiBatchClient {
    pub fn new(client: reqwest::Client) -> Self {
        Self {
            client,
            base_url: DEFAULT_BASE_URL.to_string(),
        }
    }

    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into().trim_end_matches('/').to_string();
        self
    }

    pub fn supervisor_reply_json_schema() -> String {
        serde_json::to_string_pretty(&schema_for!(SupervisorReplyEnvelope))
            .expect("supervisor reply schema should serialize")
    }
}

impl TrainingPlanSupervisorBatchPort for GeminiBatchClient {
    fn submit_review(
        &self,
        api_key: &str,
        request: TrainingPlanSupervisorBatchRequest,
    ) -> BoxFuture<Result<TrainingPlanSupervisorBatchSubmission, TrainingPlanError>> {
        let client = self.client.clone();
        let base_url = self.base_url.clone();
        let api_key = api_key.to_string();
        Box::pin(async move {
            let api_model = normalize_gemini_model_name(&request.model);
            let batch_url = format!("{base_url}/models/{api_model}:batchGenerateContent");
            let payload = build_supervisor_batch_create_request(&request)?;
            tracing::info!(
                url = %batch_url,
                model = %request.model,
                worker_operation_key = %request.worker_operation_key,
                "sending gemini supervisor batch create request"
            );
            let response = client
                .post(batch_url.clone())
                .header("x-goog-api-key", &api_key)
                .json(&payload)
                .send()
                .await
                .map_err(|error| {
                    TrainingPlanError::Repository(format!(
                        "gemini batch create transport failure: {}",
                        error.without_url()
                    ))
                })?;
            let status = response.status();
            let body = response.text().await.map_err(|error| {
                TrainingPlanError::Repository(format!(
                    "gemini batch create response body read failed: {}",
                    error.without_url()
                ))
            })?;
            if !status.is_success() {
                tracing::warn!(
                    url = %batch_url,
                    model = %request.model,
                    worker_operation_key = %request.worker_operation_key,
                    status = status.as_u16(),
                    response_body = %truncate_logged_body(&body),
                    "gemini batch create request failed"
                );
                return Err(map_transport_error(
                    "gemini batch create request failed",
                    status,
                    body,
                ));
            }
            let response: GeminiBatchCreateResponse =
                serde_json::from_str(&body).map_err(|error| {
                    TrainingPlanError::Repository(format!(
                        "gemini batch create json parsing failed: {error}"
                    ))
                })?;
            Ok(TrainingPlanSupervisorBatchSubmission {
                batch_name: response.name,
            })
        })
    }

    fn download_result(
        &self,
        api_key: &str,
        batch_name: &str,
    ) -> BoxFuture<Result<TrainingPlanSupervisorReview, TrainingPlanError>> {
        let client = self.client.clone();
        let base_url = self.base_url.clone();
        let api_key = api_key.to_string();
        let batch_name = batch_name.to_string();
        Box::pin(async move {
            let batch_url = format!("{}/{batch_name}", base_url);
            tracing::info!(url = %batch_url, batch_name = %batch_name, "sending gemini batch get request");
            let response = client
                .get(batch_url.clone())
                .header("x-goog-api-key", &api_key)
                .send()
                .await
                .map_err(|error| {
                    TrainingPlanError::Repository(format!(
                        "gemini batch get transport failure: {}",
                        error.without_url()
                    ))
                })?;

            let status = response.status();
            let body = response.text().await.map_err(|error| {
                TrainingPlanError::Repository(format!(
                    "gemini batch get response body read failed: {}",
                    error.without_url()
                ))
            })?;
            if !status.is_success() {
                tracing::warn!(
                    url = %batch_url,
                    batch_name = %batch_name,
                    status = status.as_u16(),
                    response_body = %truncate_logged_body(&body),
                    "gemini batch get request failed"
                );
                return Err(map_transport_error(
                    "gemini batch get request failed",
                    status,
                    body,
                ));
            }

            let batch: GeminiBatchGetResponse = serde_json::from_str(&body).map_err(|error| {
                TrainingPlanError::Repository(format!(
                    "gemini batch get json parsing failed: {error}"
                ))
            })?;
            let state = batch
                .metadata
                .as_ref()
                .and_then(|metadata| metadata.state.as_deref());
            if state != Some("JOB_STATE_SUCCEEDED") {
                let error_message =
                    batch
                        .error
                        .and_then(|error| error.message)
                        .unwrap_or_else(|| {
                            format!(
                                "unexpected Gemini batch state for {}: {:?}",
                                batch.name, state
                            )
                        });
                return Err(TrainingPlanError::Repository(error_message));
            }
            let responses_file = batch
                .response
                .and_then(|response| response.responses_file)
                .ok_or_else(|| {
                    TrainingPlanError::Repository(format!(
                        "Gemini batch {} succeeded without responsesFile",
                        batch.name
                    ))
                })?;

            let download_url = format!(
                "{}/download/v1beta/{}:download?alt=media",
                base_url.trim_end_matches("/v1beta"),
                responses_file
            );
            tracing::info!(
                url = %download_url,
                batch_name = %batch_name,
                responses_file = %responses_file,
                "sending gemini batch download request"
            );
            let response = client
                .get(download_url.clone())
                .header("x-goog-api-key", &api_key)
                .send()
                .await
                .map_err(|error| {
                    TrainingPlanError::Repository(format!(
                        "gemini batch download transport failure: {}",
                        error.without_url()
                    ))
                })?;
            let status = response.status();
            let body = response.text().await.map_err(|error| {
                TrainingPlanError::Repository(format!(
                    "gemini batch download response body read failed: {}",
                    error.without_url()
                ))
            })?;
            if !status.is_success() {
                tracing::warn!(
                    url = %download_url,
                    batch_name = %batch_name,
                    status = status.as_u16(),
                    response_body = %truncate_logged_body(&body),
                    "gemini batch download request failed"
                );
                return Err(map_transport_error(
                    "gemini batch download request failed",
                    status,
                    body,
                ));
            }

            parse_result_file(&body)
        })
    }
}

fn build_supervisor_batch_create_request(
    request: &TrainingPlanSupervisorBatchRequest,
) -> Result<GeminiBatchCreateRequest, TrainingPlanError> {
    let response_schema: serde_json::Value = serde_json::from_str(
        &GeminiBatchClient::supervisor_reply_json_schema(),
    )
    .map_err(|error| {
        TrainingPlanError::Repository(format!(
            "gemini supervisor reply schema parsing failed: {error}"
        ))
    })?;

    Ok(GeminiBatchCreateRequest {
        batch: GeminiBatchCreateBody {
            display_name: format!(
                "aiwattcoach-supervisor-{}",
                sanitize_batch_display_name(&request.worker_operation_key)
            ),
            input_config: GeminiBatchInputConfig {
                requests: GeminiBatchInlineRequests {
                    requests: vec![GeminiBatchInlineRequest {
                        request: GeminiBatchGenerateContentRequest {
                            contents: vec![GeminiContent {
                                role: "user".to_string(),
                                parts: vec![GeminiTextPart {
                                    text: supervisor_prompt(&request.original_plan),
                                }],
                            }],
                            generation_config: GeminiGenerationConfig {
                                response_mime_type: Some("application/json".to_string()),
                                response_schema: Some(response_schema),
                            },
                        },
                        metadata: GeminiBatchRequestMetadata {
                            key: request.worker_operation_key.clone(),
                        },
                    }],
                },
            },
        },
    })
}

fn supervisor_prompt(original_plan: &str) -> String {
    format!(
        "You are a second-pass cycling coach supervisor reviewing an already generated 14-day training plan. Return only JSON matching the provided schema. Decide exactly one of: accept, replace, fail. Use accept when the plan is coherent, safe, parser-friendly, and only needs no material change. Use replace when the plan is usable as context but materially violates training logic, availability, recovery, race handling, or workout syntax; in that case return a complete replacement plan in `plan` containing all 14 dated sections, not a diff. Use fail when the input is unusable, incomplete, unsafe to repair confidently, or cannot be converted into the supported workout grammar. For replace plans, output parser-friendly workout-builder text only inside the JSON string: one YYYY-MM-DD section per day, then either Rest Day, Rest Day: <reason>, or workout text where actionable steps start with `- `. Do not include markdown fences. Original plan:\n\n{original_plan}"
    )
}

fn normalize_gemini_model_name(model: &str) -> &str {
    model.strip_prefix("google/").unwrap_or(model)
}

fn sanitize_batch_display_name(value: &str) -> String {
    value
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect()
}

fn parse_result_file(body: &str) -> Result<TrainingPlanSupervisorReview, TrainingPlanError> {
    let mut parsed_lines = body
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str::<GeminiBatchResultLine>(line).map_err(|error| {
                TrainingPlanError::Repository(format!(
                    "gemini batch result line is not valid JSON: {error}"
                ))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let line = parsed_lines.drain(..).next().ok_or_else(|| {
        TrainingPlanError::Repository("gemini batch result file is empty".to_string())
    })?;
    if let Some(error) = line.error.and_then(|error| error.message) {
        return Err(TrainingPlanError::Repository(format!(
            "gemini batch result returned error: {error}"
        )));
    }
    let text = line
        .response
        .and_then(|response| response.candidates)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|candidate| candidate.content)
        .flat_map(|content| content.parts.into_iter())
        .filter_map(|part| part.text.map(|text| text.trim().to_string()))
        .find(|text| !text.is_empty())
        .ok_or_else(|| {
            TrainingPlanError::Repository(
                "gemini batch result returned no supervisor reply text".to_string(),
            )
        })?;
    parse_review(&text)
}

fn parse_review(raw: &str) -> Result<TrainingPlanSupervisorReview, TrainingPlanError> {
    let payload = extract_json_payload(raw);
    let parsed: SupervisorReplyEnvelope = serde_json::from_str(payload).map_err(|error| {
        TrainingPlanError::Repository(format!(
            "gemini supervisor batch reply is not valid JSON: {error}"
        ))
    })?;
    let decision = TrainingPlanSupervisorDecision::try_from(parsed.decision.trim())
        .map_err(TrainingPlanError::Repository)?;
    let review = TrainingPlanSupervisorReview {
        decision,
        reason: parsed.reason,
        plan: parsed.plan,
    };
    review.validate()?;
    Ok(review)
}

fn extract_json_payload(raw: &str) -> &str {
    let trimmed = raw.trim();

    if let Some(stripped) = trimmed.strip_prefix("```") {
        let inner = stripped.trim().trim_end_matches("```").trim();

        if inner.starts_with('{') || inner.starts_with('[') {
            return inner;
        }

        if let Some((_, rest)) = inner.split_once('\n') {
            return rest.trim();
        }

        return inner;
    }

    trimmed
}

fn map_transport_error(prefix: &str, status: StatusCode, body: String) -> TrainingPlanError {
    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
            TrainingPlanError::Unavailable(format!("{prefix}: unauthorized"))
        }
        _ => TrainingPlanError::Repository(format!("{prefix}: {}", truncate_logged_body(&body))),
    }
}

#[cfg(test)]
mod tests {
    use super::parse_result_file;
    use crate::domain::training_plan_supervisor::TrainingPlanSupervisorDecision;

    #[test]
    fn parse_result_file_reads_text_from_later_candidate() {
        let body = r#"{"response":{"candidates":[{"content":{"parts":[{"inlineData":{"mimeType":"text/plain","data":"ignored"}}]}},{"content":{"parts":[{"text":" {\"decision\":\"accept\",\"reason\":\"looks good\" } "}]}}]}}"#;

        let review = parse_result_file(body).expect("expected parsed review");

        assert_eq!(review.decision, TrainingPlanSupervisorDecision::Accept);
        assert_eq!(review.reason, "looks good");
        assert_eq!(review.plan, None);
    }

    #[test]
    fn parse_result_file_skips_empty_text_parts() {
        let body = r#"{"response":{"candidates":[{"content":{"parts":[{"text":"   "},{"text":" {\"decision\":\"accept\",\"reason\":\"later text\" } "}]}}]}}"#;

        let review = parse_result_file(body).expect("expected parsed review");

        assert_eq!(review.decision, TrainingPlanSupervisorDecision::Accept);
        assert_eq!(review.reason, "later text");
        assert_eq!(review.plan, None);
    }
}
