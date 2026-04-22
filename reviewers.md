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

### 2026-04-22 | CodeRabbit | PR #115 unresolved review follow-up

- Problem: unresolved PR #115 review threads still pointed at three real gaps and several hygiene issues: the maintenance loop accepted zero-second ticker intervals that would panic inside Tokio, terminal task mapping silently dropped `cleanup_after` when a terminal task lacked `finished_at_epoch_seconds`, recovery completion paths in workout-summary bypassed the existing post-provider retry helper, worker-state cache mutations could diverge from the persisted worker row on concurrent active-task updates or failed upserts, test verification still forced `cargo test -- --nocapture`, several Mongo test fixtures still printed connection-string context in failure messages, and the scheduler `TestCoach::failing(...)` fake kept failing forever instead of modeling a transient provider error.
- Fix: added upfront validation for maintenance heartbeat/sweep intervals and tests for both zero-valued cases; made terminal-task cleanup mapping reject terminal tasks without `finished_at_epoch_seconds`; routed both coach-reply recovery completion paths through `persist_post_provider_operation(...)` and added focused recovery regressions; serialized worker-state cache mutations across worker upserts with rollback on failure plus targeted worker-state regressions; changed `scripts/verify_rust_tests.sh` to plain `cargo test`, removed raw Mongo URI text from the touched fixture error messages, tightened the task TTL index assertion to check `expire_after = 0`, and changed the scheduler `TestCoach` fake to consume its injected failure once while updating the retry regression to rely on that one-shot behavior.
- Prevention: if code constructs Tokio intervals or tickers, validate the interval config before spawning; if a mapper computes TTL cleanup metadata for terminal records, missing terminal timestamps should be treated as invariant violations, not silently downgraded to `None`; any post-provider recovery write must go through the dedicated retry wrapper instead of ad hoc repository upserts; if in-memory worker state mirrors durable worker projections, keep mutation and persistence serialized per worker and restore the previous cache state on persist failure; test logs and scripts must not expose connection strings by default, and failure fakes should be one-shot unless a test explicitly needs persistent failure behavior.

### 2026-04-22 | Copilot/Qodo | PR #126 athlete summary scheduler review follow-up

- Problem: new `src/domain/athlete_summary/...` scheduler tests called `crate::config::spawn_task_worker(...)`, which pulled composition-root wiring into domain tests, and the shared `workout_summary.coach_reply` task timeout was narrowed to a single LLM request even though one task attempt can also regenerate athlete summary before requesting the coach reply.
- Fix: replaced the `athlete_summary` test dependency on `crate::config` with a local test worker helper built from domain scheduler primitives only, and raised `COACH_REPLY_EXECUTION_TIMEOUT_SECONDS` to cover two LLM requests plus a small buffer for context-building and checkpoint writes.
- Prevention: keep `src/domain/**` tests on domain-owned scheduler primitives or local test helpers instead of importing startup/config wiring, and for scheduler-owned LLM tasks size execution timeouts to the full end-to-end attempt path rather than the inner provider HTTP timeout alone.

### 2026-04-22 | user | LLM-backed scheduler timeout alignment

- Problem: the new `athlete_summary.generate` scheduler task kept a hard-coded execution timeout that was not explicitly aligned with the real LLM request timeout policy, and the branch still encoded model-name-based adapter timeouts separately from scheduler task timeouts.
- Fix: introduced a shared `domain::llm` timeout constant/helper with a uniform 3-minute request timeout, switched the LLM adapter to use that shared timeout for all models, and aligned scheduler execution timeouts to that same baseline instead of separate adapter-specific literals.
- Prevention: for any LLM-backed scheduler task, compare task execution timeout against the actual provider request timeout source before shipping, then add explicit buffer for non-HTTP work or nested LLM calls when the end-to-end task path needs more than one request window.

### 2026-04-22 | user | scheduler panic regression test follow-up

