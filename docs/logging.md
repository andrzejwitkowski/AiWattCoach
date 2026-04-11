# Logging Guide

This document explains how to add logging for new REST endpoints and new external HTTP clients in `AiWattCoach`.

## Goals

- Keep logs structured and consistent.
- Preserve trace propagation.
- Default to low-risk logging for request and response bodies.
- Redact secrets before they reach logs.
- Put transport logging in adapters, not domain code.

## REST Endpoint Logging

REST request and response body logging lives in `src/adapters/rest/logging/`.

Main pieces:

- `RequestLogLayer` in `src/adapters/rest/logging/request_logger.rs`
- `EndpointLogConfig` and `with_log_config(...)` in `src/adapters/rest/logging/mod.rs`
- redaction helpers in `src/adapters/rest/logging/redaction.rs`

### Default behavior

- `insert_default_log_config(...)` in `src/adapters/rest/mod.rs` inserts the default per-request logging config.
- By default, request and response body logging is off.
- `ENABLE_ENDPOINT_BODY_LOGGING=true` changes that default to `EndpointLogConfig::full()`.
- Route-specific `with_log_config(...)` is authoritative and overrides the default config.

### When to enable endpoint body logging

Use body logging only when the route needs extra observability and the payload shape is safe enough after redaction.

Typical choices:

- `EndpointLogConfig::request_only()` for write endpoints where request debugging matters more than response payloads.
- `EndpointLogConfig::response_only()` for read endpoints where the response shape matters.
- `EndpointLogConfig::full()` only for narrowly scoped troubleshooting paths or explicitly small/safe payloads.

Always set a route-specific preview cap with `with_max_body_bytes(...)` when body logging is enabled.

### Body limits

`with_max_body_bytes(...)` only limits what is written to logs. It does not limit how much of the request body the transport accepts.

If `RequestLogLayer` can buffer the request body, also add a transport-level limit such as `DefaultBodyLimit::max(...)` before the logging layer.

Use this ordering pattern:

```rust
.route("/api/example", post(handler))
.layer(DefaultBodyLimit::max(8 * 1024))
.layer(RequestLogLayer::new())
.layer(with_log_config(
    EndpointLogConfig::request_only().with_max_body_bytes(1024),
))
```

### Endpoint example

```rust
use axum::{extract::DefaultBodyLimit, routing::post, Router};

use crate::adapters::rest::logging::{with_log_config, EndpointLogConfig};
use crate::adapters::rest::logging::request_logger::RequestLogLayer;

let router = Router::new()
    .route("/api/settings/intervals/test", post(test_intervals_connection))
    .layer(DefaultBodyLimit::max(8 * 1024))
    .layer(RequestLogLayer::new())
    .layer(with_log_config(
        EndpointLogConfig::request_only().with_max_body_bytes(1024),
    ));
```

### Redaction rules

- JSON request and response bodies are parsed and redacted before preview logging.
- Sensitive headers are redacted by name.
- Binary and non-JSON textual bodies are summarized instead of logged raw in the REST adapter.
- If a route handles secrets or large uploads, prefer request-only or response-only logging, or leave body logging off.

### Endpoint checklist

- Keep the handler thin; logging stays in the REST adapter layer.
- Add `RequestLogLayer` only where endpoint body logging is needed.
- Add `with_log_config(...)` for the route.
- Add `DefaultBodyLimit::max(...)` when request buffering is possible.
- Use the smallest useful `max_body_bytes` preview limit.
- Add or update REST observability tests when behavior changes.

## External Client Logging

External HTTP client logging belongs in adapter code, not in domain services or REST handlers.

Current example:

- `src/adapters/intervals_icu/client/logging.rs`

Main helpers:

- `execute_and_log(...)`
- `execute_and_log_no_body(...)`
- adapter-local convenience helper: `IntervalsIcuClient::execute_and_log_with_trace_no_body(...)`

### Choosing the logging mode

Use `BodyLoggingMode::None` by default.

Use `BodyLoggingMode::Full` only when all of the following are true:

- the payload is small enough to preview safely,
- the response/request body is actually useful for debugging,
- the data is already redacted or safe to preview,
- the log-volume increase is acceptable.

For most normal Intervals.icu traffic, the safe default is no body preview logging.

### Client example

For a new client with trace propagation and no response/request body previews by default:

```rust
let request = client
    .get(url)
    .basic_auth("API_KEY", Some(&credentials.api_key));

let response = Self::execute_and_log_with_trace_no_body(&client, request)
    .await
    .map_err(map_connection_error)?;
```

If a client needs the generic helper directly:

```rust
let response = logging::execute_and_log(
    &client,
    request_builder,
    logging::BodyLoggingMode::None,
)
.await?;
```

### Trace propagation

Do not bypass trace propagation.

If you are inside `IntervalsIcuClient`, prefer `execute_and_log_with_trace_no_body(...)` so the request goes through `with_trace_context(...)` before execution.

If you add a new client module, keep the same shape:

- one helper that injects trace context into the `RequestBuilder`
- one helper that executes and logs with the chosen body logging mode

### Error logging rules for clients

- Log transport failures before returning the error.
- Sanitize surfaced error URLs so query strings and fragments do not leak.
- For upstream error bodies, prefer a non-reversible summary like `payload bytes=... hash=...` unless a redacted structured preview is explicitly needed and safe.
- Do not re-introduce raw body previews on paths that intentionally opted into `BodyLoggingMode::None`.

### Client checklist

- Keep the logging in the adapter.
- Inject trace context before executing the request.
- Default to `BodyLoggingMode::None`.
- Use summary logging for malformed or unsafe payloads.
- Redact sensitive fields before preview logging.
- Add adapter tests for log behavior when introducing new client logging.

## Verification

When you add or change logging behavior:

- run `cargo clippy --all-targets --all-features -- -D warnings`
- run the most relevant integration tests, usually one or more of:
  - `cargo test --test intervals_adapters -- --nocapture`
  - `cargo test --test intervals_rest -- --nocapture`
  - `cargo test --test settings_rest -- --nocapture`
  - `cargo test --test logs_rest -- --nocapture`

If the change touches broad Rust behavior, run the repo verification flow expected by hooks and CI.
