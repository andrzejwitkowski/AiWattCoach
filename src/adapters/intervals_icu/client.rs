mod api;
mod connection;
mod details;
mod errors;
mod mapping;

use opentelemetry::{propagation::TextMapPropagator, trace::TraceContextExt as _};
use opentelemetry_http::HeaderInjector;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use reqwest::{Client, RequestBuilder, StatusCode};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tracing_opentelemetry::OpenTelemetrySpanExt;

const DEFAULT_BASE_URL: &str = "https://intervals.icu";
const MAX_LOGGED_RESPONSE_BODY_CHARS: usize = 400;

#[derive(Debug)]
struct ApiFailure {
    status: Option<StatusCode>,
    error: crate::domain::intervals::IntervalsError,
    response_body: Option<String>,
}

impl ApiFailure {
    fn is_unprocessable_entity(&self) -> bool {
        self.status == Some(StatusCode::UNPROCESSABLE_ENTITY)
    }
}

#[derive(Clone)]
pub struct IntervalsIcuClient {
    client: Client,
    base_url: String,
}

impl IntervalsIcuClient {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            base_url: DEFAULT_BASE_URL.to_string(),
        }
    }

    pub fn with_timeouts(
        connect_timeout_secs: u64,
        timeout_secs: u64,
    ) -> Result<Self, reqwest::Error> {
        let client = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(connect_timeout_secs))
            .timeout(std::time::Duration::from_secs(timeout_secs))
            .build()?;
        Ok(Self {
            client,
            base_url: DEFAULT_BASE_URL.to_string(),
        })
    }

    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into().trim_end_matches('/').to_string();
        self
    }

    fn athlete_url(&self, athlete_id: &str, path: &str) -> String {
        Self::athlete_url_impl(&self.base_url, athlete_id, path)
    }

    fn athlete_url_impl(base_url: &str, athlete_id: &str, path: &str) -> String {
        format!("{base_url}/api/v1/athlete/{athlete_id}{path}")
    }

    fn with_trace_context(request: RequestBuilder) -> RequestBuilder {
        let context = tracing::Span::current().context();

        if !context.span().span_context().is_valid() {
            return request;
        }

        let mut headers = reqwest::header::HeaderMap::new();
        TraceContextPropagator::new().inject_context(&context, &mut HeaderInjector(&mut headers));

        request.headers(headers)
    }

    fn activity_url(&self, activity_id: &str, path: &str) -> String {
        Self::activity_url_impl(&self.base_url, activity_id, path)
    }

    fn activity_url_impl(base_url: &str, activity_id: &str, path: &str) -> String {
        format!("{base_url}/api/v1/activity/{activity_id}{path}")
    }
}

pub(super) fn truncate_logged_response_body(body: &str) -> String {
    let chars = body.chars().count();
    let bytes = body.len();
    let digest = Sha256::digest(body.as_bytes());
    let hash_prefix = format!("{:x}", digest);
    let hash_prefix = &hash_prefix[..12.min(hash_prefix.len())];

    let shape = match serde_json::from_str::<Value>(body) {
        Ok(Value::Array(items)) => format!("array(count={})", items.len()),
        Ok(Value::Object(map)) => format!("object(keys={})", map.len()),
        Ok(_) => "other".to_string(),
        Err(_) => {
            if chars > MAX_LOGGED_RESPONSE_BODY_CHARS {
                format!("other(truncated_at_chars={MAX_LOGGED_RESPONSE_BODY_CHARS})")
            } else {
                "other".to_string()
            }
        }
    };

    format!("payload chars={chars} bytes={bytes} type={shape} hash={hash_prefix}")
}