- Problem: the new panic-path regression test assumed the worker heartbeat row already existed and used `expect(...)` on an eventually updated worker projection, so it could fail before the worker finished startup even though the runtime behavior was correct.
- Fix: changed the first phase of the test to wait on the owned handler-start signal plus primary task state (`Running` with `claimed_by = worker-1`) instead of the worker repository, and changed the cleanup wait to poll the worker projection with `is_some_and(...)` until `active_task_ids` becomes empty.
- Prevention: in async worker tests, synchronize early assertions on direct control signals or primary task state, not on lagging heartbeat/projection writes; when polling eventually updated projections, avoid `expect(...)` until the phase that guarantees the row exists.

### 2026-04-22 | Copilot | PR #115 task worker and workout summary review follow-up

- Problem: the shared task worker still accepted zero-valued timing config that could panic at runtime when creating Tokio intervals, a panicking task handler could outlive the parent cleanup path and leave heartbeats/worker activity detached, coach-reply failure logs omitted `user_id`, and the save workflow treated any historical coach message as a finished conversation even if the latest message was still from the user.
- Fix: validated `lease_duration_seconds`, `heartbeat_interval`, and `idle_poll_interval` before spawning the worker; wrapped worker child tasks so handler and heartbeat tasks are aborted on drop while still clearing worker activity after panics; added `user_id` to the coach-reply failure `warn!`; and changed the save workflow conversation-finished check to require the last message to be from the coach, with focused regressions for each path.
- Prevention: any runtime loop that constructs `tokio::interval` or similar timers must fail fast on non-positive config before spawning, any spawned child task created only for orchestration should be owned by an abort-on-drop guard so panics and shutdown cannot detach it, failure logs at workflow boundaries should include user/workout identifiers needed for recovery, and conversation-complete predicates must inspect the terminal message state instead of searching the whole history for any matching role.

### 2026-04-22 | Copilot | training plan conversation context review follow-up

- Problem: the first review version treated missing workout summaries as `Ok(None)` when loading planning context, cloned the full planning context unnecessarily during retries, and embedded raw conversation text into `stable_context`, which is transported as system-role context for some LLM providers.
- Fix: changed `get_planning_context` to propagate `WorkoutSummaryError::NotFound` through the existing training-plan error mapping, switched training-plan prompt assembly to send prior coach/user planning history as role-correct `conversation` messages while keeping only `planning_rpe` in `stable_context`, and changed planning-context caching in the generation service to load once without cloning on each correction call.
- Prevention: when threading user-originated history into LLM requests, check every provider mapping before putting that data into `stable_context` or any system-role field; prefer role-correct conversation messages for conversational history, and do not silently downgrade missing prerequisite state into `None` unless the flow is explicitly optional.

### 2026-04-22 | user | verification strategy for heavy Rust test binaries

