# Athlete Summary Durable Generation Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make athlete-summary generation crash-safe and idempotent by persisting a pending generation operation before the LLM call, while also fixing the new review comments around system-message rendering and test-double fidelity.

**Architecture:** Add a dedicated athlete-summary generation operation model and repository port in the athlete-summary domain, mirroring the repo's existing durable external-workflow patterns without changing the athlete-summary document read model. The service will claim a pending operation before generation, reuse completed work when possible, reclaim failed or stale work, and finalize the operation after persistence. In the frontend, render `system` messages distinctly from coach replies. In test helpers, tighten the fake athlete-summary behavior to match the real service contract more closely.

**Tech Stack:** Rust, Axum, MongoDB, React, TypeScript, Vitest, cargo test

---

### Task 1: Add failing athlete-summary durability tests in the domain

**Files:**
- Modify: `tests/athlete_summary_service.rs`
- Reference: `src/domain/athlete_summary/service.rs`
- Reference: `src/domain/workout_summary/model.rs`

**Step 1: Write the failing tests**

Add focused tests that describe the new durable behavior:
- generating when missing claims a pending operation before calling the generator
- a completed operation with a persisted summary is reused without a second generator call
- a failed operation can be reclaimed and retried
- a stale pending operation can be reclaimed
- if generation succeeds but summary persistence was already completed earlier, the service can recover using the operation record instead of regenerating

Use in-memory fakes that record call order explicitly so the tests assert persistence happens before the generator call.

**Step 2: Run the test file to verify failure**

Run: `cargo test --test athlete_summary_service -- --nocapture`

Expected: FAIL because the athlete-summary domain has no operation record support yet.

**Step 3: Keep the new fake helpers minimal**

Only add the state needed to express pending/completed/failed operation flows. Do not add a generic workflow engine.

**Step 4: Re-run the same test file**

Run: `cargo test --test athlete_summary_service -- --nocapture`

Expected: still FAIL until production code is implemented.

### Task 2: Add athlete-summary generation operation models and ports

**Files:**
- Modify: `src/domain/athlete_summary/model.rs`
- Modify: `src/domain/athlete_summary/ports.rs`
- Modify: `src/domain/athlete_summary/mod.rs`

**Step 1: Add the failing compile target through tests first**

Use the failing tests from Task 1 as the driver. Do not add production code until the tests are already failing for the missing operation types and port methods.

**Step 2: Add minimal domain types**

Add athlete-summary generation workflow types parallel to the workout-summary operation pattern, but scoped only to what athlete-summary needs:
- `AthleteSummaryGenerationOperationStatus` with `Pending`, `Completed`, `Failed`
- `AthleteSummaryGenerationOperation`
- `AthleteSummaryGenerationClaimResult`

Fields should stay minimal and durable, for example:
- `user_id`
- `status`
- `summary_text: Option<String>`
- `provider: Option<String>`
- `model: Option<String>`
- `error_message: Option<String>`
- `started_at_epoch_seconds`
- `last_attempt_at_epoch_seconds`
- `attempt_count`
- `created_at_epoch_seconds`
- `updated_at_epoch_seconds`

Add small helpers for constructing pending/completed/failed states if they reduce service branching.

**Step 3: Extend the repository port**

Add a new athlete-summary operation repository port with only the methods the service needs:
- `find_by_user_id`
- `claim_pending`
- `upsert`

Keep the existing athlete-summary repository port unchanged so the summary document remains the read model.

**Step 4: Export the new types from the module**

Update `mod.rs` to re-export the new operation types and repository trait.

**Step 5: Run the athlete-summary service tests again**

Run: `cargo test --test athlete_summary_service -- --nocapture`

Expected: FAIL in service wiring or missing implementations, not in type resolution.

### Task 3: Implement the durable athlete-summary service flow

**Files:**
- Modify: `src/domain/athlete_summary/service.rs`
- Modify: `tests/athlete_summary_service.rs`

**Step 1: Update the service constructor shape**

Inject the new operation repository into `AthleteSummaryService` alongside the summary repository, generator, and clock.

**Step 2: Implement the claim-before-generate flow**

For `generate_summary(user_id, force)`:
- read the current summary once
- if `force == false` and the summary exists and is fresh, return it immediately
- claim a pending generation operation before the generator call
- if claim returns an existing completed operation with stored summary details and a persisted fresh summary exists, reuse it
- if claim returns failed or stale pending and reclaim is allowed, continue with generation
- call the generator only after the pending operation is durably claimed

**Step 3: Finalize the operation after generation**

After a successful generator result:
- upsert the athlete-summary document
- mark the operation completed with the generated summary metadata

After a failed generator result:
- mark the operation failed with a durable error message

