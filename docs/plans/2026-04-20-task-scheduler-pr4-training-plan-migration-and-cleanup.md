# Task Scheduler PR4: Training Plan Migration And Shared Worker Follow-up

**Goal:** Migrate saved-workout training-plan generation onto the existing shared task worker and clean up scheduler integration only where PR3 and PR4 prove the same patterns are genuinely repeated.

**Scope:**
- move the saved-workout training-plan flow to scheduler-backed execution
- preserve current `mark_saved` workflow semantics and response shape
- clean up shared scheduler wiring only where duplication is now proven across workout-summary, athlete-summary, and training-plan flows

**Non-goals:**
- do not migrate unrelated external-sync or race flows in this PR
- do not redesign training-plan domain logic, snapshot persistence, or projection persistence rules
- do not casually route nested background tasks through the same shared worker without an explicit concurrency design

## Context

- After PR3, the shared scheduler runtime should already exist in one place and serve at least:
  - `workout_summary.coach_reply`
  - `athlete_summary.generate`
- PR3 intentionally keeps `WorkoutSummaryService` on the direct athlete-summary dependency internally. That avoids a background task synchronously waiting on another task from the same worker pool.
- PR4 should start from that boundary instead of undoing it accidentally while migrating training-plan generation.
- The direct `TrainingPlanGenerationService` already owns the real durable workflow state via `TrainingPlanGenerationOperationRepository`, snapshots, and projections.

## Architecture Rules

- Keep the direct `TrainingPlanGenerationService` as the executor implementation.
- Keep scheduler concerns outside the core training-plan domain orchestration wherever possible.
- Keep LLM-backed scheduler execution timeouts aligned with the shared `domain::llm` request-timeout policy. Do not reintroduce per-feature timeout literals that drift away from the actual provider timeout envelope.
- Preserve the current `WorkoutSummaryService::mark_saved()` workflow behavior:
  - recap generation remains durable
  - training-plan generation remains durable
  - response DTO shape and status messaging stay compatible unless a change is explicitly justified
- If a background task would need to wait on another scheduler task handled by the same saturated pool, either:
  - keep the inner dependency direct, or
  - introduce a deliberately separate worker topology
  - but do not smuggle in nested waits as an incidental refactor

## Task 1: Define Scheduler Boundaries For Training-Plan Generation

**Files:**
- Modify: `src/domain/training_plan/service/mod.rs`
- Create: `src/domain/training_plan/service/scheduler.rs`
- Modify: `src/domain/training_plan/mod.rs`

**Task type:**
- `training_plan.generate_for_saved_workout`

**Payload shape:**
- `user_id`
- `workout_id`
- `saved_at_epoch_seconds`

**Work:**
- Use one scheduler task for the full saved-workout generation flow unless the code proves that a split is actually required.
- Let the direct service continue owning:
  - operation claiming
  - recap generation
  - snapshot persistence
  - projection persistence
  - calendar refresh
- The scheduler wrapper should only enqueue, wait, and reconstruct the final `GeneratedTrainingPlan` result from durable local state.

**Done when:**
- a saved-workout training-plan request can run through the scheduler without duplicating the direct service’s durable workflow logic

## Task 2: Preserve Caller-Visible Save Workflow Behavior

**Files:**
- Modify: `src/domain/workout_summary/service/mod.rs`
- Modify: `src/domain/workout_summary/service/use_cases.rs`
- Modify tests under `tests/workout_summary_service/**`
- Modify REST tests under `tests/workout_summary_rest/**` if externally visible behavior changes

**Work:**
- Wire `WorkoutSummaryService::mark_saved()` to the scheduler-backed training-plan use case while keeping current workflow messaging and error semantics as stable as possible.
- Preserve the current partial-success behavior where local saved state remains durable even if downstream generation pieces fail.
- Do not move save-workflow business rules into HTTP handlers.

**Done when:**
- `POST /api/workout-summaries/:id/state` remains semantically compatible while the heavy generation path runs through the scheduler

## Task 3: Consolidate Shared Helpers Only Where Duplication Is Real

**Files:**
- `src/config/task_scheduler/worker.rs`
- `src/domain/workout_summary/service/scheduler.rs`
- `src/domain/athlete_summary/service/scheduler.rs`
- `src/domain/training_plan/service/scheduler.rs`

**Work:**
- Extract only the pieces that are now clearly the same concern across multiple task types, for example:
  - terminal result parsing patterns
  - structured task error serialization helpers if they truly repeat
  - small task-construction helpers where the abstraction stays obvious
- Do not re-abstract the already-generic shared worker runtime just because more task types now exist.
- Do not force one generic nested-orchestration helper if the workflows still differ materially.

**Done when:**
- obvious duplication is removed
- business-specific executor logic still lives with its feature

## Task 4: Main Wiring Cleanup

**Files:**
- Modify: `src/main.rs`

**Work:**
- Keep separate direct services for task execution and scheduler-backed services for caller-facing use cases.
- Register shared-worker handlers in one obvious place.
- Make worker topology intentional. If PR4 needs more than one worker pool, make that explicit in code and config instead of burying it in helper calls.

**Done when:**
- app startup makes it obvious which services are direct executors, which are wrappers, and which task types each worker handles

## Task 5: Tests

**Minimum coverage:**
- scheduler-backed training-plan success for saved workout
- scheduler-backed training-plan failure preserves current `TrainingPlanError` category
- saved-workout REST flow keeps current response shape and workflow messages
- restart recovery for a running training-plan task
- no duplicate local projections after explicit retry of a failed task

## Final Verification

Run at minimum:

```bash
cargo test --test task_scheduler -- --nocapture
cargo test training_plan -- --nocapture
cargo test workout_summary -- --nocapture
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
./scripts/rebuild_graphify.sh
```

## Exit Criteria

- training-plan generation for saved workouts is scheduler-backed
- save-workflow behavior remains durable and practically compatible
- shared scheduler wiring is cleaner after PR3 and PR4, not more ad hoc
- no new coupling from domain code into Axum, Mongo documents, or provider DTOs
- worker topology remains explicit enough that nested scheduler waits cannot slip in by accident
