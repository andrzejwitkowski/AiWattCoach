# Logging Guide

This document explains how to add logging for new REST endpoints and new external HTTP clients in `AiWattCoach`.

## Goals

- Keep logs structured and consistent.
- Preserve trace propagation.
- Default to low-risk logging for request and response bodies.
- Redact secrets and PII before they reach logs.
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

### Write endpoint group

All write endpoints (POST, PUT, PATCH) and read endpoints that benefit from observability are grouped into a sub-Router with `RequestLogLayer` and `EndpointLogConfig::request_only()`.

Routes excluded from body logging:

| Route | Reason |
|---|---|
| `/api/auth/google/start`, `/api/auth/google/callback` | Login redirects, no useful body |
| `/api/auth/wahoo/start`, `/api/wahoo/callback` | Login redirects, no useful body |
| `/api/auth/logout` | Logout, no request body |
| `/api/auth/me` | Session cookie only, no body |
| `/api/logs` | Circular risk — logging about incoming logs |
| `/api/intervals/activities` POST | 16 MB file upload, impractical to buffer |
| `/api/workout-summaries/{id}/ws` | WebSocket, not buffered by RequestLogLayer |
| `/health`, `/ready` | Health checks, no body |

Note: `/api/auth/whitelist` POST is included in the write group. Its `email` field is redacted by `is_sensitive_key`.

The `/api/settings` GET route uses a separate group with `response_only()` logging because its response contains API keys and connection status that need redacted observability.

When adding a new write endpoint, add it to the existing write group sub-Router in `router_with_frontend_dist()` so it gets request body logging automatically.

GET routes inside the write group skip request body collection because `RequestLogService` only buffers the body for POST, PUT, and PATCH methods. Response body logging is controlled by the `EndpointLogConfig`; the write group uses `request_only()` so response bodies are not logged for those routes.

The `DefaultBodyLimit` on a route inside a sub-Router with `RequestLogLayer` only bounds the handler's body limit, not the logger's buffer. The logger caps its own buffer at `MAX_COLLECT_BYTES` (10 MB). For routes that need a tighter transport limit (e.g. `/api/settings/intervals/test` with 8 KB), that limit protects the handler but the logger may still buffer up to 10 MB of the request before the handler sees it.

### When to enable endpoint body logging

Use body logging only when the route needs extra observability and the payload shape is safe enough after redaction.

Typical choices:

- `EndpointLogConfig::request_only()` for write endpoints where request debugging matters more than response payloads.
- `EndpointLogConfig::response_only()` for read endpoints where the response shape matters.
- `EndpointLogConfig::full()` only for narrowly scoped troubleshooting paths or explicitly small/safe payloads.

Always set a route-specific preview cap with `with_max_body_bytes(...)` when body logging is enabled.

### Body limits

`with_max_body_bytes(...)` only limits what is written to logs. It does not limit how much of the request body the transport accepts. The default is 200 KB (204800 bytes).

The `RequestLogLayer` has a hard cap of 10 MB (`MAX_COLLECT_BYTES`) on how much body it will buffer. This prevents unbounded memory allocation when a route with body logging enabled receives an unexpectedly large payload. Routes that accept genuinely large bodies (like file uploads) should be excluded from body logging entirely, as buffering multi-megabyte payloads is impractical.

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

Sensitive key patterns (checked case-insensitively by `is_sensitive_key` in `src/telemetry.rs`):

| Pattern | Catches | Examples |
|---|---|---|
| `password` | passwords | `password`, `db_password` |
| `secret` | secrets | `client_secret`, `secret` |
| `token` | tokens | `access_token`, `refreshToken` |
| `username` | usernames | `username` |
| `api_key`, `api-key`, `apikey` | API keys | `apiKey`, `x-api-key` |
| `user` (exact) or `_user` suffix | user identifiers | `user`, `db_user` |
| `email` | emails | `email`, `userEmail` |
| `medication` | health data | `medications` |
| `fullname` (exact) or `full_name` (contains) | real names | `fullName` |
| `age` (exact) | age | `age` |
| `weightkg` (exact) or `weight_kg` (contains) | body weight | `weightKg`, `weight_kg` |
| `heightcm` (exact) or `height_cm` (contains) | height | `heightCm`, `height_cm` |
| `hrmax` or `hr_max` | max heart rate | `hrMaxBpm`, `hr_max_bpm` |
| `vo2max` or `vo2_max` | VO2 max | `vo2Max`, `vo2_max` |
| `athletenotes` (exact) or `athlete_notes` (contains) | personal notes | `athleteNotes` |
| `athleteprompt` (exact) or `athlete_prompt` (contains) | personal prompts | `athletePrompt` |
| `athleteid` or `athlete_id` | third-party IDs | `athleteId` |
| `content` (exact) | message content | `content` |
| `message` (exact), `usermessage`, `coachmessage`, `messages` | log/chat messages | `message`, `userMessage`, `coachMessage`, `messages` |
| `filecontent` or `file_content` | binary payloads | `fileContents`, `fileContentsBase64` |

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
