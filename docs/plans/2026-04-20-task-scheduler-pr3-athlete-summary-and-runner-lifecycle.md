# Task Scheduler PR3: Athlete Summary Migration On Shared Worker

**Goal:** Migrate caller-facing `athlete_summary` generation onto the current task-scheduler architecture without changing the durable generation semantics already owned by the direct athlete-summary service.

**Scope:**
- verify the current shared task-worker runtime assumptions before building on them
- add `athlete_summary.generate` as a scheduler-backed workflow on the existing shared worker
- expose the scheduler-backed wrapper to app callers while keeping the direct service as the executor body

**Non-goals:**
- do not re-introduce a feature-specific runner lifecycle helper; the shared worker already exists
- do not re-route `WorkoutSummaryService` internals through the scheduler-backed athlete-summary wrapper in this PR
- do not migrate training-plan generation yet
- do not change athlete-summary freshness rules, REST DTO shape, or direct durable operation semantics

## Current Branch Context

- The old PR2-specific runner lifecycle gap is already closed on this branch.
- Task execution now runs through the shared runtime in `src/config/task_scheduler/worker.rs`, which already persists:
  - worker heartbeat
  - enabled task types
  - `active_task_ids`
  - cleanup after panic / shutdown
- Scheduler-backed request waiting now uses `enqueue_result_task(...)` and `wait_for_result_task(...)` in `src/domain/task_scheduler/service/result.rs`, not a fixed 30-second polling timeout.
- Current runner lifecycle behavior is already covered by `src/domain/workout_summary/service/scheduler/tests/runner_lifecycle.rs`.

## Architecture Rules

- Keep the direct `AthleteSummaryService` as the executor body. It already owns:
  - pending operation claiming
  - stale/failed reclaim
  - persist-before-provider-call ordering
  - durable completion/failure writes
- Use the task scheduler only as the outer orchestration layer:
  - enqueue
  - shared worker dispatch
  - task retry / timeout handling
  - wait for terminal result
- Keep caller-facing `athlete_summary` requests on the scheduler-backed wrapper.
- Keep nested `workout_summary` coach-reply calls on the direct athlete-summary service for now. A background task waiting synchronously on another task in the same saturated worker pool is a separate design problem and not part of PR3.

## Task 1: Verify The Shared Worker Baseline

**Files:**
- Reference: `src/config/task_scheduler/worker.rs`
- Reference: `src/domain/task_scheduler/service/result.rs`
- Reference: `src/domain/workout_summary/service/scheduler/tests/runner_lifecycle.rs`

**Work:**
- Reconfirm that the current shared worker already provides the lifecycle guarantees the old PR3 draft planned to add.
- Only change the shared worker runtime if implementation of `athlete_summary.generate` exposes a real missing capability.

**Done when:**
- PR3 builds on the existing shared worker instead of re-solving solved PR2 follow-ups

## Task 2: Add `athlete_summary.generate` Scheduler Module

**Files:**
- Modify: `src/domain/athlete_summary/service.rs`
- Create: `src/domain/athlete_summary/service/scheduler.rs`
- Modify: `src/domain/athlete_summary/mod.rs`

**Task type:**
- `athlete_summary.generate`

**Payload shape:**
- `user_id`
- `force`

**Work:**
- Add a scheduler-backed wrapper around the direct `AthleteSummaryService`.
- Add a task handler that calls the direct service as the executor body.
- Persist structured terminal task errors so the wrapper can reconstruct the current `AthleteSummaryError` categories instead of collapsing everything into strings.
- Reconstruct successful caller-visible results by reloading the final summary from the direct service after task completion.
- Do not add a second durability layer for athlete-summary generation.

**Dedupe rules:**
- Non-force refreshes should dedupe within the same freshness window, not forever.
- The dedupe key for `force = false` should include the current refresh window anchor so a completed task from last week does not block a fresh regeneration this week.
- `force = true` should use a unique dedupe key per request so repeated forced regenerations remain possible even while old completed tasks still exist.

**Done when:**
- concurrent non-force callers for the same user and refresh window converge on one task
- repeated `force = true` calls still regenerate instead of reusing an old completed task forever
- failed tasks preserve enough structured state to rebuild the original `AthleteSummaryError`

## Task 3: Preserve Caller-Facing Semantics

**Files:**
- Modify: `src/domain/athlete_summary/service/scheduler.rs`
- Add targeted tests near the scheduler module

**Work:**
- Preserve current behavior of:
  - `generate_summary(user_id, force)`
  - `ensure_fresh_summary(user_id)`
  - `ensure_fresh_summary_state(user_id)`
- Keep the fresh-summary fast path honest: if a summary is already fresh and `force = false`, return it directly without enqueuing background work.
- When background work is required, wait on the scheduler result path instead of introducing a new business timeout.
- Map retryability honestly:
  - retryable `LlmError` stays retryable
  - repository failures stay retryable
  - `NotConfigured` stays non-retryable
  - inner durable-operation `already pending` conflicts should retry on the reclaim window, not on a fake short delay

**Done when:**
- scheduler-backed behavior matches the old direct-service behavior for fresh, forced, retryable, and non-retryable paths
- `ensure_fresh_summary_state` still reports `was_regenerated` honestly

## Task 4: Main Wiring On The Existing Shared Worker

**Files:**
- Modify: `src/main.rs`

**Work:**
- Keep separate direct and caller-facing athlete-summary services:
  - direct service for task execution internals and nested workout-summary use
  - scheduler-backed wrapper for REST and other top-level callers
- Register both task handlers on the existing shared task worker:
  - `workout_summary.coach_reply`
  - `athlete_summary.generate`
- Keep the wiring explicit enough that it is obvious which services are executor bodies and which are wrappers.

**Done when:**
- app startup uses one shared worker for both task types
- `AppState` gets the scheduler-backed athlete-summary service
- `WorkoutSummaryService` still depends on the direct athlete-summary service in this PR

## Task 5: Tests

**Minimum coverage:**
- scheduler-backed athlete-summary success
- fresh non-force request bypasses scheduler work
- scheduler-backed retryable LLM failure preserves structured error
- scheduler-backed non-retryable failure preserves structured error category
- `ensure_fresh_summary_state` still reports `was_regenerated` accurately
- non-force dedupe stays scoped to the current refresh window
- completed forced task does not block a later forced regeneration

## Final Verification

Run at minimum:

```bash
cargo test athlete_summary -- --nocapture
cargo test task_scheduler -- --nocapture
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
./scripts/rebuild_graphify.sh
```

## Exit Criteria

- `athlete_summary.generate` runs through the shared task worker
- the direct athlete-summary service still owns durable local state and provider ordering
- caller-facing athlete-summary behavior remains compatible
- PR3 does not re-open already solved shared-worker lifecycle work
- PR3 does not introduce nested scheduler waits inside background task handlers
