use reqwest::StatusCode;
use schemars::{schema_for, JsonSchema};
use serde::Deserialize;

use crate::domain::{
    llm::truncate_logged_body,
    training_plan::TrainingPlanError,
    training_plan_supervisor::{
        BoxFuture, TrainingPlanSupervisorBatchPort, TrainingPlanSupervisorDecision,
        TrainingPlanSupervisorReview,
    },
};

use super::batch_dto::{GeminiBatchGetResponse, GeminiBatchResultLine};

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
        .find_map(|part| part.text.map(|text| text.trim().to_string()))
        .filter(|text| !text.is_empty())
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
}