If summary persistence fails after a successful provider response:
- persist a completed operation containing the generated summary payload so the next call can recover without a duplicate LLM call

Keep the summary document as the source for normal reads. The operation record is only for crash-safe generation and recovery.

**Step 4: Keep stale logic unchanged where possible**

Do not change the Monday-based freshness rule unless the failing tests require it.

**Step 5: Run the athlete-summary service tests**

Run: `cargo test --test athlete_summary_service -- --nocapture`

Expected: PASS.

### Task 4: Add Mongo storage for athlete-summary generation operations

**Files:**
- Create: `src/adapters/mongo/athlete_summary_generation_operations.rs`
- Modify: `src/adapters/mongo/mod.rs`
- Modify: `src/main.rs`

**Step 1: Write an adapter-level compile target through existing service tests**

Do not add standalone adapter tests unless needed; let the build fail first because the live app wiring is incomplete.

**Step 2: Implement the Mongo repository**

Add a collection-backed adapter similar to the repo's existing operation repositories:
- unique index on `user_id`
- `claim_pending` that inserts a new pending record or reclaims failed/stale pending records
- `find_by_user_id`
- `upsert`

Store only the fields needed by the domain operation type.

**Step 3: Wire it into app startup**

In `src/main.rs`:
- construct the new Mongo repository
- ensure its indexes
- pass it into `AthleteSummaryService::new(...)`

**Step 4: Run the athlete-summary service tests again**

Run: `cargo test --test athlete_summary_service -- --nocapture`

Expected: PASS.

### Task 5: Tighten the settings-rest athlete-summary fake to match the real contract

**Files:**
- Modify: `tests/settings_rest/shared/athlete_summary.rs`
- Modify: `tests/settings_rest/athlete_summary_endpoints.rs`

**Step 1: Write a failing test if needed**

If there is no existing test that would catch the mismatch, add a focused handler-level test proving repeated generation and fresh reads behave like the real service:
- `generate_summary(..., false)` semantics should reuse a fresh summary
- `generate_summary(..., true)` should regenerate
- `ensure_fresh_summary_state()` should report `was_regenerated` accurately

**Step 2: Run the focused test to verify failure**

Run: `cargo test --test settings_rest athlete_summary_endpoints -- --nocapture`

Expected: FAIL if the fake still diverges.

**Step 3: Update the fake minimally**

Track enough per-user state to distinguish:
- no summary yet
- fresh existing summary
- forced regeneration

Do not add fake-only behavior that the production service does not have.

**Step 4: Re-run the focused test**

Run: `cargo test --test settings_rest athlete_summary_endpoints -- --nocapture`

Expected: PASS.

### Task 6: Add distinct frontend rendering for system messages

**Files:**
- Modify: `frontend/src/features/coach/components/ChatWindow.test.tsx`
- Modify: `frontend/src/features/coach/components/ChatMessage.tsx`

**Step 1: Write the failing test**

Add a focused test that renders a `role: 'system'` message and asserts it is presented differently from a coach reply. Keep the assertion concrete and stable, for example by checking for system-specific container text or class markers that are intentionally unique.

**Step 2: Run the frontend test to verify failure**

Run: `bun run --cwd frontend test src/features/coach/components/ChatWindow.test.tsx`

Expected: FAIL because `ChatMessage` currently treats all non-user messages the same.

**Step 3: Implement the minimal UI change**

Update `ChatMessage.tsx` so `system` messages render with their own visual treatment, distinct from both `user` and `coach`. Keep the change local to this component unless the test forces a small shared helper.

**Step 4: Re-run the frontend test**

Run: `bun run --cwd frontend test src/features/coach/components/ChatWindow.test.tsx`

Expected: PASS.

### Task 7: Verify the new branch behavior end-to-end

**Files:**
- Modify only if verification exposes a real issue

**Step 1: Run focused Rust tests**

Run:
- `cargo test --test athlete_summary_service -- --nocapture`
- `cargo test --test settings_rest athlete_summary_endpoints -- --nocapture`

Expected: PASS.

**Step 2: Run the focused frontend test**

Run: `bun run --cwd frontend test src/features/coach/components/ChatWindow.test.tsx`

Expected: PASS.

**Step 3: Run the repo-level verification**

Run: `bun run verify:all`

Expected: PASS.

**Step 4: Commit**

```bash
git add src/domain/athlete_summary src/adapters/mongo src/main.rs tests/athlete_summary_service.rs tests/settings_rest/shared/athlete_summary.rs tests/settings_rest/athlete_summary_endpoints.rs frontend/src/features/coach/components/ChatMessage.tsx frontend/src/features/coach/components/ChatWindow.test.tsx docs/plans/2026-04-04-athlete-summary-durable-generation-implementation-plan.md
git commit -m "fix: make athlete summary generation crash safe"
```
