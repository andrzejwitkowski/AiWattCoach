# Observability Health Check With Traceparent Logs

> 5 nodes · cohesion 0.40

## Key Concepts

- **observability.rs** (4 connections) — `tests\health_check\observability.rs`
- **health_check_with_traceparent_logs_matching_trace_id()** (1 connections) — `tests\health_check\observability.rs`
- **health_check_without_traceparent_logs_generated_trace_id()** (1 connections) — `tests\health_check\observability.rs`
- **not_found_api_route_emits_warn_classification_log()** (1 connections) — `tests\health_check\observability.rs`
- **readiness_check_emits_error_classification_log_for_service_unavailable()** (1 connections) — `tests\health_check\observability.rs`

## Relationships

- No strong cross-community connections detected

## Source Files

- `tests\health_check\observability.rs`

## Audit Trail

- EXTRACTED: 8 (100%)
- INFERRED: 0 (0%)
- AMBIGUOUS: 0 (0%)

---

*Part of the graphify knowledge wiki. See [[index]] to navigate.*