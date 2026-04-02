# Durable Coach Reply Finalization Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Ensure workout-summary coach replies remain recoverable and replayable even when any post-provider persistence step fails, without issuing duplicate provider calls.

**Architecture:** Keep the workflow orchestration in `src/domain/workout_summary/service.rs`, keep persistence semantics in `src/adapters/mongo/coach_reply_operations.rs` and the summary repository, and preserve the repo rule that durable local state must exist before side effects become irrecoverable. The fix should treat the provider response as a checkpointed workflow stage, so crashes or repository errors after the LLM call can be resumed on the next request instead of re-calling the provider.

**Tech Stack:** Rust, Axum, MongoDB, tracing, existing `workout_summary_service` and REST integration tests

---

### Task 1: Define the failure window and recovery contract

**Files:**
- Read: `src/domain/workout_summary/service.rs`
- Read: `src/domain/workout_summary/model.rs`
- Read: `src/adapters/mongo/coach_reply_operations.rs`
- Test: `tests/workout_summary_service/messaging.rs`

**Step 1: Write the failing recovery tests first**

Add regression tests for these windows:

- provider response obtained, checkpoint write fails once, next request replays without a second provider call
- reserved coach message append succeeds, completed upsert fails once, next request recovers from existing message without a second provider call
- failure upsert after provider error fails once, next request returns the preserved mapped failure instead of re-calling provider unexpectedly

**Step 2: Run test to verify it fails**

Run: `cargo test --test workout_summary_service generate_coach_reply -- --nocapture`
Expected: FAIL in the new regression tests because the current implementation still has post-provider write gaps.

**Step 3: Capture the intended contract in the code comments if needed**

Document the rule inside the service or test names:

- if a provider response already exists durably, recovery must replay/finalize it before any new provider call
- if a coach message already exists with the reserved id, recovery must complete from that message instead of appending again

**Step 4: Run test again after test cleanup only**

Run: `cargo test --test workout_summary_service generate_coach_reply -- --nocapture`
Expected: Still FAIL, but now with precise failure descriptions tied to the intended recovery contract.

**Step 5: Commit**

```bash
git add tests/workout_summary_service/messaging.rs
git commit -m "test: cover coach reply recovery gaps"
```

### Task 2: Add deterministic failure injection to the in-memory test fakes

**Files:**
- Modify: `tests/workout_summary_service/shared.rs`
- Test: `tests/workout_summary_service/messaging.rs`

**Step 1: Write the failing test helper behavior**

Extend the in-memory repositories so tests can fail exactly one call for:

- `CoachReplyOperationRepository::upsert`
- `WorkoutSummaryRepository::append_message`

The helper API should be minimal, for example “fail next append” or “fail next upsert”.

**Step 2: Run test to verify helper-driven failures are exercised**

Run: `cargo test --test workout_summary_service generate_coach_reply -- --nocapture`
Expected: FAIL in the targeted tests because the fake now reproduces the real write-failure windows.

**Step 3: Write minimal helper implementation**

Use simple one-shot failure toggles in the in-memory fakes. Avoid generalized fault injection frameworks.

**Step 4: Run test to verify helper setup is correct**

Run: `cargo test --test workout_summary_service generate_coach_reply -- --nocapture`
Expected: Tests still fail, but now through the intended repository failure path rather than invalid test setup.

**Step 5: Commit**

```bash
git add tests/workout_summary_service/shared.rs tests/workout_summary_service/messaging.rs
git commit -m "test: add workout summary write failure fakes"
```

### Task 3: Make provider-response checkpoint recovery explicit and idempotent

**Files:**
- Modify: `src/domain/workout_summary/model.rs`
- Modify: `src/domain/workout_summary/service.rs`
- Test: `tests/workout_summary_service/messaging.rs`

**Step 1: Write the minimal domain helpers needed for recovery**

Add the smallest helpers needed to express recovery clearly, for example:

- helper to detect whether an operation has a durable provider-response checkpoint
- helper to detect whether an operation can be finalized from an existing message

Prefer helper methods on `CoachReplyOperation` over spreading nullable-field checks across the service.

**Step 2: Run targeted test to verify current implementation still fails**

Run: `cargo test --test workout_summary_service generate_coach_reply_replays_persisted_response_message_after_partial_crash -- --nocapture`
Expected: FAIL before service changes.

**Step 3: Write minimal implementation in `generate_coach_reply()`**

Normalize the recovery order before any provider call:

1. completed operation returns the existing coach reply
2. pending operation with existing reserved coach message finalizes from that message
3. pending operation with durable provider-response checkpoint appends the reserved coach message and finalizes
4. failed operation returns the mapped existing failure
5. only then call the provider

**Step 4: Run targeted test to verify it passes**

Run: `cargo test --test workout_summary_service generate_coach_reply_replays_persisted_response_message_after_partial_crash -- --nocapture`
Expected: PASS

**Step 5: Commit**

```bash
git add src/domain/workout_summary/model.rs src/domain/workout_summary/service.rs tests/workout_summary_service/messaging.rs
git commit -m "feat: replay persisted coach replies after partial crashes"
```

### Task 4: Preserve exactly-once provider behavior across recovery retries

**Files:**
- Modify: `src/domain/workout_summary/service.rs`
- Modify: `tests/workout_summary_service/messaging.rs`
- Possibly modify: `tests/workout_summary_service/shared.rs`

**Step 1: Write the failing duplicate-call assertions**

Use a counting coach fake to assert that recovery retries do not trigger another provider call when:

- the provider checkpoint already exists
- the final coach message already exists
- the existing failure record should be surfaced directly

**Step 2: Run test to verify it fails**

Run: `cargo test --test workout_summary_service generate_coach_reply_recovers -- --nocapture`
Expected: FAIL because at least one recovery path still performs an extra provider call or returns the wrong error.

**Step 3: Write minimal implementation changes**

Keep the current reserved message id workflow, but make every retry path short-circuit from the durable operation state before provider invocation.

**Step 4: Run test to verify it passes**

Run: `cargo test --test workout_summary_service generate_coach_reply_recovers -- --nocapture`
Expected: PASS with stable message ids and no duplicate provider calls.

**Step 5: Commit**

```bash
git add src/domain/workout_summary/service.rs tests/workout_summary_service/messaging.rs tests/workout_summary_service/shared.rs
git commit -m "fix: avoid duplicate llm calls during coach reply recovery"
```

### Task 5: Verify Mongo replacement semantics preserve recovery data

**Files:**
- Modify: `src/adapters/mongo/coach_reply_operations.rs`
- Test: `tests/workout_summary_service/messaging.rs`

**Step 1: Write a failing persistence-shape regression if needed**

If service tests reveal a document transition drops checkpoint data, add a regression that proves these fields survive until completion:

- reserved `coach_message_id`
- provider metadata
- checkpointed `response_message`
- usage and cache fields needed for replay/finalization

**Step 2: Run test to verify it fails**

Run: `cargo test --test workout_summary_service generate_coach_reply_recovers_existing_message_without_losing_provider_metadata -- --nocapture`
Expected: FAIL only if a transition actually drops persisted recovery metadata.

**Step 3: Write minimal adapter fix**

Preserve the necessary recovery fields across `Pending -> Pending` and `Pending -> Completed` replacements. Do not redesign the document shape unless the tests prove it is necessary.

**Step 4: Run test to verify it passes**

Run: `cargo test --test workout_summary_service generate_coach_reply_recovers_existing_message_without_losing_provider_metadata -- --nocapture`
Expected: PASS

**Step 5: Commit**

```bash
git add src/adapters/mongo/coach_reply_operations.rs tests/workout_summary_service/messaging.rs
git commit -m "fix: preserve coach reply recovery metadata"
```

### Task 6: Run focused verification, then the backend gate

**Files:**
- No new files

**Step 1: Run focused recovery tests**

Run: `cargo test --test workout_summary_service -- --nocapture`
Expected: PASS

**Step 2: Run formatting**

Run: `cargo fmt --all --check`
Expected: PASS

**Step 3: Run linting**

Run: `cargo clippy --all-targets --all-features -- -D warnings`
Expected: PASS

**Step 4: Run the broader backend suite**

Run: `cargo test`
Expected: PASS

**Step 5: Commit**

```bash
git add src/domain/workout_summary/model.rs src/domain/workout_summary/service.rs src/adapters/mongo/coach_reply_operations.rs tests/workout_summary_service/shared.rs tests/workout_summary_service/messaging.rs
git commit -m "harden coach reply finalization after partial persistence failures"
```

---

## Scope Guardrails

Keep this follow-up narrowly focused on durable coach-reply finalization and replay after post-provider write failures.

Do not expand it into:

- provider cache invalidation
- frontend settings UX
- general LLM error taxonomy cleanup
- startup background repair passes

unless a failing recovery test proves one of those is directly required.

## Suggested Commit Theme

`harden coach reply finalization after partial persistence failures`
