# Issue 61 Logging Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Finish and harden the `General http client logging` work so backend endpoint logging and Intervals client logging are both safe, actually wired, redacted, and traceable.

**Architecture:** Keep the current split between REST logging and Intervals adapter logging, but finish both paths with minimal changes. First remove the dangerous body-mutation behavior from the REST middleware, then make route-level logging truly declarative, then wire outbound client logging into real Intervals call paths and close redaction gaps in error logging.

**Tech Stack:** Rust, Axum, Tower middleware, reqwest, tracing, OpenTelemetry, existing Rust integration and unit test suites.

---

### Scope
Keep this plan focused on issue `#61`. Do not expand into generic logging abstractions for every adapter/client in the repo yet.

### Files Likely To Change
- Modify: `src/adapters/rest/logging/request_logger.rs`
- Modify: `src/adapters/rest/logging/mod.rs`
- Modify: `src/adapters/rest/logging/redaction.rs`
- Modify: `src/adapters/rest/mod.rs`
- Modify: `src/adapters/intervals_icu/client/logging.rs`
- Modify: `src/adapters/intervals_icu/client/api.rs`
- Modify: `src/adapters/intervals_icu/client/details.rs`
- Modify: `src/adapters/intervals_icu/client/errors.rs`
- Possibly add tests in:
  - `tests/health_check/observability.rs`
  - `tests/intervals_rest/error_logging.rs`
  - `tests/intervals_adapters/activity_lists.rs`
  - `tests/intervals_adapters/activity_mutations.rs`
  - `tests/intervals_adapters/activity_details.rs`
  - or a new focused suite like `tests/intervals_adapters/logging.rs` if that keeps things smaller and clearer

### Task 1: Make REST Body Logging Safe

**Purpose:** Ensure logging never changes live request/response bodies and does no buffering work unless a route actually requested body logging.

**Changes**
- Rework `RequestLogLayer` so it only buffers request/response bodies when `EndpointLogConfig` says it should.
- Never rebuild requests/responses from truncated bytes. Logging preview can be truncated, but the forwarded body must remain complete.
- If full-body-preserving buffering is too invasive for some response types, fall back to skipping body logging rather than mutating traffic.

**Implementation Notes**
- The current bug is in `src/adapters/rest/logging/request_logger.rs`.
- The key invariant: preview truncation is for logs only, never for runtime body reconstruction.
- The middleware should short-circuit quickly for the default config.

**Tests**
- Add a focused test proving a request body larger than `max_body_bytes` still reaches the handler intact.
- Add a focused test proving a response body larger than `max_body_bytes` still reaches the caller intact.
- Add a test proving no body buffering work happens when both body flags are false.

**Verification**
- `cargo test <targeted REST logging tests>`
- `cargo fmt --all --check`
- `cargo clippy --all-targets --all-features -- -D warnings`

### Task 2: Finish Declarative Per-Endpoint REST Logging

**Purpose:** Deliver the declarative per-endpoint backend logging part of issue `#61`.

**Changes**
- Use `with_log_config(...)` on concrete routes in `src/adapters/rest/mod.rs`.
- Start with a minimal, intentional set:
  - `POST /api/logs`: no body logging
  - settings connection-test endpoints: request-only or response-only depending on payload sensitivity
  - safe GET endpoints that return small JSON payloads: response-only
  - upload and download routes: no body logging
- Keep the global env flag only if it still adds value, but route-level config should become the primary control surface.

**Implementation Notes**
- Avoid trying to configure every route at once if the choices are unclear.
- Prefer explicit opt-in for body logging on a few routes rather than implicit logging everywhere.
- Keep handler code thin; all behavior stays in adapter middleware/router wiring.

**Tests**
- Add an integration test proving a configured route emits body logs.
- Add an integration test proving a non-configured route does not emit body logs.
- Add a test for the env flag behavior only if that path remains after refactor.

**Verification**
- `cargo test --test health_check`
- `cargo test --test settings_rest`
- `cargo test --test logs_rest`

### Task 3: Harden REST Redaction Rules

**Purpose:** Prevent secret leakage in truncated JSON and non-JSON text payloads.

**Changes**
- Fix request and response redaction flow so JSON is redacted before preview truncation.
- For non-JSON text bodies, including `application/x-www-form-urlencoded`, do not log raw content. Log only content type, size, and hash style summaries.
- Keep binary bodies as size and hash only.

**Implementation Notes**
- The current gap is in `src/adapters/rest/logging/request_logger.rs` and `src/adapters/rest/logging/redaction.rs`.
- Favor conservative logging over heuristic parsing for sensitive text encodings.

**Tests**
- Add a test with a large JSON body containing `token` or `password` fields that confirms secrets are still redacted in preview logs.
- Add a test covering form-encoded input so raw secrets are not emitted.
- Add a test for header redaction on combined body and header logging.

**Verification**
- `cargo test <targeted redaction tests>`
- `cargo test --test settings_rest`
- `cargo clippy --all-targets --all-features -- -D warnings`

### Task 4: Improve REST Traceability For Body Logs

**Purpose:** Make body log events correlate cleanly with the existing request span.

