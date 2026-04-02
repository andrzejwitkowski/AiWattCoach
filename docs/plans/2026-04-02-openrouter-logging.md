# OpenRouter Logging Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add safe, actionable OpenRouter adapter diagnostics so live `/api/settings/ai-agents/test` failures show upstream status and response details.

**Architecture:** Keep the change local to the OpenRouter HTTP adapter. Log sanitized request metadata before sending, log transport failures from reqwest, and log truncated upstream response bodies for non-2xx responses. Do not change REST behavior or error mapping in this plan.

**Tech Stack:** Rust, reqwest, tracing, Axum integration tests

---

### Task 1: Add a failing test for log-body sanitization

**Files:**
- Modify: `src/adapters/llm/openrouter/client.rs`

**Step 1: Write the failing test**

Add a small unit test for a helper that truncates long upstream response bodies to a stable maximum length and appends an explicit truncation marker.

**Step 2: Run test to verify it fails**

Run: `cargo test truncates_logged_openrouter_response_bodies --lib -- --nocapture`

Expected: FAIL because the helper does not exist yet.

### Task 2: Add structured OpenRouter diagnostics

**Files:**
- Modify: `src/adapters/llm/openrouter/client.rs`

**Step 1: Write minimal implementation**

Add a small helper for truncating logged response text. Log request metadata before the HTTP call, log reqwest transport failures, and log non-success upstream responses with `status`, `model`, `url`, and sanitized body text.

**Step 2: Run focused tests to verify it passes**

Run: `cargo test truncates_logged_openrouter_response_bodies --lib -- --nocapture`

Expected: PASS.

### Task 3: Verify adapter and REST coverage still pass

**Files:**
- No additional code changes expected

**Step 1: Run focused adapter and REST tests**

Run: `cargo test openrouter_client_maps_cache_discount_and_write_tokens --test llm_adapters -- --nocapture`

Run: `cargo test openrouter_client_maps_payment_required_to_provider_rejected --test llm_adapters -- --nocapture`

Run: `cargo test ai_settings_test_uses_live_openrouter_adapter_and_auth_header --test llm_rest -- --nocapture`

Run: `cargo fmt --all --check`

Expected: PASS.
