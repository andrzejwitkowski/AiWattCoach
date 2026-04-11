pub mod redaction;
pub mod request_logger;
pub mod status_class;

pub use status_class::status_class;

use std::sync::OnceLock;

use tower::{Layer, Service};

/// Per-endpoint logging configuration.
///
/// Attach this to request extensions via a layer to control
/// what gets logged for specific routes.
#[derive(Clone, Debug)]
pub struct EndpointLogConfig {
    /// Whether to log the request body.
    pub log_request_body: bool,
    /// Whether to log the response body.
    pub log_response_body: bool,
    /// Maximum characters/bytes to include in body previews written to logs.
    pub max_body_bytes: usize,
}

impl Default for EndpointLogConfig {
    fn default() -> Self {
        Self {
            log_request_body: false,
            log_response_body: false,
            max_body_bytes: 4096,
        }
    }
}

impl EndpointLogConfig {
    /// Full logging: request + response bodies.
    pub fn full() -> Self {
        Self {
            log_request_body: true,
            log_response_body: true,
            ..Default::default()
        }
    }

    /// Only log response body (common for GET endpoints).
    pub fn response_only() -> Self {
        Self {
            log_response_body: true,
            ..Default::default()
        }
    }

    /// Only log request body (common for write endpoints where you don't care about the response shape).
    pub fn request_only() -> Self {
        Self {
            log_request_body: true,
            ..Default::default()
        }
    }

    /// Set custom body preview size for logging.
    pub fn with_max_body_bytes(mut self, bytes: usize) -> Self {
        self.max_body_bytes = bytes;
        self
    }
}

#[derive(Clone, Debug)]
pub struct InsertLogConfigLayer {
    config: EndpointLogConfig,
}

impl InsertLogConfigLayer {
    fn new(config: EndpointLogConfig) -> Self {
        Self { config }
    }
}

#[derive(Clone, Debug)]
pub struct InsertLogConfigService<S> {
    inner: S,
    config: EndpointLogConfig,
}

impl<S> Layer<S> for InsertLogConfigLayer {
    type Service = InsertLogConfigService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        InsertLogConfigService {
            inner,
            config: self.config.clone(),
        }
    }
}

impl<S, B> Service<axum::http::Request<B>> for InsertLogConfigService<S>
where
    S: Service<axum::http::Request<B>>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: axum::http::Request<B>) -> Self::Future {
        req.extensions_mut().insert(self.config.clone());
        self.inner.call(req)
    }
}

/// Create a layer that inserts the given `EndpointLogConfig` into request extensions.
pub fn with_log_config(config: EndpointLogConfig) -> InsertLogConfigLayer {
    InsertLogConfigLayer::new(config)
}

/// Global toggle for endpoint body logging via environment variable.
///
/// When `ENABLE_ENDPOINT_BODY_LOGGING=true`, the default config becomes `full()`.
pub fn body_logging_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();

    *ENABLED.get_or_init(|| {
        std::env::var("ENABLE_ENDPOINT_BODY_LOGGING")
            .ok()
            .map(|v| v.trim().eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    })
}

#[cfg(test)]
mod tests {
    use axum::{body::Body, http::Request};
    use tower::{service_fn, ServiceExt};

    use super::*;

    #[test]
    fn default_config_disables_body_logging() {
        let config = EndpointLogConfig::default();
        assert!(!config.log_request_body);
        assert!(!config.log_response_body);
        assert_eq!(config.max_body_bytes, 4096);
    }

    #[test]
    fn full_config_enables_both() {
        let config = EndpointLogConfig::full();
        assert!(config.log_request_body);
        assert!(config.log_response_body);
    }

    #[test]
    fn response_only() {
        let config = EndpointLogConfig::response_only();
        assert!(!config.log_request_body);
        assert!(config.log_response_body);
    }

    #[test]
    fn request_only() {
        let config = EndpointLogConfig::request_only();
        assert!(config.log_request_body);
        assert!(!config.log_response_body);
    }

    #[test]
    fn with_max_body_bytes_overrides() {
        let config = EndpointLogConfig::default().with_max_body_bytes(8192);
        assert_eq!(config.max_body_bytes, 8192);
    }

    #[test]
    fn body_logging_flag_default_false() {
        // This tests the env var when not set
        assert!(!body_logging_enabled());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn insert_log_config_service_overrides_existing_config() {
        let mut service = with_log_config(EndpointLogConfig::default()).layer(service_fn(
            |req: Request<Body>| async move {
                let config = req
                    .extensions()
                    .get::<EndpointLogConfig>()
                    .cloned()
                    .unwrap();
                Ok::<_, std::convert::Infallible>(config)
            },
        ));

        let mut req = Request::builder().uri("/").body(Body::empty()).unwrap();
        req.extensions_mut().insert(EndpointLogConfig {
            log_request_body: true,
            log_response_body: false,
            max_body_bytes: 123,
        });

        let config = service.ready().await.unwrap().call(req).await.unwrap();

        assert!(!config.log_request_body);
        assert!(!config.log_response_body);
        assert_eq!(config.max_body_bytes, 4096);
    }
}
