use std::{borrow::Cow, env, error::Error, io::Error as IoError};

use opentelemetry::trace::TracerProvider as _;
use opentelemetry::KeyValue;
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_otlp::{LogExporter, WithExportConfig};
use opentelemetry_sdk::{
    logs::{SdkLogger, SdkLoggerProvider},
    trace::SdkTracerProvider,
    Resource,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Registry};

const REDACTED_VALUE: &str = "[REDACTED]";

#[derive(Debug)]
pub struct TelemetryGuard {
    tracer_provider: Option<SdkTracerProvider>,
    logger_provider: Option<SdkLoggerProvider>,
}

impl TelemetryGuard {
    pub fn shutdown(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let tracer_shutdown_error = self
            .tracer_provider
            .take()
            .and_then(|tracer_provider| tracer_provider.shutdown().err())
            .map(|error| Box::new(error) as Box<dyn Error + Send + Sync>);

        let logger_shutdown_error = self
            .logger_provider
            .take()
            .and_then(|logger_provider| logger_provider.shutdown().err())
            .map(|error| Box::new(error) as Box<dyn Error + Send + Sync>);

        match (tracer_shutdown_error, logger_shutdown_error) {
            (None, None) => Ok(()),
            (Some(error), None) | (None, Some(error)) => Err(error),
            (Some(tracer_error), Some(logger_error)) => Err(combine_shutdown_errors(
                "tracer",
                tracer_error,
                "logger",
                logger_error,
            )),
        }
    }
}

fn combine_shutdown_errors(
    first_label: &str,
    first_error: Box<dyn Error + Send + Sync>,
    second_label: &str,
    second_error: Box<dyn Error + Send + Sync>,
) -> Box<dyn Error + Send + Sync> {
    Box::new(IoError::other(format!(
        "{first_label} shutdown failed: {first_error}; {second_label} shutdown failed: {second_error}"
    )))
}

impl Drop for TelemetryGuard {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

pub fn setup_telemetry(service_name: &str) -> Result<TelemetryGuard, Box<dyn Error + Send + Sync>> {
    let effective_service_name = resolve_service_name(service_name);
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

fn resolve_service_name(service_name: &str) -> String {
    env::var("OTEL_SERVICE_NAME")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| service_name.to_string())
}

fn build_resource(service_name: &str) -> Resource {
    Resource::builder_empty()
        .with_attributes([KeyValue::new("service.name", service_name.to_owned())])
        .build()
}

fn build_tracer_provider(
    resource: &Resource,
    endpoint: Option<&str>,
) -> Result<Option<SdkTracerProvider>, Box<dyn Error + Send + Sync>> {
    let Some(endpoint) = endpoint else {
        return Ok(None);
    };

    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(endpoint.to_string())
        .build()?;

    Ok(Some(
        SdkTracerProvider::builder()
            .with_batch_exporter(exporter)
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
            .with_batch_exporter(exporter)
            .with_resource(resource.clone())
            .build(),
    ))
}

fn build_log_bridge_layer(
    logger_provider: &SdkLoggerProvider,
) -> OpenTelemetryTracingBridge<SdkLoggerProvider, SdkLogger> {
    OpenTelemetryTracingBridge::new(logger_provider)
}

pub fn is_sensitive_key(key: &str) -> bool {
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
    use std::{
        env,
        ffi::OsString,
        sync::{Mutex, OnceLock},
    };

    use opentelemetry::{
        trace::{Tracer as _, TracerProvider as _},
        Key,
    };
    use opentelemetry_sdk::{
        logs::{InMemoryLogExporter, SdkLoggerProvider},
        trace::{InMemorySpanExporter, SdkTracerProvider},
    };
    use tracing_subscriber::{layer::SubscriberExt, Registry};

    use super::{
        build_log_bridge_layer, build_resource, combine_shutdown_errors, redact_if_sensitive,
        resolve_service_name,
    };

    fn telemetry_env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

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
        let logger_provider = SdkLoggerProvider::builder()
            .with_simple_exporter(exporter.clone())
            .with_resource(build_resource("aiwattcoach-test"))
            .build();
        let subscriber = Registry::default().with(build_log_bridge_layer(&logger_provider));
        let _default = tracing::subscriber::set_default(subscriber);

        tracing::info!(target: "telemetry-test", event_name = "telemetry.started", "bridge log message");

        assert!(logger_provider.force_flush().is_ok());

        let logs = exporter
            .get_emitted_logs()
            .expect("in-memory exporter should expose emitted logs");
        let log = logs.first().expect("expected one emitted log");

        assert_eq!(logs.len(), 1);
        assert_eq!(
            log.record.target().map(|target| target.as_ref()),
            Some("telemetry-test")
        );
        assert_eq!(
            log.resource
                .get(&Key::new("service.name"))
                .map(|value| value.to_string()),
            Some("aiwattcoach-test".to_string())
        );
    }

