mod logging;

use opentelemetry::{propagation::TextMapPropagator, trace::TraceContextExt as _};
use opentelemetry_http::HeaderInjector;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use reqwest::Url;
use sha2::Digest as _;
use tracing_opentelemetry::OpenTelemetrySpanExt;

use crate::{
    adapters::wahoo::dto::{
        WahooFileReferenceResponse, WahooTokenResponse, WahooWorkoutListResponse,
        WahooWorkoutResponse, WahooWorkoutSummaryResponse,
    },
    domain::wahoo::{
        BoxFuture, WahooApiPort, WahooError, WahooFileReference, WahooOAuthPort, WahooToken,
        WahooWorkout, WahooWorkoutList, WahooWorkoutSummary,
    },
};

const DEFAULT_BASE_URL: &str = "https://api.wahooligan.com";

#[derive(Clone)]
pub struct WahooOAuthClient {
    client: reqwest::Client,
    client_id: String,
    client_secret: String,
    redirect_url: String,
    authorize_url: String,
    token_url: String,
    scope: String,
    base_url: String,
}

impl WahooOAuthClient {
    pub fn new(
        client: reqwest::Client,
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
        redirect_url: impl Into<String>,
        authorize_url: impl Into<String>,
        token_url: impl Into<String>,
        scope: impl Into<String>,
    ) -> Self {
        Self {
            client,
            client_id: client_id.into(),
            client_secret: client_secret.into(),
            redirect_url: redirect_url.into(),
            authorize_url: authorize_url.into(),
            token_url: token_url.into(),
            scope: scope.into(),
            base_url: DEFAULT_BASE_URL.to_string(),
        }
    }

    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into().trim_end_matches('/').to_string();
        self
    }

    fn with_trace_context(request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        let context = tracing::Span::current().context();

        if !context.span().span_context().is_valid() {
            return request;
        }

        let mut headers = reqwest::header::HeaderMap::new();
        TraceContextPropagator::new().inject_context(&context, &mut HeaderInjector(&mut headers));

        request.headers(headers)
    }

    async fn exchange_form(
        client: reqwest::Client,
        token_url: String,
        form: Vec<(&'static str, String)>,
    ) -> Result<WahooToken, WahooError> {
        let response = logging::execute_and_log_no_body(
            &client,
            Self::with_trace_context(client.post(token_url).form(&form)),
        )
        .await
        .map_err(|error| WahooError::External(error.to_string()))?;
        if !response.status.is_success() {
            return Err(WahooError::External(format!(
                "Wahoo OAuth request failed with status {} ({})",
                response.status,
                summarize_error_body(&response.body)
            )));
        }
        let payload: WahooTokenResponse = serde_json::from_slice(&response.body)
            .map_err(|error| WahooError::External(error.to_string()))?;
        let now = chrono::Utc::now().timestamp();

        Ok(WahooToken {
            access_token: payload.access_token,
            refresh_token: payload.refresh_token,
            expires_at_epoch_seconds: now.saturating_add(payload.expires_in),
        })
    }

    fn api_url(&self, path: &str) -> String {
        format!("{}{path}", self.base_url)
    }

    fn bearer_request(&self, url: String, access_token: &str) -> reqwest::RequestBuilder {
        Self::with_trace_context(self.client.get(url).bearer_auth(access_token))
    }

    async fn decode_json<T>(response: logging::LoggedResponse) -> Result<T, WahooError>
    where
        T: serde::de::DeserializeOwned,
    {
        serde_json::from_slice(&response.body)
            .map_err(|error| WahooError::External(error.to_string()))
    }

    async fn execute_api_get<T>(&self, request: reqwest::RequestBuilder) -> Result<T, WahooError>
    where
        T: serde::de::DeserializeOwned,
    {
        let response = logging::execute_and_log_no_body(&self.client, request)
            .await
            .map_err(|error| WahooError::External(error.to_string()))?;
        match response.status {
            status if status.is_success() => Self::decode_json(response).await,
            reqwest::StatusCode::NOT_FOUND => Err(WahooError::NotFound),
            status => Err(WahooError::External(format!(
                "Wahoo API request failed with status {} ({})",
                status,
                summarize_error_body(&response.body)
            ))),
        }
    }
}

fn map_file_reference(file: Option<WahooFileReferenceResponse>) -> Option<WahooFileReference> {
    let url = file?.url?.trim().to_string();
    if url.is_empty() {
        None
    } else {
        Some(WahooFileReference { url })
    }
}

fn parse_optional_decimal(value: Option<String>) -> Option<f64> {
    value?.trim().parse().ok()
}