- Problem: I launched multiple heavy `cargo test --test ...` binaries in parallel while verifying the training-plan conversation-context change. In this repo that produced non-diagnostic `SIGKILL` failures under host pressure, which obscured whether the code itself was actually broken.
- Fix: switched verification to sequential, narrowly filtered Rust tests for the touched behavior, kept `cargo fmt --all --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `bun run verify:arch` as the reliable completion gates, and stopped treating the parallel-suite `SIGKILL`s as actionable product failures.
- Prevention: never parallelize heavy Rust test binaries in this repo. If broad suites hit host-level `SIGKILL`s, verify the changed behavior with sequential targeted filters and explicitly report the broader suites as skipped for environment reasons instead of chasing a fake code regression.

### 2026-04-21 | user | training plan projected-day roots and calendar view refresh

- Problem: after saving a new workout, a fresh training-plan projection was persisted for the new window, but the calendar could still miss the first projected day of that new snapshot. Real Mongo data showed `training_plan_projected_days` already contained an active row for `2026-04-22` while `calendar_entry_views` did not. The root cause was not refresh-range cleanup; the bridge readers for projected plans were still filtering with `date > snapshot.start_date`, so the first day of every snapshot was silently dropped before calendar refresh and other projected-day readers ever saw it.
- Fix: changed the projected-day root readers to treat `snapshot.start_date` as inclusive in both Mongo adapters and the in-memory training-plan test repository, updated the Mongo regression tests that had encoded the old exclusion behavior, and added an integration regression proving `CalendarEntryViewRefreshService` now writes a planned calendar entry for a projected workout that lands on `snapshot.start_date`.
- Prevention: when a projection snapshot stores dated `days`, treat `start_date` as the first real day in that window unless there is an explicit separate anchor-day model. Before blaming downstream refresh logic, compare the durable source rows and the first read adapter that reconstructs canonical roots from them.

### 2026-04-21 | user | workout summary scheduler pending-retry recovery

- Problem: scheduler-backed `workout_summary.coach_reply` retries still used the generic 30-second task delay even when the direct coach-reply workflow returned `ReplyAlreadyPending`, but the underlying `CoachReplyOperation` only became reclaimable after the 300-second stale window. That mismatch could exhaust task retries and leave a failed task even though the durable reply operation was still recoverable later.
- Fix: added a scheduler failure-path retry-delay override so `ReplyAlreadyPending` reschedules the task at the coach-reply stale-operation window instead of the generic fixed delay, kept the direct workflow semantics unchanged, and added a scheduler-backed regression test that proves the delayed retry succeeds once the reclaim window opens.
- Prevention: whenever a scheduled wrapper sits on top of another durable operation with its own reclaim or stale timeout, compare the scheduler retry cadence against that recovery window explicitly; do not let wrapper retries exhaust before the wrapped durable state can be reclaimed.

### 2026-04-21 | user | task scheduler review follow-up on PR #120

- Problem: the shared task worker runtime still lived under `src/domain/task_scheduler`, which kept `tokio` runtime orchestration in the domain layer, and both `src/domain/task_scheduler/service.rs` and the scheduler worker file had grown too large to review comfortably. The task storage also lacked a cleanup policy for terminal task records.
- Fix: moved the runtime worker loop into `src/config/task_scheduler/worker.rs`, kept only task handler/config types in `src/domain/task_scheduler`, split the scheduler service into concern-based modules under `src/domain/task_scheduler/service/`, split config scheduler wiring into `maintenance.rs` and `worker.rs`, and added a Mongo TTL cleanup field/index for completed/failed/timed-out tasks.
- Prevention: when adding scheduler/runtime behavior, keep `tokio` spawning, timers, shutdown handles, and logging orchestration outside `src/domain`; if a scheduler or config file starts exceeding a few phases or a few hundred lines, split it immediately by concern instead of waiting for review to call it out.

### 2026-04-21 | user | Rust test harness instability follow-up

- Problem: after the earlier memory-hygiene cleanup, the full `cargo test -- --nocapture` run was still failing for harness reasons that looked like flaky suite instability: tracing-capture helpers assumed the global active buffer map must be empty even while parallel tests were still running, and multiple Mongo-backed test helpers reused a `OnceLock<mongodb::Client>` across separate `#[tokio::test]` runtimes, which led to cancelled driver tasks and runtime-shutdown errors.
- Fix: changed the tracing-capture assertions to verify that the current capture was cleaned up instead of asserting global emptiness, removed the outer `tokio::time::timeout(...)` cancellation pattern from Mongo test availability checks in favor of short driver `server_selection_timeout` settings, and stopped reusing shared Mongo clients across separate test runtimes in the affected helpers.
- Prevention: when stabilizing Rust test harness code, validate per-test isolation instead of global-emptiness assumptions, and do not share async driver clients across independent `#[tokio::test]` runtimes unless the resource lifetime is guaranteed to outlive every runtime that uses it.

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
