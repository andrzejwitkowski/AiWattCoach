# Reviewers Log

This file records fixes made in response to review feedback so similar PR and review mistakes are less likely to repeat.

Read this file before planning and before implementation.

## How To Use

- Scan the newest entries first.
- Focus on entries that match the current task area, failure mode, or review pattern.
- When you apply a fix based on feedback from the user, Copilot, or CodeRabbit, add a new entry immediately after the fix.

## Entry Format

- Date: `YYYY-MM-DD`
- Source: user | Copilot | CodeRabbit
- Scope: file, feature, or review area
- Problem: what was wrong or missing
- Fix: what changed to address it
- Prevention: what to check next time before sending work for review

## Entries

### 2026-04-21 | user | integration vs unit test boundary cleanup

- Problem: several REST integration suites were asserting domain behavior and adapter helper rules that already belonged below the HTTP layer, which duplicated coverage and made the endpoint suites larger than necessary.
- Fix: moved settings masking/interval normalization checks into adapter unit tests, moved completed-workout id fallback checks into `CompletedWorkoutReadService` unit tests, strengthened existing calendar domain tests for sync/update behavior, and removed the redundant REST cases while keeping auth/status/limit/happy-path coverage at the integration layer.
- Prevention: before adding or keeping a REST integration test, ask whether it proves transport-boundary behavior or just repeats service/helper logic; if it is not boundary-specific, cover it with a lower-level fake-backed test and leave only one straightforward happy path per endpoint.

### 2026-04-20 | user | test-suite memory leaks and unstable Rust harnesses

- Problem: several Rust test binaries leaked memory and process resources by retaining frontend fixtures in globals, recreating expensive Mongo clients repeatedly, and spawning Axum test servers with unmanaged `tokio::spawn` tasks that lived until process exit. This made `cargo test -- --nocapture` fail with shifting suite-level `SIGKILL`s.
- Fix: replaced unbounded retained fixtures with bounded shared fixtures, reused `mongodb::Client` per test binary where safe, disabled the unnecessary bin test harness in `Cargo.toml`, and updated the affected test helpers to own spawned server tasks and abort them in `Drop`.
- Prevention: when writing tests, treat memory and task lifetime as part of the helper contract: no unbounded globals, no unmanaged spawned servers, prefer bounded per-binary singletons for expensive immutable resources, and diagnose suite-level flakiness only with sequential heavy test runs.

### 2026-04-20 | user | scheduler worker loop ownership and generic boundaries

- Problem: the dedicated `workout_summary` task runner embedded the whole claim, idle wait, task heartbeat, completion, and failure persistence loop inside feature code, which made the critical scheduler flow hard to read and tied generic worker behavior to one LLM-specific use case.
- Fix: extracted the shared worker loop into `src/domain/task_scheduler/runner.rs` with a small generic `TaskRunnerHandler` contract and `TaskRunOutcome`, then reduced the workout summary runner to payload parsing and coach-reply-specific success/error mapping only.
- Prevention: when adding another scheduled workflow, first ask whether the logic is generic worker orchestration or feature-specific task handling; keep claim/lease/heartbeat/complete/fail mechanics in `task_scheduler`, and let feature runners provide only payload parsing plus domain outcome mapping.

### 2026-04-20 | user | scheduler result waiting must stay generic

- Problem: `SchedulerBackedWorkoutSummaryService` still had its own task-status polling loop for waiting on completed/failed/timed-out results, so the scheduler orchestration was split between `task_scheduler` and feature code.
- Fix: added generic `ResultTaskHandler`, `enqueue_result_task(...)`, `wait_for_result_task(...)`, and `enqueue_no_result_task(...)` to `src/domain/task_scheduler/service.rs`, then rewired the workout summary wrapper to provide only checkpoint/error parsing and final result hydration.
- Prevention: for background workflows that return a caller-visible result, keep enqueue/retry/poll/result orchestration inside `task_scheduler`; feature wrappers may build the task and map terminal scheduler state into domain output, but must not own custom polling loops.

### 2026-04-20 | user | single scheduler worker loop and smaller service methods

- Problem: the scheduler still had a per-feature worker spawn shape and `TaskSchedulerService` accumulated large orchestration methods that were hard to review; the result path also still looked like a custom loop instead of a generic scheduler-owned mechanism.
- Fix: replaced the per-feature worker flow with one global worker loop in `src/domain/task_scheduler/runner.rs` that dispatches by registered `task_type` handlers, changed result waiting to event-driven task updates via in-memory watchers instead of polling, and split scheduler service logic into smaller request-building and state-transition helpers.
- Prevention: keep exactly one worker claim/dispatch loop in the scheduler layer, let handlers only implement task-type-specific execution/result mapping, and split any scheduler/service method as soon as it spans multiple orchestration phases.

### 2026-04-20 | user | scheduler workers need real concurrency and shared active-task state

- Problem: the first global worker loop still awaited a claimed task inline, which effectively serialized task handling, and the initial concurrency refactor briefly used non-shared active-task state that could drop `active_task_ids` updates under parallel work.
- Fix: introduced bounded worker concurrency via a semaphore-backed task pool in `src/domain/task_scheduler/runner.rs`, kept task execution in spawned task slots, and centralized per-worker active-task tracking so claim/heartbeat/release all update the same shared runtime state.
- Prevention: when adding concurrency to worker loops, verify both throughput semantics and shared state semantics together; if multiple tasks can run in parallel, any worker-level heartbeat or active-task snapshot must come from one shared source of truth, not per-task local state.

