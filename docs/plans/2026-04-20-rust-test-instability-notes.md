# Rust Test Instability Notes

## Current Problem

- `cargo test` is flaky in this branch.
- Failures are not stable assertion failures. The process sometimes ends with `SIGKILL`.
- The kill point moves between runs, which makes this look more like suite-level instability than one deterministic broken test.

## Observed Failures

- `cargo test --lib` sometimes ends with `SIGKILL`.
- Full-suite runs have ended with `SIGKILL` in different targets across runs, including:
  - `intervals_adapters`
  - `intervals_rest`
  - `intervals_service`
  - `llm_adapters`
  - `external_sync_mongo`
- Isolated runs of these targets passed when run on their own.

## What This Probably Is Not

- The current workout-summary scheduler tests are not sharing Mongo `tasks` state.
- The scheduler tests under `src/domain/workout_summary/service/scheduler/tests/` use:
  - `InMemoryTaskRepository`
  - `InMemoryTaskWorkerRepository`
- Those tests were also marked `#[serial]` in this branch to avoid overlapping background worker activity inside the same test process.

## Mongo Test Isolation Findings

- There are currently 44 tests that definitely use real Mongo state with their own unique database and cleanup.
- These include:
  - `training_load_mongo`
  - `training_plan_mongo`
  - `canonical_roots_mongo`
  - `calendar_entry_views_mongo`
  - `races_mongo`
  - `external_sync_mongo`
  - `intervals_pest_parser_poc`
  - `workout_summary_mongo`
  - `main_runtime`
- There is also a second group of REST/integration helpers that build `AppState` with a real `mongodb::Client` but use `Settings::test_defaults()`, which points to the shared database name `aiwattcoach` unless overridden.
- That means shared Mongo state is still a credible hypothesis for some integration suites, but it does not explain all observed `SIGKILL`s because several killed targets were outside the confirmed Mongo-heavy group.

## Verification Wiring Changes In This Branch

- Main-runtime helper code was moved out of `src/main.rs` into `src/main_runtime.rs`.
- Tests for that area were moved from the bin target into `tests/main_runtime.rs`.
- Scheduler tests were serialized with `serial_test`.
- `package.json` was changed to route Rust tests through `scripts/verify_rust_tests.sh`.
- That script is only an experiment. It is not a proven fix because `cargo test --lib` can still be killed.

## Memory Pressure Hypothesis

- The failure pattern is compatible with memory pressure or another suite-level resource problem:
  - isolated targets pass
  - full runs fail late
  - the killed target changes between runs
  - the failure is `SIGKILL`, not a normal test assertion
- During diagnosis, system memory pressure looked high on the machine:
  - physical memory: 36 GB
  - memory used: about 31.96 GB
  - compressed: about 15.30 GB
  - swap used: about 4.10 GB
- That does not prove OOM, but it materially strengthens the hypothesis that local machine memory pressure may be contributing.

## Most Likely Remaining Explanations

1. Local machine memory pressure during full-suite runs.
2. Shared Mongo state in some REST/integration suites that still use the default `aiwattcoach` database.
3. Test-process lifetime retention in shared helpers, such as tracing capture, temp frontend fixtures, or other long-lived resources.
4. A large or flaky `cargo test --lib` harness that still needs further splitting or cleanup.

## After Reboot

1. Re-run `cargo test --lib` first.
2. Re-run `cargo test` after reboot with Activity Monitor open on memory pressure and swap.
3. If flakiness remains, audit suites that use `Settings::test_defaults()` with a real Mongo client and decide whether to force unique databases or explicit cleanup.
4. If needed, revert or replace the experimental `verify_rust_tests.sh` approach with a fix based on confirmed root cause.

## Important Status Note

- This branch contains useful diagnosis and code movement, but the Rust test instability is not fully resolved yet.
- Do not treat this branch as fully verified just because it is pushed.
