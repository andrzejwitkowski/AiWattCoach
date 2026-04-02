# Health Check Log Noise Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Reduce local log noise by downgrading successful `/health` and `/ready` completion logs from `INFO` to `DEBUG` while preserving failure visibility.

**Architecture:** Keep the change inside the shared REST trace logger in `src/adapters/rest/mod.rs`. Add a small route-aware log-level decision so successful health/readiness responses use `DEBUG`, while all existing warning/error behavior stays the same.

**Tech Stack:** Rust, Axum, tower-http tracing, tracing

---

### Task 1: Add a focused test for health/readiness success log level

**Files:**
- Modify: `src/adapters/rest/mod.rs`

**Step 1: Write the failing test**

Add a small unit test for the route/status log-level selection so `/health` and `/ready` with success statuses resolve to `DEBUG`, while non-health success routes still resolve to `INFO`.

**Step 2: Run test to verify it fails**

Run: `cargo test health_check_successes_log_at_debug --lib -- --nocapture`

Expected: FAIL because the route-aware helper does not exist yet.

### Task 2: Implement route-aware health/readiness log downgrading

**Files:**
- Modify: `src/adapters/rest/mod.rs`

**Step 1: Write minimal implementation**

Add a helper that selects the response log level based on route and status. Use `DEBUG` for successful `/health` and `/ready`, `WARN`/`ERROR` for failures as today, and preserve `INFO` for other successful routes.

**Step 2: Run test to verify it passes**

Run: `cargo test health_check_successes_log_at_debug --lib -- --nocapture`

Expected: PASS.

### Task 3: Verify formatting and nearby behavior

**Files:**
- No additional code changes expected

**Step 1: Run focused verification**

Run: `cargo fmt --all --check`

Expected: PASS.