### 2026-04-20 | user | workout summary chat use case method too large

- Problem: `generate_coach_reply_impl` in `src/domain/workout_summary/service/use_cases/chat.rs` had grown into a near-file-sized orchestration method that mixed validation, operation claiming, LLM call execution, checkpoint persistence, message append, and final result hydration in one block.
- Fix: split the method into small helpers for loading the persisted user message, claiming/recovering the reply operation, requesting and checkpointing the LLM response, appending the coach message, finalizing the completed operation, and building the final `CoachReply`.
- Prevention: when a use-case method starts spanning the whole file, stop and split it by phase immediately; orchestration methods should read top-to-bottom as a short pipeline, with detailed persistence and recovery logic pushed into named helpers.

### 2026-04-20 | user | workout summary scheduler-backed coach reply PR2 review fixes

- Problem: the dedicated `workout_summary.coach_reply` runner did not publish its own worker heartbeat or `active_task_ids`, the scheduler-backed wait path introduced a new 30-second caller-visible timeout that the direct path never had, and the wrapper dropped `athlete_summary_was_regenerated` from the `generate_coach_reply()` contract.
- Fix: made the dedicated runner persist its own worker state while idle and while holding a task, removed the wrapper-only 30-second timeout so synchronous callers wait for terminal task state, and stored a structured completed-task checkpoint that preserves both the persisted coach message and the regeneration flag while remaining backward-compatible with older message-only checkpoints.
- Prevention: when wrapping an existing synchronous flow with the scheduler, verify that the wrapper does not shorten the old success contract, that any worker used for recovery semantics publishes the same lifecycle data the sweeper relies on, and that task checkpoints preserve every field the original return type promised to callers.

### 2026-04-20 | user | task scheduler restart recovery semantics

- Problem: the first scheduler core version left `running` tasks stuck after process restart unless someone manually retried a later `timed_out` task, which was too weak for instance restarts and docker-style redeploys.
- Fix: added explicit task recovery in the scheduler sweep so `running` tasks move back to `retry_scheduled` when their owner worker disappears or restarts without reporting the task as active; `timed_out` now stays for the narrower truly abandoned case.
- Prevention: when designing worker leases, test restart behavior separately from timeout behavior and verify that a dead or restarted owner leads to automatic reclaim when the state is unambiguous.

### 2026-04-20 | user | task scheduler worker identity and timeout defaults

- Problem: the first PR left `worker_id` lifecycle implicit and used an aggressively low default worker-staleness window that could misclassify long-running LLM tasks as abandoned.
- Fix: added `default_task_scheduler_worker_id()` so workers prefer a stable `TASK_SCHEDULER_WORKER_ID` or `HOSTNAME` identity before falling back to a per-process UUID, and raised the default `worker_stale_after_seconds` in `src/config/task_scheduler.rs` to a safer 30-minute window.
- Prevention: when introducing distributed worker coordination, define the worker-id source explicitly and make container-friendly stable identities the default when restart recovery is required; also sanity-check timeout defaults against the slowest expected external operation before sending for review.

### 2026-04-19 | Copilot | admin metrics backfill test coverage

- Problem: the non-admin metrics backfill REST test omitted same-origin headers, so it could return `403` at the CSRF/same-origin guard before reaching `require_admin`.
- Fix: added `Host` and `Origin` headers to the non-admin metrics backfill test so it now exercises the authorization branch intentionally.
- Prevention: when testing authorization behind request-shape guards, satisfy the earlier transport checks first so the test reaches the branch it claims to cover.

### 2026-04-19 | CodeRabbit | metrics backfill selection and observability

- Problem: metrics backfill imported activities whenever any metric existed upstream, even if none of the currently missing fields would be filled, and fetch/import failures were counted without diagnostic context.
- Fix: tightened the metrics backfill gate to require at least one missing field to be provided by the fetched activity, added a regression test for that case, and logged fetch/import failures with structured `warn!` fields.
- Prevention: for partial backfills, compare upstream data against the specific missing local fields before counting an item as enriched, and log batch-processing failures with enough identifiers to debug retries.

### 2026-04-19 | user | backfill refactor readability

- Problem: test doubles used tuple-shaped call records that obscured field meaning, `backfill_missing_metrics` stayed too monolithic, and backfill tests were still too large to navigate comfortably.
- Fix: replaced tuple call records with named structs, split metrics backfill orchestration into explicit helper phases, and divided backfill tests into `details`, `metrics`, and shared `support` modules.
- Prevention: when a test helper or orchestration path starts relying on positional values or exceeds a few logical phases, refactor immediately into named data structures and concern-based files before adding more behavior.

### 2026-04-19 | user | completed workout metrics backfill

- Problem: the new metrics backfill used the stale completed-workout date to choose `recomputed_from`, which could miss earlier snapshots if the Intervals activity import corrected the activity date.
- Fix: changed the backfill flow to derive `recomputed_from` from `detailed_activity.start_date_local` after fetching the refreshed Intervals payload.
- Prevention: for any batch import followed by recompute, confirm that the recompute boundary comes from the final imported source-of-truth record, not the pre-import local copy.

### 2026-04-19 | user | agent process docs

- Problem: the repo instructions did not include a durable review-fix loop, so repeated PR and review mistakes were not being logged in a reusable place.
- Fix: created `reviewers.md`, added the review-fix loop to `AGENTS.md`, and added the reusable lesson to `tasks/lessons.md`.
- Prevention: before writing a plan or implementing changes, read `reviewers.md` and check whether the current task repeats a known review pattern.