    #[test]
    fn log_bridge_exports_active_span_trace_context() {
        let log_exporter = InMemoryLogExporter::default();
        let span_exporter = InMemorySpanExporter::default();
        let logger_provider = SdkLoggerProvider::builder()
            .with_simple_exporter(log_exporter.clone())
            .with_resource(build_resource("aiwattcoach-test"))
            .build();
        let tracer_provider = SdkTracerProvider::builder()
            .with_simple_exporter(span_exporter.clone())
            .with_resource(build_resource("aiwattcoach-test"))
            .build();
        let subscriber = Registry::default()
            .with(build_log_bridge_layer(&logger_provider))
            .with(
                tracing_opentelemetry::layer()
                    .with_tracer(tracer_provider.tracer("aiwattcoach-test")),
            );
        let _default = tracing::subscriber::set_default(subscriber);

        tracer_provider
            .tracer("aiwattcoach-test")
            .in_span("telemetry-span", |_| {
                tracing::info!(target: "telemetry-test", "bridge log in active span");
            });

        assert!(logger_provider.force_flush().is_ok());
        assert!(tracer_provider.force_flush().is_ok());

        let logs = log_exporter
            .get_emitted_logs()
            .expect("in-memory exporter should expose emitted logs");
        let spans = span_exporter
            .get_finished_spans()
            .expect("in-memory exporter should expose finished spans");
        let log = logs.first().expect("expected one emitted log");
        let span = spans
            .iter()
            .find(|span| span.name.as_ref() == "telemetry-span")
            .expect("expected tracing span to be exported");
        let trace_context = log
            .record
            .trace_context()
            .expect("expected log trace context when emitted inside span");

        assert_eq!(trace_context.trace_id, span.span_context.trace_id());
        assert_eq!(trace_context.span_id, span.span_context.span_id());
    }

    #[test]
    fn resolve_service_name_prefers_otel_service_name_env_var() {
        let _guard = telemetry_env_lock()
            .lock()
            .expect("telemetry env lock should not be poisoned");
        let original = env::var_os("OTEL_SERVICE_NAME");

        env::set_var("OTEL_SERVICE_NAME", "env-service");

        let effective = resolve_service_name("fallback-service");

        restore_env_var("OTEL_SERVICE_NAME", original);

        assert_eq!(effective, "env-service");
    }

    #[test]
    fn resolve_service_name_falls_back_when_env_is_blank() {
        let _guard = telemetry_env_lock()
            .lock()
            .expect("telemetry env lock should not be poisoned");
        let original = env::var_os("OTEL_SERVICE_NAME");

        env::set_var("OTEL_SERVICE_NAME", "   ");

        let effective = resolve_service_name("fallback-service");

        restore_env_var("OTEL_SERVICE_NAME", original);

        assert_eq!(effective, "fallback-service");
    }

    #[test]
    fn combine_shutdown_errors_preserves_both_messages() {
        let error = combine_shutdown_errors(
            "tracer",
            Box::new(std::io::Error::other("tracer boom")),
            "logger",
            Box::new(std::io::Error::other("logger boom")),
        );

        assert!(error.to_string().contains("tracer boom"));
        assert!(error.to_string().contains("logger boom"));
    }

    fn restore_env_var(key: &str, value: Option<OsString>) {
        match value {
            Some(value) => env::set_var(key, value),
            None => env::remove_var(key),
        }
    }
}
