# PR 84 Review Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Resolve all actionable Copilot and CodeRabbit review feedback on PR `#84`, explicitly reply to every review thread, and preserve the chosen redaction policy of keeping nested non-secret fields under sensitive parent objects.

**Architecture:** Keep the existing split between REST logging and Intervals adapter logging. Fix actual correctness and observability gaps with minimal diffs, avoid changing the chosen redaction policy, and distinguish real code fixes from comments already addressed by later commits or comments that need technical pushback.

**Tech Stack:** Rust, Axum, Tower middleware, reqwest, tracing, OpenTelemetry, GitHub CLI.

---

## Scope

Fix all actionable review comments on PR `#84` from Copilot and CodeRabbit. Do not broaden this into a larger logging redesign beyond what the review feedback requires.

## Files Likely To Change

- Modify: `src/adapters/intervals_icu/client/api.rs`
- Modify: `src/adapters/intervals_icu/client/errors.rs`
- Modify: `src/adapters/intervals_icu/client/logging.rs`
- Modify: `src/adapters/rest/logging/mod.rs`
- Modify: `src/adapters/rest/logging/request_logger.rs`
- Modify: `src/adapters/rest/logging/redaction.rs`
- Modify: `src/adapters/rest/mod.rs`
- Modify: `tests/settings_rest/observability.rs`
- Modify: `docs/superpowers/plans/2026-04-11-issue-61-logging-hardening.md`
- Possibly modify: `Cargo.toml` if dependency cleanup becomes part of the final fix

## Review Policy Notes

- The chosen policy for object-valued sensitive parents is: preserve nested non-sensitive fields, but redact nested sensitive fields.
- That means comments asking to redact every descendant under a sensitive object parent should be answered with pushback, not implemented.
- Missing dependency/import comments that were already fixed by commit `ae25637` should be answered on-thread but do not require more code changes.

### Task 1: Normalize Intervals Adapter Error Mapping

**Purpose:** Ensure non-success Intervals responses consistently use the shared mapper and never leak query strings in surfaced provider errors.

**Files:**
- Modify: `src/adapters/intervals_icu/client/api.rs`
- Modify: `src/adapters/intervals_icu/client/errors.rs`
- Test: `tests/intervals_adapters/activity_lists.rs`
- Test: `tests/intervals_adapters/activity_mutations.rs`
- Test: `tests/intervals_adapters/events.rs`

- [ ] Replace direct `IntervalsError::ApiError(...)` returns in `list_activities` and `upload_activity` with `map_error_response_from_logged_response(response).error`.
- [ ] Sanitize `response.url` in `map_error_response_from_logged_response` by removing query and fragment before building the error string.
- [ ] Keep existing semantics where `401` and `403` map to `IntervalsError::CredentialsNotConfigured`.
- [ ] Re-run the Intervals adapter tests that cover list/upload/error handling.

### Task 2: Log Intervals Transport and Read Failures

**Purpose:** Emit safe logs for outbound transport failures before propagating errors back to the caller.

**Files:**
- Modify: `src/adapters/intervals_icu/client/logging.rs`
- Test: `tests/intervals_adapters/*.rs` as needed

- [ ] Wrap `client.execute(request).await` with explicit error logging that records safe request context only.
- [ ] Wrap `response.bytes().await` with explicit error logging that records safe response/read context only.
- [ ] Ensure logged URLs stay sanitized and headers stay redacted.
- [ ] Re-run relevant Intervals adapter tests.

### Task 3: Fix REST Request Logger Body Handling

**Purpose:** Remove unsafe body-rebuild fallbacks and make preview sizing honor route config.

**Files:**
- Modify: `src/adapters/rest/logging/request_logger.rs`
- Test: `src/adapters/rest/logging/request_logger.rs`

- [ ] Replace `http_body_util::BodyExt` usage with Axum-native body collection.
- [ ] Remove `unwrap_or_default()` request/response rebuild paths that silently convert body-read failures into empty payloads.
- [ ] Choose an explicit safe behavior for body-read failures rather than forwarding mutated traffic.
- [ ] Replace hardcoded `1024` and `512` preview limits with the configured `max_body_bytes` value.
- [ ] Re-run focused request logger tests.

### Task 4: Tighten REST Logging Config Semantics

**Purpose:** Make route logging config naming and implementation match behavior and reduce avoidable per-request overhead.

**Files:**
- Modify: `src/adapters/rest/logging/mod.rs`
- Modify: `src/adapters/rest/mod.rs`

- [ ] Replace `http::Request<B>` usages with `axum::http::Request<B>` for consistency with the rest of the adapter.
- [ ] Clarify `max_body_bytes` semantics in code comments and behavior so it matches actual usage.
- [ ] Cache the env-backed default body logging decision instead of reading the environment on every request.
- [ ] Re-run logging config tests and route-level observability tests.

### Task 5: Tighten Settings Observability Assertions

**Purpose:** Make the redaction test prove request body redaction specifically, not just generic redacted output from any field.

**Files:**
- Modify: `tests/settings_rest/observability.rs`

- [ ] Replace the broad `[REDACTED]` assertion with assertions that check for a `request_body` field and the redacted `apiKey` JSON shape.
- [ ] Keep the assertion that the raw secret never appears.
- [ ] Re-run the settings observability test target.

### Task 6: Fix Plan Doc Heading Structure

**Purpose:** Resolve the markdown heading-level nit raised by CodeRabbit.

**Files:**
- Modify: `docs/superpowers/plans/2026-04-11-issue-61-logging-hardening.md`

- [ ] Change the first `### Scope` heading to `## Scope` (or add an `##` heading immediately before it).

### Task 7: Verification

**Purpose:** Verify the review-fix batch with the actual repo commands before replying or committing.

**Files:**
- No code changes

- [ ] Run `cargo fmt --all --check`.
- [ ] Run `cargo clippy --all-targets --all-features -- -D warnings`.
- [ ] Run targeted tests for intervals adapters, REST logging/request logger, logs REST, and settings observability.
- [ ] If targeted verification passes, run the repo-level Rust verification path if needed.

### Task 8: Review Thread Reply Pass

**Purpose:** Reply to every actionable review comment on the PR with either the implemented fix, the already-fixed reference, or technical pushback.

**Files:**
- No repo file changes

- [ ] Reply to all shared-mapper, sanitized-URL, transport-failure logging, request logger, preview-limit, config, test, and heading comments with what changed.
- [ ] Reply to dependency/import comments by pointing to commit `ae25637`.
- [ ] Reply to the subtree-redaction comment with the chosen policy: keep nested non-secret fields while redacting nested sensitive leaves.
- [ ] Optionally update the PR title if a clearer title is still desired.