**Changes**
- Ensure request and response body log events are emitted within the request span or include the same trace-identifying fields.
- Include stable correlation fields such as:
  - `trace_id`
  - `http.route` when available
  - `http.method`
  - `http.status_code` on response logs

**Implementation Notes**
- The current tracing foundation in `src/adapters/rest/mod.rs` is already good.
- Prefer reusing the current span over inventing new correlation mechanisms.

**Tests**
- Extend an observability integration test to assert body logs are correlated with the request trace and span context.
- Keep this focused on one representative endpoint.

**Verification**
- `cargo test --test health_check`
- `cargo test --test auth_rest`
- `cargo test --test settings_rest`

### Task 5: Wire Intervals Client Logging Into Real Runtime Paths

**Purpose:** Make the new outbound HTTP logging actually run for Intervals requests.

**Changes**
- Replace direct `.send()` use in `src/adapters/intervals_icu/client/api.rs` and `src/adapters/intervals_icu/client/details.rs` with the new logging helpers, or introduce one thin shared send helper that always applies:
  - trace context injection
  - request and response logging policy
  - body versus no-body choice
- Use no-body logging for large multipart or binary-heavy requests like upload and download.
- Keep call sites minimal and explicit.

**Implementation Notes**
- This should stay local to the Intervals adapter for now.
- A small shared helper inside `src/adapters/intervals_icu/client/` is enough; do not build a repo-wide logging framework yet.
- Make sure the helper preserves the response body for normal JSON parsing downstream.

**Tests**
- Add or extend adapter tests so at least one list call, one mutation call, and one detail-enrichment call exercise the logging path.
- Confirm logging path does not break existing JSON parsing or binary handling.

**Verification**
- `cargo test --test intervals_adapters/events`
- `cargo test --test intervals_adapters/activity_lists`
- `cargo test --test intervals_adapters/activity_mutations`
- `cargo test --test intervals_adapters/activity_details`

### Task 6: Make Intervals Client Logging Truly Per-Client And Safe

**Purpose:** Match issue `#61`'s per-client intent more closely.

**Changes**
- Add explicit client identity fields to Intervals outbound logs, such as `provider = "intervals_icu"` or `http.client = "intervals_icu"`, instead of generic `http_client`.
- Review logged URL fields and avoid leaking sensitive query params.
  - Recommended: log scheme, host, and path while omitting or redacting the query string.
- Keep header redaction consistent with REST redaction rules.

**Implementation Notes**
- The current generic provider field in `src/adapters/intervals_icu/client/logging.rs` is too weak.
- Logging full `http.url` is risky once upload and update query params are present.

**Tests**
- Add a unit test for URL sanitization or redaction.
- Add or update helper tests to assert provider and client identity fields are stable.

**Verification**
- `cargo test <targeted client logging tests>`
- `cargo test --test intervals_adapters/events`

### Task 7: Redact Intervals Error Body Logging Consistently

**Purpose:** Close the raw upstream error-body leak path.

**Changes**
- Change `src/adapters/intervals_icu/client/errors.rs` so error-body capture is sanitized before being included in `IntervalsError::ApiError` and before any `tracing::warn!` fields.
- Update `details.rs` fallback warnings to log sanitized summaries, not raw `failure.response_body`.

**Implementation Notes**
- Reuse the same preview and redaction policy as the outbound logging helper where practical.
- If parsing or redacting arbitrary upstream bodies is unreliable, log only bounded shape and hash summaries.

**Tests**
- Add a test where an upstream error body contains `token` or `api_key` and confirm logs and error text do not include the raw secret.
- Keep the test focused on the mapped error path.

**Verification**
- `cargo test --test intervals_rest error_logging`
- `cargo test --test intervals_adapters/activity_details`

### Task 8: Final Verification And Review Pass

**Purpose:** Prove the issue is actually closed and the branch is safe.

**Checks**
- Run formatting and clippy.
- Run all targeted tests touched by the logging work.
- Run a final broader verification for confidence because logging touches cross-cutting behavior.

**Verification**
- `cargo fmt --all --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --test health_check`
- `cargo test --test settings_rest`
- `cargo test --test logs_rest`
- `cargo test --test intervals_rest`
- `cargo test --test intervals_adapters/events`
- `cargo test --test intervals_adapters/activity_lists`
- `cargo test --test intervals_adapters/activity_mutations`
- `cargo test --test intervals_adapters/activity_details`

### Recommended Execution Order
1. Task 1
2. Task 3
3. Task 2
4. Task 4
5. Task 5
6. Task 6
7. Task 7
8. Task 8

This order reduces risk early: first stop the middleware from changing traffic, then harden redaction, then finish the missing wiring.

### Minimal Acceptance Criteria
- REST logging never mutates request or response bodies.
- At least a few real routes use explicit `EndpointLogConfig`.
- Intervals outbound logging is used by real adapter calls.
- No raw secrets appear in logged headers, body previews, or mapped upstream error bodies.
- Non-JSON text and form payloads are summary-only.
- Body log events are trace-correlated with request spans.
- Targeted tests pass.