fn map_workout_summary(summary: WahooWorkoutSummaryResponse) -> WahooWorkoutSummary {
    WahooWorkoutSummary {
        id: summary.id,
        name: summary.name,
        ascent_meters: parse_optional_decimal(summary.ascent_accum),
        cadence_avg_rpm: parse_optional_decimal(summary.cadence_avg),
        calories: parse_optional_decimal(summary.calories_accum),
        distance_meters: parse_optional_decimal(summary.distance_accum),
        duration_active_seconds: parse_optional_decimal(summary.duration_active_accum),
        duration_paused_seconds: parse_optional_decimal(summary.duration_paused_accum),
        duration_total_seconds: parse_optional_decimal(summary.duration_total_accum),
        heart_rate_avg_bpm: parse_optional_decimal(summary.heart_rate_avg),
        normalized_power_watts: parse_optional_decimal(summary.power_bike_np_last),
        training_stress_score: parse_optional_decimal(summary.power_bike_tss_last),
        average_power_watts: parse_optional_decimal(summary.power_avg),
        speed_avg_mps: parse_optional_decimal(summary.speed_avg),
        total_work_joules: parse_optional_decimal(summary.work_accum),
        time_zone: summary.time_zone,
        manual: summary.manual,
        edited: summary.edited,
        fitness_app_id: summary.fitness_app_id,
        file: map_file_reference(summary.file),
        created_at: summary.created_at,
        updated_at: summary.updated_at,
    }
}

fn map_workout(workout: WahooWorkoutResponse) -> WahooWorkout {
    WahooWorkout {
        id: workout.id,
        starts: workout.starts,
        minutes: workout.minutes,
        name: workout.name,
        plan_id: workout.plan_id,
        plan_ids: workout.plan_ids,
        route_id: workout.route_id,
        workout_token: workout.workout_token,
        workout_type_id: workout.workout_type_id,
        workout_summary: workout.workout_summary.map(map_workout_summary),
        created_at: workout.created_at,
        updated_at: workout.updated_at,
    }
}

fn summarize_error_body(body: &[u8]) -> String {
    let fallback = || {
        let digest = sha2::Sha256::digest(body);
        let hash = format!("{digest:x}");
        format!(
            "payload bytes={} hash={}",
            body.len(),
            &hash[..12.min(hash.len())]
        )
    };
    let trimmed = std::str::from_utf8(body)
        .ok()
        .map(str::trim)
        .filter(|text| !text.is_empty());

    let Some(text) = trimmed else {
        return fallback();
    };

    let Ok(error_payload) = serde_json::from_str::<WahooOAuthErrorResponse>(text) else {
        return fallback();
    };
    let Some(error) = error_payload
        .error
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return fallback();
    };

    let mut summary = format!("oauth_error={error}");
    if let Some(description) = error_payload
        .error_description
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        summary.push_str(" description=");
        summary.push_str(&normalize_preview(description, 160));
    }

    summary
}

#[derive(serde::Deserialize)]
struct WahooOAuthErrorResponse {
    error: Option<String>,
    error_description: Option<String>,
}

fn normalize_preview(text: &str, max_chars: usize) -> String {
    text.chars()
        .take(max_chars)
        .map(|character| match character {
            '\r' | '\n' => ' ',
            other => other,
        })
        .collect()
}

impl WahooOAuthPort for WahooOAuthClient {
    fn build_authorize_url(&self, state: &str) -> Result<String, WahooError> {
        let mut url = Url::parse(&self.authorize_url)
            .map_err(|error| WahooError::External(error.to_string()))?;
        url.query_pairs_mut()
            .append_pair("response_type", "code")
            .append_pair("client_id", &self.client_id)
            .append_pair("redirect_uri", &self.redirect_url)
            .append_pair("scope", &self.scope)
            .append_pair("state", state);

        Ok(url.to_string())
    }

    fn exchange_code(&self, code: &str) -> BoxFuture<Result<WahooToken, WahooError>> {
        let client = self.client.clone();
        let client_id = self.client_id.clone();
        let client_secret = self.client_secret.clone();
        let redirect_url = self.redirect_url.clone();
        let token_url = self.token_url.clone();
        let code = code.to_string();

        Box::pin(async move {
            Self::exchange_form(
                client,
                token_url,
                vec![
                    ("client_id", client_id),
                    ("client_secret", client_secret),
                    ("code", code),
                    ("redirect_uri", redirect_url),
                    ("grant_type", "authorization_code".to_string()),
                ],
            )
            .await
        })
    }

