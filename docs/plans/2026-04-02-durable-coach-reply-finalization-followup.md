# Durable Coach Reply Finalization Follow-Up Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Close the remaining post-provider durability gap so a completed LLM call is never repeated just because the next local persistence step fails transiently.

**Architecture:** Keep `CoachReplyOperation` as the single durable workflow record and add narrowly scoped retry handling around the post-provider persistence steps in `WorkoutSummaryService`. Preserve the current replay/finalization model and update the state-machine doc so it matches the final recovery behavior.

**Tech Stack:** Rust, MongoDB, Axum, tracing, existing `workout_summary_service` tests and architecture docs

---

### Task 1: Lock in the remaining post-provider durability gaps with failing tests

**Files:**
- Modify: `tests/workout_summary_service/messaging.rs`
- Modify: `tests/workout_summary_service/shared.rs`

**Step 1: Write the failing test for transient success-checkpoint failure**

Add a regression test where:

- the provider returns a successful reply
- the first `reply_operations.upsert()` for `record_provider_response(...)` fails once
- the same `generate_coach_reply(...)` call still succeeds after retrying locally
- the provider is called exactly once

**Step 2: Run the targeted test to verify it fails**

Run: `cargo test --test workout_summary_service generate_coach_reply_retries_success_checkpoint_write_before_returning -- --nocapture`
Expected: FAIL because the current implementation returns the repository error immediately.

**Step 3: Write the failing test for transient failure-checkpoint failure**

Add a regression test where:

- the provider returns an error
- the first `reply_operations.upsert()` for `mark_failed(...)` fails once
- the same `generate_coach_reply(...)` call still returns the original `WorkoutSummaryError::Llm(...)`
- the provider is called exactly once

**Step 4: Run the targeted test to verify it fails**

Run: `cargo test --test workout_summary_service generate_coach_reply_retries_failure_checkpoint_write_before_returning -- --nocapture`
Expected: FAIL because the current implementation returns the repository error immediately.

**Step 5: Commit**

```bash
git add tests/workout_summary_service/messaging.rs tests/workout_summary_service/shared.rs
git commit -m "test: cover transient post-provider checkpoint failures"
```

### Task 2: Add minimal one-shot fault injection for the test repositories

**Files:**
- Modify: `tests/workout_summary_service/shared.rs`
- Test: `tests/workout_summary_service/messaging.rs`

**Step 1: Add one-shot helper knobs**

Extend the in-memory reply-operation repository with targeted one-shot failures for:

- the next pending upsert
- the next failed upsert

Keep the helpers explicit and state-specific. Do not add a general-purpose failure framework.

**Step 2: Run the targeted tests again**

Run: `cargo test --test workout_summary_service generate_coach_reply_retries -- --nocapture`
Expected: FAIL, but now via the intended persistence-window failures.

**Step 3: Commit**

```bash
git add tests/workout_summary_service/shared.rs tests/workout_summary_service/messaging.rs
git commit -m "test: add targeted post-provider upsert failure fakes"
```

### Task 3: Retry transient post-provider operation writes before returning

**Files:**
- Modify: `src/domain/workout_summary/service.rs`
- Test: `tests/workout_summary_service/messaging.rs`

**Step 1: Add a small helper for transient operation-write retries**

Implement the smallest helper needed in `WorkoutSummaryService` to retry `reply_operations.upsert(...)` a bounded number of times for post-provider writes only.

Use it for:

- `record_provider_response(...)`
- `mark_failed(...)`
- `mark_completed(...)` only if the retry is strictly post-provider and does not change semantics

Do not add backoff, timers, or configurable retry policies in this task.

**Step 2: Run the new success-checkpoint test**

Run: `cargo test --test workout_summary_service generate_coach_reply_retries_success_checkpoint_write_before_returning -- --nocapture`
Expected: PASS

**Step 3: Run the new failure-checkpoint test**

Run: `cargo test --test workout_summary_service generate_coach_reply_retries_failure_checkpoint_write_before_returning -- --nocapture`
Expected: PASS

**Step 4: Keep the retry helper tightly scoped**

Make sure the helper is only used after the provider has already returned. Do not change unrelated repository writes such as `append_user_message(...)`, `create_summary(...)`, or `update_rpe(...)`.

**Step 5: Commit**

```bash
git add src/domain/workout_summary/service.rs tests/workout_summary_service/messaging.rs
git commit -m "fix: retry transient post-provider coach reply writes"
```

### Task 4: Verify recovery still stays exactly-once after retry support

**Files:**
- Modify: `tests/workout_summary_service/messaging.rs`
- Possibly modify: `src/domain/workout_summary/service.rs`

**Step 1: Add duplicate-call guard assertions where needed**

Strengthen the recovery tests so they explicitly assert the provider call count remains `1` across:

- transient success checkpoint retry
- transient failure checkpoint retry
- completion finalization retry from an already persisted coach message

**Step 2: Run focused recovery tests**

Run: `cargo test --test workout_summary_service generate_coach_reply_recovers -- --nocapture`
Expected: PASS with no duplicate provider calls.

**Step 3: Commit**

```bash
git add tests/workout_summary_service/messaging.rs src/domain/workout_summary/service.rs
git commit -m "test: assert exactly-once recovery after transient writes"
```

### Task 5: Refresh the state-machine documentation

**Files:**
- Modify: `docs/architecture/llm-coach-reply-state-machine.md`

**Step 1: Update the high-level flow**

Document that after the provider returns, the service now retries transient local persistence failures before surfacing an error.

**Step 2: Update the deferred-gap section**

Narrow the “Still Deferred” section so it reflects only what remains after this follow-up, instead of the already-fixed transient write window.

**Step 3: Run a quick read-through for consistency**

Ensure the doc matches the final code path and does not claim stronger guarantees than the implementation actually provides.

**Step 4: Commit**

```bash
git add docs/architecture/llm-coach-reply-state-machine.md
git commit -m "docs: refresh coach reply recovery state machine"
```

### Task 6: Run focused verification and the backend gate

**Files:**
- No new files

**Step 1: Run the workout-summary service suite**

Run: `cargo test --test workout_summary_service -- --nocapture`
Expected: PASS

**Step 2: Run formatting**

Run: `cargo fmt --all --check`
Expected: PASS

**Step 3: Run linting**

Run: `cargo clippy --all-targets --all-features -- -D warnings`
Expected: PASS

**Step 4: Run the full backend suite**

Run: `cargo test`
Expected: PASS

**Step 5: Commit**

```bash
git add src/domain/workout_summary/service.rs tests/workout_summary_service/messaging.rs tests/workout_summary_service/shared.rs docs/architecture/llm-coach-reply-state-machine.md
git commit -m "harden coach reply finalization after transient write failures"
```

---

## Scope Guardrails

Keep this follow-up focused on transient post-provider local write failures in the workout-summary coach reply workflow.

Do not expand it into:

- background reconciliation jobs
- startup repair passes
- frontend changes
- provider cache changes
- broad error-taxonomy refactors

unless a failing test proves one is directly required.

## Suggested Branch

`feature/durable-coach-reply-finalization`

## Suggested PR Title

`fix: harden coach reply finalization after transient write failures`
