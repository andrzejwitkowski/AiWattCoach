# OpenRouter Prompt Caching Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Enable OpenRouter prompt caching for workout-summary requests by caching the stable prefix and leaving only the live conversation uncached.

**Architecture:** Extend the OpenRouter adapter to support block-based message content with `cache_control` markers. Keep the change local to the OpenRouter DTO and mapping layers so the domain request shape stays unchanged, and map the workout-summary stable prefix into cacheable blocks while appending the conversation as uncached messages.

**Tech Stack:** Rust, serde, reqwest, OpenRouter chat completions API, cargo test

---

## Task 1: Add a failing OpenRouter mapping test for cached prefix blocks

**Files:**
- Modify: `tests/llm_adapters.rs`
- Modify: `src/adapters/llm/openrouter/mapping.rs`

**Step 1: Write the failing test**

Add a test that builds an `LlmChatRequest` with `system_prompt`, `stable_context`, `volatile_context`, and one user conversation message, then asserts the mapped OpenRouter request:
- emits cacheable block content for the stable prefix
- leaves the conversation uncached

**Step 2: Run test to verify it fails**

Run: `cargo test --test llm_adapters openrouter_request_caches_stable_prefix_only -- --nocapture`
Expected: FAIL because current mapping only emits plain string messages with no cache markers.

**Step 3: Write minimal implementation**

Update OpenRouter DTOs and mapping so the request can represent either plain string content or block-based content with `cache_control`.

**Step 4: Run test to verify it passes**

Run: `cargo test --test llm_adapters openrouter_request_caches_stable_prefix_only -- --nocapture`
Expected: PASS

## Task 2: Keep response parsing and adapter behavior intact

**Files:**
- Modify: `src/adapters/llm/openrouter/dto.rs`
- Modify: `src/adapters/llm/openrouter/mapping.rs`
- Test: `tests/llm_adapters.rs`

**Step 1: Add or update focused tests**

Cover that:
- existing response parsing still works
- cache usage fields continue mapping into domain cache usage

**Step 2: Run focused tests**

Run: `cargo test --test llm_adapters openrouter_client_maps_cache_discount_and_write_tokens openrouter_client_parses_array_content_parts -- --nocapture`
Expected: PASS

**Step 3: Adjust implementation minimally if needed**

Only fix DTO/mapping fallout required by the new request-content shape.

**Step 4: Re-run focused tests**

Run the same command and confirm PASS.

### Task 3: Verify the full LLM adapter suite

**Files:**
- No new files

**Step 1: Run full LLM adapter tests**

Run: `cargo test --test llm_adapters`
Expected: PASS

**Step 2: Run formatting check**

Run: `cargo fmt --all --check`
Expected: PASS

**Step 3: Run clippy**

Run: `cargo clippy --all-targets --all-features -- -D warnings`
Expected: PASS
