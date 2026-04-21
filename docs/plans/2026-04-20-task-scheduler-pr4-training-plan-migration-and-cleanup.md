# Task Scheduler PR4: Training Plan Migration And Cleanup

**Goal:** Migrate saved-workout training-plan generation onto the task scheduler and clean up the shared scheduler wiring after PR2 and PR3.

**Scope:**
- move the saved-workout training-plan flow to scheduler-backed execution
- preserve current `mark_saved` workflow semantics and response shape
- consolidate duplicated runner / terminal-result logic only where duplication is now proven

**Non-goals:**
- do not migrate unrelated external-sync or race flows in this PR
- do not redesign training-plan domain logic or projection persistence rules
- do not change the calendar refresh contract unless the current behavior is already wrong

## Context

- After PR3, the scheduler pattern should already be proven on two durable LLM workflows:
  - `workout_summary.coach_reply`
  - `athlete_summary.generate`
- The next natural migration target in this repo is training-plan generation for saved workouts because it already has durable local operation state in `TrainingPlanGenerationOperationRepository`.
- PR4 should use that existing durable training-plan logic as the direct executor body instead of inventing a second persistence layer.

## Architecture Rules

- Keep the direct `TrainingPlanGenerationService` as the executor implementation.
- Keep scheduler concerns outside the core training-plan domain orchestration wherever possible.
- Preserve the current `WorkoutSummaryService::mark_saved()` workflow behavior:
  - recap generation remains durable
  - training-plan generation remains durable
  - response DTO shape and status messaging stay compatible unless a change is explicitly justified

## Task 1: Define scheduler boundaries for training-plan generation

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
- Use one scheduler task for the full saved-workout generation flow rather than splitting recap and plan into separate public tasks unless the code proves that split is necessary.
- Let the direct service continue owning:
  - operation claiming
  - recap generation
  - snapshot persistence
  - projection persistence
  - calendar refresh
- The scheduler wrapper should only enqueue, wait, and reconstruct the final `GeneratedTrainingPlan` result from durable local state.

**Done when:**
- a saved-workout training-plan request can be run through the task scheduler without changing the direct service’s local durability logic

## Task 2: Preserve caller-visible save workflow behavior

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

## Task 3: Consolidate shared scheduler helpers only where duplication is now real

**Files:**
- `src/config/task_scheduler.rs`
- `src/domain/workout_summary/service/scheduler.rs`
- `src/domain/athlete_summary/service/scheduler.rs`
- `src/domain/training_plan/service/scheduler.rs`

**Work:**
- Extract only the now-proven shared pieces, for example:
  - dedicated runner worker lifecycle helper
  - terminal wait helper patterns
  - structured error/checkpoint serialization helpers where the shape is actually repeated
- Do not force a generic abstraction if the three workflows still differ materially.

**Done when:**
- duplication that is obviously the same concern is removed
- business-specific executor logic still lives close to each feature

## Task 4: Main wiring cleanup

**Files:**
- Modify: `src/main.rs`

**Work:**
- Keep separate direct services for runner execution and scheduler-backed services for caller-facing use cases.
- Register dedicated worker ids and enabled task types in one obvious place.
- Avoid ad hoc per-feature runner spawning patterns if PR3 already introduced a clearer helper.

**Done when:**
- app startup makes it obvious which services are direct executors and which are caller-facing wrappers

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
- shared scheduler wiring is cleaner than after PR2/PR3, not more ad hoc
- no new coupling from domain code into Axum, Mongo documents, or provider DTOs
