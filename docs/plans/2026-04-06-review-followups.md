# Review Followups Implementation Plan

**Goal:** Close the remaining final-review findings around projection replay healing, workout-summary legacy identifier ambiguity, reclaimed correction retry budget, and small follow-up test/doc nits.

**Architecture:** Keep the existing standalone-Mongo eventual-consistency approach, but make replay and identifier matching deterministic. Fix the remaining bugs by tightening repository semantics and replay checkpoints rather than introducing transactions or broad refactors.

**Tech Stack:** Rust, MongoDB, cargo test, cargo fmt, cargo clippy

---

### Task 1: Make projection replay heal partial same-operation inserts

**Files:**
- Modify: `src/adapters/mongo/training_plan_projections.rs`
- Modify: `tests/training_plan_mongo.rs`
- Test: `tests/training_plan_service/main.rs`

**Step 1: Write the failing test**

Add a Mongo regression test that simulates a same-`operation_key` replay after a partial projection insert, and assert replay converges instead of failing with duplicate keys.

**Step 2: Run test to verify it fails**

Run: `cargo test --test training_plan_mongo <new_test_name> -- --nocapture`

Expected: FAIL because `replace_window()` uses blind `insert_many()` and cannot heal partial same-operation inserts.

**Step 3: Write minimal implementation**

Change the projection persistence path so replay for the same `operation_key` becomes idempotent. Prefer deterministic upsert/replace behavior over transactions.

**Step 4: Run test to verify it passes**

Run: `cargo test --test training_plan_mongo <new_test_name> -- --nocapture`

Expected: PASS.

### Task 2: Make workout-summary legacy fallback deterministic

**Files:**
- Modify: `src/adapters/mongo/workout_summary.rs`
- Modify: `tests/workout_summary_mongo.rs`
- Modify: `tests/workout_summary_service/create_and_get.rs` if needed for coverage

**Step 1: Write the failing test**

Add a regression test where both of these exist for the same user:
- a current document with `workout_id = X`
- a legacy document with `event_id = X`

Assert repository reads and writes prefer the current `workout_id` document deterministically.

**Step 2: Run test to verify it fails**

Run: `cargo test --test workout_summary_mongo <new_test_name> -- --nocapture`

Expected: FAIL because the current `$or` filter is ambiguous.

**Step 3: Write minimal implementation**

Replace the ambiguous `$or` read/write behavior with deterministic precedence for `workout_id` first, then legacy `event_id` only when the primary match is absent.

**Step 4: Run test to verify it passes**

Run: `cargo test --test workout_summary_mongo <new_test_name> -- --nocapture`

Expected: PASS.

### Task 3: Preserve correction retry budget across reclaim

**Files:**
- Modify: `src/domain/training_plan/service.rs`
- Modify: `tests/training_plan_service/main.rs`

**Step 1: Write the failing test**

Add a regression test for a reclaimed pending operation that already has one stored correction response. Assert recovery still allows the full remaining retry budget rather than silently skipping one retry.

**Step 2: Run test to verify it fails**

Run: `cargo test --test training_plan_service <new_test_name> -- --nocapture`

Expected: FAIL because reclaim currently skips `attempt_index == 0` and reduces retries by one.

**Step 3: Write minimal implementation**

Update reclaim correction-loop logic so resumed workflows keep the intended remaining correction budget.

**Step 4: Run test to verify it passes**

Run: `cargo test --test training_plan_service <new_test_name> -- --nocapture`

Expected: PASS.

### Task 4: Close low-level test and docs nits

**Files:**
- Modify: `src/main_tests.rs`
- Modify: `tests/workout_summary_rest/shared/workout_summary.rs`
- Modify: `docs/plans/2026-04-06-backend-slice-size-refactor.md`
- Optional split if still warranted: `src/domain/training_context/packing/payloads.rs`

**Step 1: Tighten log assertions**

Update the log tests so they assert related fragments on the same captured log entry instead of separate global `contains(...)` checks.

**Step 2: Fix fake service user propagation**

Make `TestWorkoutSummaryService::create_summary()` respect its `user_id` input instead of hard-coding `user-1`.

**Step 3: Remove remaining reviewer-workflow text from committed docs**

Keep the plan file focused on project work, not reviewer-process instructions.

**Step 4: Re-check `payloads.rs` size**

If the file still exceeds the repo threshold and can be split minimally without churn, split it. If not practical in the same pass, note it explicitly in follow-up review output.

### Task 5: Final verification

**Files:**
- Modify only if verification exposes a real issue

**Step 1: Run targeted tests**

Run:
- `cargo test --test training_plan_service -- --nocapture`
- `cargo test --test training_plan_mongo -- --nocapture`
- `cargo test --test workout_summary_mongo -- --nocapture`
- `cargo test training_context --lib -- --nocapture`

**Step 2: Run final repo checks for this batch**

Run:
- `cargo fmt --all --check`
- `cargo clippy --all-targets --all-features -- -D warnings`

Expected: PASS.
