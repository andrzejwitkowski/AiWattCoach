# Backend Slice Size Refactor Design

**Goal:** Reduce the current training-plan and workout-summary backend slice so oversized files in that slice are split into smaller concern-based modules without changing behavior.

## Scope

- Refactor only oversized files in the current backend slice.
- Keep behavior, public APIs, durable workflow ordering, and existing test semantics unchanged.
- Preserve hexagonal boundaries and repo naming conventions.

## Chosen Approach

Use directory-based Rust modules and focused helper files instead of introducing new abstraction layers.

- Split `src/domain/training_context/service.rs` into a `service/` directory with focused siblings for builder flow, context assembly helpers, power compression helpers, date/load helpers, and tests.
- Split `src/domain/workout_summary/service.rs` into a `service/` directory with a small root module plus focused files for use-case methods and tests.
- Split `src/domain/training_context/packing.rs` into a `packing/` directory with focused payload mapping/rendering files and separate tests.
- Split tests out of `src/adapters/mongo/workout_summary.rs` first, and only split the adapter into smaller helper files if it still remains oversized.
- Split bulky current-slice test helpers and integration suites such as `tests/workout_summary_service/shared.rs` and `tests/llm_adapters.rs` into directory-based suites with focused files.

## Why This Approach

- Smallest structural change that gets file sizes down without changing runtime behavior.
- Matches the repo's existing guidance to split growing backend areas into directory modules with focused siblings.
- Keeps orchestration in domain services and persistence in adapters instead of hiding logic behind generic utilities.
- Makes later reviewer passes more actionable because responsibility boundaries become visible.

## Module Boundaries

### `training_context`

- Keep public exports in `src/domain/training_context/mod.rs` unchanged.
- Move service internals into concern-based siblings such as context assembly, power compression, and date/load helpers.
- Move inline tests and test doubles into dedicated test modules so production files stay smaller and easier to review.

### `workout_summary`

- Keep `WorkoutSummaryService` as the main entrypoint.
- Move the trait implementation body and message/recovery workflow helpers into sibling files under `service/`.
- Keep persist-before-side-effects ordering unchanged for save and coach-reply flows.

### Mongo adapter

- Keep `MongoWorkoutSummaryRepository` behavior unchanged.
- Separate repository methods from document mapping/filter helpers and adapter tests.

### Tests

- Convert large integration or shared-helper files into directory modules with focused files by concern.
- Keep test names and assertions stable where possible.

## Verification

- Re-run focused Rust tests after each file-family split.
- Run broader backend verification after the full refactor.
- Then run the requested reviewer passes on the resulting diff.