    fn refresh_token(&self, refresh_token: &str) -> BoxFuture<Result<WahooToken, WahooError>> {
        let client = self.client.clone();
        let client_id = self.client_id.clone();
        let client_secret = self.client_secret.clone();
        let token_url = self.token_url.clone();
        let refresh_token = refresh_token.to_string();

        Box::pin(async move {
            Self::exchange_form(
                client,
                token_url,
                vec![
                    ("client_id", client_id),
                    ("client_secret", client_secret),
                    ("refresh_token", refresh_token),
                    ("grant_type", "refresh_token".to_string()),
                ],
            )
            .await
        })
    }
}

impl WahooApiPort for WahooOAuthClient {
    fn list_workouts(
        &self,
        access_token: &str,
        page: usize,
        per_page: usize,
    ) -> BoxFuture<Result<WahooWorkoutList, WahooError>> {
        let client = self.clone();
        let access_token = access_token.to_string();
        Box::pin(async move {
            let mut url = reqwest::Url::parse(&client.api_url("/v1/workouts"))
                .map_err(|error| WahooError::External(error.to_string()))?;
            url.query_pairs_mut()
                .append_pair("page", &page.to_string())
                .append_pair("per_page", &per_page.to_string());
            let payload: WahooWorkoutListResponse = client
                .execute_api_get(client.bearer_request(url.to_string(), &access_token))
                .await?;

            Ok(WahooWorkoutList {
                workouts: payload.workouts.into_iter().map(map_workout).collect(),
                total: payload.total.unwrap_or_default(),
                page: payload.page.unwrap_or(page),
                per_page: payload.per_page.unwrap_or(per_page),
                order: payload.order,
                sort: payload.sort,
            })
        })
    }

    fn get_workout(
        &self,
        access_token: &str,
        workout_id: i64,
    ) -> BoxFuture<Result<WahooWorkout, WahooError>> {
        let client = self.clone();
        let access_token = access_token.to_string();
        Box::pin(async move {
            let payload: WahooWorkoutResponse = client
                .execute_api_get(client.bearer_request(
                    client.api_url(&format!("/v1/workouts/{workout_id}")),
                    &access_token,
                ))
                .await?;
            Ok(map_workout(payload))
        })
    }

    fn get_workout_summary(
        &self,
        access_token: &str,
        workout_id: i64,
    ) -> BoxFuture<Result<Option<WahooWorkoutSummary>, WahooError>> {
        let client = self.clone();
        let access_token = access_token.to_string();
        Box::pin(async move {
            match client
                .execute_api_get::<WahooWorkoutSummaryResponse>(client.bearer_request(
                    client.api_url(&format!("/v1/workouts/{workout_id}/workout_summary")),
                    &access_token,
                ))
                .await
            {
                Ok(summary) => Ok(Some(map_workout_summary(summary))),
                Err(WahooError::NotFound) => Ok(None),
                Err(error) => Err(error),
            }
        })
    }

    fn download_workout_file(&self, file_url: &str) -> BoxFuture<Result<Vec<u8>, WahooError>> {
        let file_url = match Url::parse(file_url) {
            Ok(file_url) if file_url.scheme() == "https" && file_url.host_str().is_some() => {
                file_url
            }
            Ok(file_url) => {
                return Box::pin(async move {
                    Err(WahooError::External(format!(
                        "invalid Wahoo FIT file URL scheme/host: {file_url}"
                    )))
                });
            }
            Err(error) => {
                return Box::pin(async move {
                    Err(WahooError::External(format!(
                        "invalid Wahoo FIT file URL: {error}"
                    )))
                });
            }
        };
        let client = self.client.clone();
        let request = Self::with_trace_context(client.get(file_url));
        Box::pin(async move {
            let response = logging::execute_and_log_no_body(&client, request)
                .await
                .map_err(|error| WahooError::External(error.to_string()))?;
            if !response.status.is_success() {
                return Err(WahooError::External(format!(
                    "Wahoo file download failed with status {} ({})",
                    response.status,
                    summarize_error_body(&response.body)
                )));
            }
            Ok(response.body.to_vec())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::summarize_error_body;

    #[test]
    fn summarize_error_body_uses_utf8_preview_when_available() {
        assert_eq!(
            summarize_error_body(b"{\"error\":\"invalid_grant\"}\r\n"),
            "oauth_error=invalid_grant"
        );
    }

    #[test]
    fn summarize_error_body_includes_description_when_present() {
        assert_eq!(
            summarize_error_body(
                b"{\"error\":\"invalid_client\",\"error_description\":\"bad redirect\\nuri\"}",
            ),
            "oauth_error=invalid_client description=bad redirect uri"
        );
    }

    #[test]
    fn summarize_error_body_falls_back_to_size_and_hash_for_binary_payloads() {
        let summary = summarize_error_body(&[0, 159, 146, 150]);

        assert!(summary.starts_with("payload bytes=4 hash="));
    }
}
