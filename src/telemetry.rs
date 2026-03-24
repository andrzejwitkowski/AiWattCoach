use std::{borrow::Cow, env, error::Error};

use opentelemetry::trace::TracerProvider as _;
use opentelemetry::KeyValue;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{runtime::Tokio, trace::TracerProvider, Resource};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Registry};

const REDACTED_VALUE: &str = "[REDACTED]";

#[derive(Debug)]
pub struct TelemetryGuard {
    tracer_provider: Option<TracerProvider>,
}

impl TelemetryGuard {
    pub fn shutdown(&mut self) -> Result<(), opentelemetry::trace::TraceError> {
        if let Some(tracer_provider) = self.tracer_provider.take() {
            tracer_provider.shutdown()?;
        }

        Ok(())
    }
}

impl Drop for TelemetryGuard {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

pub fn init_telemetry(service_name: &str) -> Result<TelemetryGuard, Box<dyn Error + Send + Sync>> {
    let effective_service_name = env::var("OTEL_SERVICE_NAME")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| service_name.to_string());
    let tracer_provider = build_tracer_provider(&effective_service_name)?;
    let otel_layer = tracer_provider.as_ref().map(|provider| {
        tracing_opentelemetry::layer().with_tracer(provider.tracer(effective_service_name.clone()))
    });

    let init_result = Registry::default()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(
            tracing_subscriber::fmt::layer()
                .json()
                .with_current_span(true)
                .with_span_list(true),
        )
        .with(otel_layer)
        .try_init();

    if let Err(error) = init_result {
        if let Some(tracer_provider) = tracer_provider {
            let _ = tracer_provider.shutdown();
        }

        return Err(Box::new(error));
    }

    Ok(TelemetryGuard { tracer_provider })
}

pub fn redact_if_sensitive<'a>(key: &str, value: &'a str) -> Cow<'a, str> {
    if is_sensitive_key(key) {
        Cow::Borrowed(REDACTED_VALUE)
    } else {
        Cow::Borrowed(value)
    }
}

fn build_tracer_provider(
    service_name: &str,
) -> Result<Option<TracerProvider>, Box<dyn Error + Send + Sync>> {
    let Some(endpoint) = env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
        .ok()
        .filter(|value| !value.trim().is_empty())
    else {
        return Ok(None);
    };

    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(endpoint)
        .build()?;

    let resource = Resource::new([KeyValue::new("service.name", service_name.to_owned())]);

    Ok(Some(
        TracerProvider::builder()
            .with_batch_exporter(exporter, Tokio)
            .with_resource(resource)
            .build(),
    ))
}

fn is_sensitive_key(key: &str) -> bool {
    let lowercase = key.to_ascii_lowercase();

    lowercase.contains("password")
        || lowercase.contains("secret")
        || lowercase.contains("token")
        || lowercase.contains("username")
        || lowercase.contains("api_key")
        || lowercase.contains("api-key")
        || lowercase.contains("apikey")
        || lowercase == "user"
        || lowercase.ends_with("_user")
}

#[cfg(test)]
mod tests {
    use super::redact_if_sensitive;

    #[test]
    fn redacts_password_like_fields() {
        assert_eq!(redact_if_sensitive("password", "secret"), "[REDACTED]");
        assert_eq!(redact_if_sensitive("db_password", "secret"), "[REDACTED]");
    }

    #[test]
    fn redacts_secret_like_fields() {
        assert_eq!(redact_if_sensitive("client_secret", "secret"), "[REDACTED]");
        assert_eq!(redact_if_sensitive("secret", "secret"), "[REDACTED]");
    }

    #[test]
    fn redacts_token_like_fields() {
        assert_eq!(redact_if_sensitive("access_token", "secret"), "[REDACTED]");
        assert_eq!(redact_if_sensitive("refreshToken", "secret"), "[REDACTED]");
    }

    #[test]
    fn redacts_username_like_fields() {
        assert_eq!(redact_if_sensitive("username", "alice"), "[REDACTED]");
        assert_eq!(redact_if_sensitive("db_user", "alice"), "[REDACTED]");
    }

    #[test]
    fn redacts_api_key_like_fields() {
        assert_eq!(redact_if_sensitive("api_key", "secret"), "[REDACTED]");
        assert_eq!(redact_if_sensitive("x-api-key", "secret"), "[REDACTED]");
    }

    #[test]
    fn leaves_non_sensitive_fields_unchanged() {
        assert_eq!(redact_if_sensitive("request_id", "abc-123"), "abc-123");
        assert_eq!(redact_if_sensitive("environment", "dev"), "dev");
    }
}
