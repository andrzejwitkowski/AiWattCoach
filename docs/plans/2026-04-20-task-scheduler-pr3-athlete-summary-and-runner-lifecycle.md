# Task Scheduler PR3: Athlete Summary And Runner Lifecycle

**Goal:** Close the remaining correctness gaps left by PR2 and migrate `athlete_summary` generation onto the task scheduler without changing current caller-visible behavior.

**Scope:**
- fix the runner lifecycle gap from PR2 so dedicated task runners participate in worker heartbeat / active-task recovery
- keep current request-response semantics for scheduler-backed flows honest
- migrate only athlete-summary generation in this PR

**Non-goals:**
- do not migrate training-plan generation yet
- do not introduce a generic framework for every future task type
- do not change athlete-summary freshness rules, DTO shape, or REST contract

## Context From PR2 Review

- PR2 proved the outer-orchestration pattern for `workout_summary.coach_reply`, but the first runner wiring still has a recovery blind spot: the dedicated `...-workout-summary` runner claims tasks, yet no worker heartbeat is persisted for that exact worker id.
- PR2 also introduced a request-side polling timeout that is too short to be trusted as a business timeout for LLM-backed work.
- PR3 should fix those two issues before repeating the pattern for the next durable LLM flow.

## Architecture Rules

- Keep the existing direct `AthleteSummaryService` as the executor body. It already owns the real durable operation semantics via `AthleteSummaryGenerationOperationRepository`.
- Use the task scheduler only as the outer orchestration layer: enqueue, claim, heartbeat, complete/fail, and wait for terminal result.
- Preserve the current semantics of:
  - `generate_summary(user_id, force)`
  - `ensure_fresh_summary(user_id)`
  - `ensure_fresh_summary_state(user_id)`
- Persist local operation state before side effects exactly as the direct service already does today.

## Task 1: Fix dedicated runner lifecycle before another migration

**Files:**
- Modify: `src/config/task_scheduler.rs`
- Modify: `src/main.rs`
- Modify: `src/domain/workout_summary/service/scheduler.rs`
- Add tests around worker registration / restart recovery if current coverage is still indirect

**Work:**
- Introduce a small dedicated runner helper that:
  - persists worker heartbeat for the exact worker id used by the runner
  - reports enabled task types honestly
  - tracks `active_task_ids` while a task is being processed
- Wire the workout-summary runner through that helper instead of spawning a claim loop that is invisible to the worker registry.
- Keep the existing maintenance loop for timeout sweep, but stop relying on it as the only worker heartbeat path for dedicated runners.

**Done when:**
- a restart of the scheduler-backed workout-summary runner can be recovered by worker-state logic, not only by waiting for stale task heartbeat age-out
- the stable worker-id work from PR1/PR2 is actually used by the dedicated runner path

## Task 2: Make scheduler-backed request semantics honest

**Files:**
- Modify: `src/domain/workout_summary/service/scheduler.rs`
- Add targeted tests in the same module or in `tests/workout_summary_service/**`

**Work:**
- Revisit the PR2 wait strategy so request-response flows do not hide retryable LLM/provider failures behind an arbitrary polling timeout.
- Preserve the old direct-service behavior for user-visible failures:
  - retryable LLM failures still surface as structured `WorkoutSummaryError::Llm(...)`
  - slow-but-valid work should not be turned into a fake terminal timeout unless there is a real business decision to do that
- If a synchronous caller still waits for task completion, make the wait budget align with the real provider timeout envelope instead of an arbitrary 30-second constant.

**Done when:**
- scheduler-backed request behavior matches the old direct path for retryable and non-retryable failures
- there is explicit test coverage for a retryable scheduler-backed failure path

## Task 3: Add `athlete_summary.generate` task orchestration

**Files:**
- Create: `src/domain/athlete_summary/service/scheduler.rs`
- Modify: `src/domain/athlete_summary/service.rs`
- Modify: `src/domain/athlete_summary/mod.rs`
- Modify: `src/main.rs`

**Task type:**
- `athlete_summary.generate`

**Payload shape:**
- `user_id`
- `force`

**Work:**
- Add a scheduler-backed wrapper service around the direct `AthleteSummaryService`.
- Keep the direct service unchanged as the executor body used by the runner.
- Terminal result should let the wrapper reconstruct the exact current return values without guessing:
  - for `generate_summary` and `ensure_fresh_summary`, reload the final summary from the direct repository after completion
  - for `ensure_fresh_summary_state`, preserve whether a regeneration actually happened
- Failed tasks should persist structured `AthleteSummaryError` information, not only a string message.

**Done when:**
- athlete-summary generation can run through the task scheduler while preserving existing freshness and recovery semantics

## Task 4: Wire athlete-summary callers through the scheduler-backed wrapper

**Files:**
- Modify: `src/main.rs`
- Search and verify all existing `AthleteSummaryUseCases` consumers

**Work:**
- Keep a direct athlete-summary service for the runner executor.
- Expose the scheduler-backed wrapper to application callers.
- Ensure workout-summary chat still calls athlete-summary through the same `AthleteSummaryUseCases` boundary and does not learn any scheduler-specific details.

**Done when:**
- app wiring uses the wrapper for caller-facing use cases and the direct service only for task execution internals

## Task 5: Tests

**Files:**
- `tests/workout_summary_service/**` if workout-summary integration changes are visible
- athlete-summary tests near the feature or in `tests/**` depending on existing coverage layout

**Minimum coverage:**
- scheduler-backed athlete-summary success
- scheduler-backed athlete-summary non-retryable failure preserves structured error
- scheduler-backed athlete-summary retryable failure preserves structured error
- `ensure_fresh_summary_state` still reports `was_regenerated` honestly
- restart recovery for dedicated runner worker registration

## Final Verification

Run at minimum:

```bash
cargo test --test task_scheduler -- --nocapture
cargo test athlete_summary -- --nocapture
cargo test workout_summary -- --nocapture
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
./scripts/rebuild_graphify.sh
```

## Exit Criteria

- dedicated task runners heartbeat worker state with active task ids
- PR2 review gaps are closed before the next migration repeats them
- athlete-summary generation is scheduler-backed without changing current semantics
- no domain module learns Mongo, Axum, or provider SDK details it did not already own
