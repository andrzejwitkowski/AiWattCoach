mod logging;

use opentelemetry::{propagation::TextMapPropagator, trace::TraceContextExt as _};
use opentelemetry_http::HeaderInjector;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use reqwest::Url;
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
                "Wahoo OAuth request failed with status {}",
                response.status
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
