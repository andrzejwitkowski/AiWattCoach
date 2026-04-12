# Test Auth Rest Tracing Capture

> 20 nodes · cohesion 0.23

## Key Concepts

- **capture_tracing_logs()** (8 connections) — `tests\support\tracing_capture.rs`
- **tracing_capture.rs** (7 connections) — `tests\health_check\tracing_capture.rs`
- **tracing_capture.rs** (7 connections) — `tests\support\tracing_capture.rs`
- **GlobalLogWriter** (7 connections) — `tests\support\tracing_capture.rs`
- **SharedLogBuffer** (7 connections) — `tests\support\tracing_capture.rs`
- **tracing_capture.rs** (6 connections) — `tests\auth_rest\tracing_capture.rs`
- **tracing_capture.rs** (6 connections) — `tests\settings_rest\tracing_capture.rs`
- **ActiveLogBufferGuard** (6 connections) — `tests\support\tracing_capture.rs`
- **global_log_writer_commits_each_event_atomically()** (6 connections) — `tests\support\tracing_capture.rs`
- **GlobalLogRouter** (5 connections) — `tests\support\tracing_capture.rs`
- **init_test_tracing_subscriber()** (5 connections) — `tests\support\tracing_capture.rs`
- **.drop()** (3 connections) — `tests\support\tracing_capture.rs`
- **.install()** (3 connections) — `tests\support\tracing_capture.rs`
- **.contents()** (3 connections) — `tests\support\tracing_capture.rs`
- **.make_writer()** (2 connections) — `tests\support\tracing_capture.rs`
- **.drop()** (1 connections) — `tests\support\tracing_capture.rs`
- **.flush()** (1 connections) — `tests\support\tracing_capture.rs`
- **.write()** (1 connections) — `tests\support\tracing_capture.rs`
- **.flush()** (1 connections) — `tests\support\tracing_capture.rs`
- **.write()** (1 connections) — `tests\support\tracing_capture.rs`

## Relationships

- No strong cross-community connections detected

## Source Files

- `tests\auth_rest\tracing_capture.rs`
- `tests\health_check\tracing_capture.rs`
- `tests\settings_rest\tracing_capture.rs`
- `tests\support\tracing_capture.rs`

## Audit Trail

- EXTRACTED: 86 (100%)
- INFERRED: 0 (0%)
- AMBIGUOUS: 0 (0%)

---

*Part of the graphify knowledge wiki. See [[index]] to navigate.*