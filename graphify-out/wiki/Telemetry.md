# Telemetry

> 28 nodes · cohesion 0.13

## Key Concepts

- **telemetry.rs** (25 connections) — `src\telemetry.rs`
- **setup_telemetry()** (8 connections) — `src\telemetry.rs`
- **build_resource()** (4 connections) — `src\telemetry.rs`
- **resolve_service_name()** (4 connections) — `src\telemetry.rs`
- **resolve_service_name_falls_back_when_env_is_blank()** (4 connections) — `src\telemetry.rs`
- **resolve_service_name_prefers_otel_service_name_env_var()** (4 connections) — `src\telemetry.rs`
- **.shutdown()** (4 connections) — `src\telemetry.rs`
- **build_log_bridge_layer()** (3 connections) — `src\telemetry.rs`
- **combine_shutdown_errors()** (3 connections) — `src\telemetry.rs`
- **log_bridge_exports_active_span_trace_context()** (3 connections) — `src\telemetry.rs`
- **log_bridge_exports_tracing_event_with_service_name_resource()** (3 connections) — `src\telemetry.rs`
- **restore_env_var()** (3 connections) — `src\telemetry.rs`
- **telemetry_env_lock()** (3 connections) — `src\telemetry.rs`
- **TelemetryGuard** (3 connections) — `src\telemetry.rs`
- **build_logger_provider()** (2 connections) — `src\telemetry.rs`
- **build_tracer_provider()** (2 connections) — `src\telemetry.rs`
- **combine_shutdown_errors_preserves_both_messages()** (2 connections) — `src\telemetry.rs`
- **get_otlp_endpoint()** (2 connections) — `src\telemetry.rs`
- **init_telemetry()** (2 connections) — `src\telemetry.rs`
- **is_sensitive_key()** (2 connections) — `src\telemetry.rs`
- **redact_if_sensitive()** (2 connections) — `src\telemetry.rs`
- **.drop()** (2 connections) — `src\telemetry.rs`
- **leaves_non_sensitive_fields_unchanged()** (1 connections) — `src\telemetry.rs`
- **redacts_api_key_like_fields()** (1 connections) — `src\telemetry.rs`
- **redacts_password_like_fields()** (1 connections) — `src\telemetry.rs`
- *... and 3 more nodes in this community*

## Relationships

- No strong cross-community connections detected

## Source Files

- `src\telemetry.rs`

## Audit Trail

- EXTRACTED: 96 (100%)
- INFERRED: 0 (0%)
- AMBIGUOUS: 0 (0%)

---

*Part of the graphify knowledge wiki. See [[index]] to navigate.*