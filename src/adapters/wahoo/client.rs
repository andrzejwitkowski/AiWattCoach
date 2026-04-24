mod logging;

use opentelemetry::{propagation::TextMapPropagator, trace::TraceContextExt as _};
use opentelemetry_http::HeaderInjector;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use reqwest::Url;
use sha2::Digest as _;
use tracing_opentelemetry::OpenTelemetrySpanExt;

use crate::{
    adapters::wahoo::dto::WahooTokenResponse,
    domain::wahoo::{BoxFuture, WahooError, WahooOAuthPort, WahooToken},
};

#[derive(Clone)]
pub struct WahooOAuthClient {
    client: reqwest::Client,
    client_id: String,
    client_secret: String,
    redirect_url: String,
    authorize_url: String,
    token_url: String,
    scope: String,
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
        }
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
