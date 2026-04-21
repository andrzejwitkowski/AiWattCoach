# Lessons

## Review Fix Logging Loop

- When I implement a fix based on feedback from the user, Copilot, or CodeRabbit, I must record it in `reviewers.md`.
- Each `reviewers.md` entry must state both the problem that was identified and the fix that was applied.
- The purpose of this loop is to reduce repeated PR and review mistakes over time.
- I must read `reviewers.md` before writing a plan and before starting implementation work.

## Backfill Recompute Ranges

- When a backfill or reimport operation can change canonical record dates, I must derive recompute ranges from the refreshed upstream payload, not from the stale local record.
- Before finalizing batch recompute logic, verify that the chosen `oldest_changed` date still covers records whose timestamps may be corrected during import.

## Test Doubles And Shapes

- In tests, avoid tuple aliases for multi-field call records when the field meaning matters. Use named structs or named sub-structs so assertions stay self-explanatory.
- When a function grows past a few distinct phases, split it into small helpers named after each phase instead of leaving one long orchestration block.
- When a test file grows large, split it by behavior group and extract shared fakes/fixtures into a local `support` module.
- When a service method starts mixing validation, request building, persistence, orchestration, and result interpretation, split it immediately into small helpers. Long public methods in scheduler/service code are hard to review and hide control-flow bugs.
- When converting a single-task loop into a concurrent worker pool, keep worker-level state in one shared runtime structure. Per-task copies of active-task state lead to lost heartbeats and flaky concurrency behavior.
- When a use-case orchestration method starts owning validation, claim/recovery, provider I/O, checkpoint writes, persistence completion, and result hydration all at once, split it into named phase helpers before adding more behavior.
- Treat function size as a hard clean-code rule: aim to stay at or below about 100 lines of code, and if a function grows past roughly 130 lines, refactor it into smaller logical helpers before continuing. Do not keep adding behavior to oversized functions.
- When runtime orchestration for a domain workflow needs `tokio` tasks, timers, channels, or shutdown handles, keep the task handler contract in `src/domain` but move the runtime loop and background-task wiring into `src/config` or another adapter/wiring layer. Do not leave runtime-specific loops in domain modules just because the workflow is domain-owned.
- When a scheduled task wraps another durable operation with its own stale or reclaim timeout, align the scheduler retry delay with that durable reclaim window. Otherwise the wrapper can burn through retries and mark a task dead before the underlying operation is actually recoverable.

## Distributed Worker Defaults

- When adding worker registries or claim/lease coordination, decide and document the `worker_id` source explicitly. Prefer stable env- or hostname-based ids when restart recovery should treat a restarted instance as the same logical worker.
- Before finalizing timeout defaults, compare them against the slowest realistic external operation, especially LLM calls and other long-polling network work.
- Model restart recovery separately from `timed_out`: if the old owner is gone or restarted and no longer reports the task as active, prefer automatic recovery to a reclaimable queue state instead of forcing manual retry.

## Test Stability Diagnosis

- When diagnosing suite-level `SIGKILL` or memory-pressure failures, never launch multiple heavy `cargo test` targets in parallel. Those runs create artificial contention and make the results non-diagnostic.
- For flaky test-harness failures, prefer sequential reruns of the exact binary order from the failing suite, then inspect test helpers for leaked servers, background tasks, or retained global fixtures before changing production code.
- In parallel Rust test binaries, do not assert that a shared global capture registry is completely empty unless the helper truly owns every concurrent capture. Assert that the current test's capture was deregistered instead.
- Do not reuse `mongodb::Client` or similar async driver clients across separate `#[tokio::test]` runtimes via `OnceLock` or other process-global singletons. A client tied to a runtime that has already shut down can fail later with cancelled-task or runtime-shutdown errors.

## Projection Window Semantics

- If a persisted snapshot exposes `start_date`, `end_date`, and a concrete `days` list, verify whether `start_date` is inclusive before writing bridge readers or tests. Do not silently encode `date > start_date` unless the model explicitly defines `start_date` as an anchor outside the visible plan.
- When a calendar/read-model row is missing, compare the durable source collection with the first canonical reader that reconstructs domain objects from it before changing refresh or cleanup logic. A missing read-model row can be caused upstream by an over-filtering root adapter, not by the projector itself.

## Integration Test Scope

- Keep REST integration tests for transport-boundary behavior only: auth, user scoping, HTTP status mapping, request parsing/validation, body-size limits, and one simple happy path per endpoint.
- If a REST test is mostly checking domain decisions, DTO masking, normalization, merge logic, or repository fallback behavior, move that coverage into the relevant unit test module with fakes and delete the duplicate integration case.

## Test Memory Hygiene

- Test helpers must own every spawned background task. If a test starts `tokio::spawn(axum::serve(...))` or similar long-lived async work, keep the `JoinHandle` and abort or shut it down in `Drop`.
- Global test state must stay bounded. Never keep app fixtures, temp directories, or other per-test resources in an ever-growing `Vec` behind `OnceLock`, `Mutex`, or similar globals.
- If a test resource is expensive but safe to share, prefer a single per-binary singleton such as `OnceLock<Client>` or `OnceLock<FrontendFixture>` instead of recreating one instance per test.
- Sharing a client is not the same as sharing mutable data: keep per-test database names and mutable test records isolated even when the underlying client is reused.
- When a suite starts many HTTP mock servers or websocket apps, centralize that startup in a helper with cleanup semantics instead of open-coded `tokio::spawn` blocks in each test.
- When a suite gets `SIGKILL` only after many earlier test binaries pass, suspect retained test infrastructure first and inspect the binaries that run immediately before the failure point.
