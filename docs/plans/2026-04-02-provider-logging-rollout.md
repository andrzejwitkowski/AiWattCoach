# Provider Logging Rollout Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add safe, actionable diagnostics to the OpenAI and Gemini adapters so live LLM failures can be debugged without changing REST-layer behavior.

**Architecture:** Mirror the balanced OpenRouter logging pattern inside `openai/client.rs` and `gemini/client.rs`. Read successful response bodies as raw text before parsing so parse failures can log the upstream payload. Keep request payload logging limited to metadata only.

**Tech Stack:** Rust, reqwest, tracing, Axum integration tests

---

### Task 1: Add balanced diagnostics to the OpenAI adapter

**Files:**
- Modify: `src/adapters/llm/openai/client.rs`

**Step 1: Write minimal implementation**

Add request-start logs, transport-failure logs, non-success logs with truncated response body, and parse/mapping failure logs. Read the successful body as text before `serde_json` parsing.

**Step 2: Run focused verification**

Run: `cargo test openai_client_maps_response_and_cached_tokens --test llm_adapters -- --nocapture`

Expected: PASS.

### Task 2: Add balanced diagnostics to the Gemini adapter

**Files:**
- Modify: `src/adapters/llm/gemini/client.rs`

**Step 1: Write minimal implementation**

Add request-start logs for cache-create and generate calls, transport-failure logs, non-success logs with truncated response body, and parse/mapping failure logs. Keep cache creation behavior unchanged.

**Step 2: Run focused verification**

Run: `cargo test gemini_client_creates_cache_and_reuses_cached_content --test llm_adapters -- --nocapture`

Expected: PASS.

### Task 3: Verify nearby LLM integration coverage

**Files:**
- No additional code changes expected

**Step 1: Run focused provider and REST tests**

Run: `cargo test openai_client_maps_forbidden_to_credentials_not_configured --test llm_adapters -- --nocapture`

Run: `cargo test gemini_client_skips_cache_creation_without_durable_cache_keys --test llm_adapters -- --nocapture`

Run: `cargo test ai_settings_test_uses_live_openrouter_adapter_and_auth_header --test llm_rest -- --nocapture`

Run: `cargo fmt --all --check`

Expected: PASS.
