use std::{borrow::Cow, env, error::Error};

use opentelemetry::trace::TracerProvider as _;
use opentelemetry::KeyValue;
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_otlp::{LogExporter, WithExportConfig};
use opentelemetry_sdk::{
    logs::{Logger as SdkLogger, LoggerProvider as SdkLoggerProvider},
    runtime::Tokio,
    trace::TracerProvider,
    Resource,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Registry};

const REDACTED_VALUE: &str = "[REDACTED]";

#[derive(Debug)]
pub struct TelemetryGuard {
    tracer_provider: Option<TracerProvider>,
    logger_provider: Option<SdkLoggerProvider>,
}

impl TelemetryGuard {
    pub fn shutdown(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut shutdown_error: Option<Box<dyn Error + Send + Sync>> = None;

        if let Some(tracer_provider) = self.tracer_provider.take() {
            if let Err(error) = tracer_provider.shutdown() {
                shutdown_error = Some(Box::new(error));
            }
        }

        if let Some(logger_provider) = self.logger_provider.take() {
            if let Err(error) = logger_provider.shutdown() {
                if shutdown_error.is_none() {
                    shutdown_error = Some(Box::new(error));
                }
            }
        }

        match shutdown_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

impl Drop for TelemetryGuard {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

pub fn setup_telemetry(service_name: &str) -> Result<TelemetryGuard, Box<dyn Error + Send + Sync>> {
    let effective_service_name = env::var("OTEL_SERVICE_NAME")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| service_name.to_string());
    let resource = build_resource(&effective_service_name);
    let otlp_endpoint = get_otlp_endpoint();
    let tracer_provider = build_tracer_provider(&resource, otlp_endpoint.as_deref())?;
    let logger_provider = match build_logger_provider(&resource, otlp_endpoint.as_deref()) {
        Ok(logger_provider) => logger_provider,
        Err(error) => {
            if let Some(tracer_provider) = tracer_provider {
                let _ = tracer_provider.shutdown();
            }

            return Err(error);
        }
    };
    let otel_trace_layer = tracer_provider.as_ref().map(|provider| {
        tracing_opentelemetry::layer().with_tracer(provider.tracer(effective_service_name.clone()))
    });
    let otel_log_layer = logger_provider.as_ref().map(build_log_bridge_layer);

    let init_result = Registry::default()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(
            tracing_subscriber::fmt::layer()
                .json()
                .with_current_span(true)
                .with_span_list(true),
        )
        .with(otel_log_layer)
        .with(otel_trace_layer)
        .try_init();

    if let Err(error) = init_result {
        let mut telemetry = TelemetryGuard {
            tracer_provider,
            logger_provider,
        };
        let _ = telemetry.shutdown();

        return Err(Box::new(error));
    }

    Ok(TelemetryGuard {
        tracer_provider,
        logger_provider,
    })
}

pub fn init_telemetry(service_name: &str) -> Result<TelemetryGuard, Box<dyn Error + Send + Sync>> {
    setup_telemetry(service_name)
}

pub fn redact_if_sensitive<'a>(key: &str, value: &'a str) -> Cow<'a, str> {
    if is_sensitive_key(key) {
        Cow::Borrowed(REDACTED_VALUE)
    } else {
        Cow::Borrowed(value)
    }
}

fn get_otlp_endpoint() -> Option<String> {
    env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn build_resource(service_name: &str) -> Resource {
    Resource::new([KeyValue::new("service.name", service_name.to_owned())])
}

fn build_tracer_provider(
    resource: &Resource,
    endpoint: Option<&str>,
) -> Result<Option<TracerProvider>, Box<dyn Error + Send + Sync>> {
    let Some(endpoint) = endpoint else {
        return Ok(None);
    };

    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(endpoint.to_string())
        .build()?;

    Ok(Some(
        TracerProvider::builder()
            .with_batch_exporter(exporter, Tokio)
            .with_resource(resource.clone())
            .build(),
    ))
}

fn build_logger_provider(
    resource: &Resource,
    endpoint: Option<&str>,
) -> Result<Option<SdkLoggerProvider>, Box<dyn Error + Send + Sync>> {
    let Some(endpoint) = endpoint else {
        return Ok(None);
    };

    let exporter = LogExporter::builder()
        .with_tonic()
        .with_endpoint(endpoint.to_string())
        .build()?;

    Ok(Some(
        SdkLoggerProvider::builder()
            .with_batch_exporter(exporter, Tokio)
            .with_resource(resource.clone())
            .build(),
    ))
}

fn build_log_bridge_layer(
    logger_provider: &SdkLoggerProvider,
) -> OpenTelemetryTracingBridge<SdkLoggerProvider, SdkLogger> {
    OpenTelemetryTracingBridge::new(logger_provider)
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
    use opentelemetry::Key;
    use opentelemetry_sdk::{logs::LoggerProvider, testing::logs::InMemoryLogExporter};
    use tracing_subscriber::{layer::SubscriberExt, Registry};

    use super::{build_log_bridge_layer, build_resource, redact_if_sensitive};

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

    #[test]
    fn log_bridge_exports_tracing_event_with_service_name_resource() {
        let exporter = InMemoryLogExporter::default();
        let logger_provider = LoggerProvider::builder()
            .with_simple_exporter(exporter.clone())
            .with_resource(build_resource("aiwattcoach-test"))
            .build();
        let subscriber = Registry::default().with(build_log_bridge_layer(&logger_provider));
        let _default = tracing::subscriber::set_default(subscriber);

        tracing::info!(target: "telemetry-test", event_name = "telemetry.started", "bridge log message");

        assert!(logger_provider
            .force_flush()
            .into_iter()
            .all(|result| result.is_ok()));

        let logs = exporter
            .get_emitted_logs()
            .expect("in-memory exporter should expose emitted logs");
        let log = logs.first().expect("expected one emitted log");

        assert_eq!(logs.len(), 1);
        assert_eq!(log.record.target.as_deref(), Some("telemetry-test"));
        assert_eq!(
            log.resource
                .get(Key::new("service.name"))
                .map(|value| value.to_string()),
            Some("aiwattcoach-test".to_string())
        );
    }
}
