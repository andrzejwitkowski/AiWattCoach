# Lessons

## Review Fix Logging Loop

- When I implement a fix based on feedback from the user, Copilot, or CodeRabbit, I must record it in `reviewers.md`.
- Each `reviewers.md` entry must state both the problem that was identified and the fix that was applied.
- The purpose of this loop is to reduce repeated PR and review mistakes over time.
- I must read `reviewers.md` before writing a plan and before starting implementation work.

## PR Conflict Verification

- When resolving PR conflicts, fetch the current base branch ref and test the merge against that exact `origin/<base>` immediately before calling the PR conflict-free. A branch can be clean and synced with its remote head branch while still conflicting if the base branch advanced.
- When resolving conflicts in rolling logs like `reviewers.md` or `tasks/lessons.md`, preserve entries from both branches and restore newest-first ordering instead of picking one side and dropping history.

## Signature Change Verification

- When changing a function signature, grep every call site including local `#[cfg(test)]` modules in the same file before calling the refactor done. `cargo clippy --all-targets` compiles test targets too, so a missed unit-test call site will still fail CI even if the runtime code builds.
- For constructors with many positional arguments of the same type, re-check test fixtures against the canonical signature before treating behavior regressions as product bugs. A misordered fixture can silently move a link/id into the wrong field and produce misleading CI failures.

## Fixture Refactor Verification

- When converting test helpers to a shared fixture strategy, grep for removed helper names as well as the new shared helper. Adjacent test builders often retain one stale call site that only shows up once `--all-targets` compiles that test binary.

## Backfill Recompute Ranges

- When a backfill or reimport operation can change canonical record dates, I must derive recompute ranges from the refreshed upstream payload, not from the stale local record.
- Before finalizing batch recompute logic, verify that the chosen `oldest_changed` date still covers records whose timestamps may be corrected during import.

## Test Doubles And Shapes

- When a sync workflow adds a discovery or recovery step before the previous happy path, revisit the touched test doubles immediately. A fake that used to be sufficient can silently stop modeling the branch the test name claims to exercise.
- In tests, avoid tuple aliases for multi-field call records when the field meaning matters. Use named structs or named sub-structs so assertions stay self-explanatory.
- When a function grows past a few distinct phases, split it into small helpers named after each phase instead of leaving one long orchestration block.
- When a test file grows large, split it by behavior group and extract shared fakes/fixtures into a local `support` module.
- When a service method starts mixing validation, request building, persistence, orchestration, and result interpretation, split it immediately into small helpers. Long public methods in scheduler/service code are hard to review and hide control-flow bugs.
- When converting a single-task loop into a concurrent worker pool, keep worker-level state in one shared runtime structure. Per-task copies of active-task state lead to lost heartbeats and flaky concurrency behavior.
- When a use-case orchestration method starts owning validation, claim/recovery, provider I/O, checkpoint writes, persistence completion, and result hydration all at once, split it into named phase helpers before adding more behavior.
- Treat function size as a hard clean-code rule: aim to stay at or below about 100 lines of code, and if a function grows past roughly 130 lines, refactor it into smaller logical helpers before continuing. Do not keep adding behavior to oversized functions.
- When runtime orchestration for a domain workflow needs `tokio` tasks, timers, channels, or shutdown handles, keep the task handler contract in `src/domain` but move the runtime loop and background-task wiring into `src/config` or another adapter/wiring layer. Do not leave runtime-specific loops in domain modules just because the workflow is domain-owned.
- When a scheduled task wraps another durable operation with its own stale or reclaim timeout, align the scheduler retry delay with that durable reclaim window. Otherwise the wrapper can burn through retries and mark a task dead before the underlying operation is actually recoverable.
- For scheduler-backed wrappers over durable `claim_pending` operations, automatic retries after worker panics or other scheduler-level failures must wait at least until the stale pending window opens. Retrying sooner can bounce straight back into `already in progress` and turn a recoverable scheduler failure into a terminal task.
- Do not let `src/domain/**` tests depend on `crate::config` or other composition-root wiring just to start background workers. If domain tests need worker execution, add a domain-owned test helper or exercise the scheduler via domain primitives only.
- For LLM-backed scheduled tasks, size `execution_timeout_seconds` to the whole attempt path, not just the inner HTTP request timeout. Include any preceding nested LLM calls, context building, and post-provider checkpoint writes when choosing the scheduler timeout budget.
- If a worker heartbeat persists the full active-task snapshot while other code mutates active-task ids incrementally, hold the same cache lock through persistence and roll back on failure. Otherwise a stale heartbeat can erase active tasks from the worker projection.
- In recovery-path tests, assert idempotent repository writes, not just the returned result, so duplicate side effects cannot hide behind the same final response.
- When a scheduled-task success path must serialize a persisted checkpoint, do not swallow serialization failures with `.ok()`. Convert them into explicit task failure so result handlers surface the real cause.

## Small Review Fixes

- For detail modals, keep data fetching in a dedicated hook or container-level component and keep the rendered detail panel presentational. Do not thread transport/config props like `apiBaseUrl` into a child just to power one late-added fetch.
- When adding a new section to an existing detail view, verify whether the requirement is additive or substitutive before deleting the previous section. If an existing section still carries distinct user value, preserve it and add the new section alongside it.
- For completed-workout recap loading, derive the lookup id from the most authoritative completed-workout source available (`actualWorkout.activityId` before a best-effort loaded activity object) so partial activity-load failures do not suppress recap data.
- Do not surface raw `HttpError` transport strings like `GET /api/... failed: 503` directly in UI state. Map expected HTTP failures to user-facing localized copy unless the raw message is explicitly intended for the user.
- When adding a REST sub-route beside an existing resource route, match the surrounding path parameter name (`activity_id` vs `workout_id`) unless there is a real semantic distinction. Mismatched names create avoidable confusion in handlers, tests, and debugging.

- In a dual-read/dual-write timestamp migration, every field that is required at the domain level must be verified against a `DateTime`-only persisted document, not just legacy-plus-new mixed documents. If the persistence struct still uses a non-optional legacy epoch field, the migration is incomplete even when normal writes succeed.
- When promising a staged migration with a later backfill step, complete the backfill before calling the rollout done. Readability improvements are not actually delivered for existing Mongo rows until the historical documents are updated too.
- For dense UI mini-charts, keep the canonical sequence intact and apply any width compression only in the rendering layer. Sampling or equal-width fallbacks are likely to destroy either temporal order or duration meaning.
- When frontend UX explicitly wants to use stylized planned-workout zone heights, do not silently revert to raw `%FTP` heights to make old tests pass. Update the tests to the intended visual contract instead.

- For upstream file-upload APIs, do not assume a nested JSON body is equivalent to documented `resource[file]` params. Check whether the provider expects `application/x-www-form-urlencoded` or multipart transport and whether the file field must be wrapped as `data:<mime>;base64,...` instead of raw base64.
- When migrating Mongo documents from one timestamp representation to dual epoch-plus-DateTime storage, remove `expect(...)` from read mappers for any collection that can contain legacy or manually corrupted rows. Missing required timestamps should surface as repository/storage errors, not panic the request or worker.
- If a write lock or optimistic update filter previously keyed off one persisted timestamp field, update it to consider every persisted representation of that state. A new DateTime mirror field can otherwise reopen writes that should remain locked.
- For BSON `DateTime` to epoch-second conversion, use euclidean division on milliseconds so negative timestamps floor correctly instead of truncating toward zero.
- If a Mongo collection is known to be brand new with no legacy rows, do not weaken required document fields to `Option<T>` just to match a broader migration pattern. Keep serde-level guarantees where backward compatibility is not actually needed.
- When simplifying poll-cursor helpers, verify both branches explicitly: existing cursor plus new upstream results should usually advance, while existing cursor plus no new results may need to stay put. Do not drop the data-presence signal unless a regression test proves the simplified semantics are still correct.
- If a polling loop filters fetched upstream records before import, do not tie cursor advancement only to the filtered subset unless repeated rereads of skipped records are intentional. Cursor/watermark movement usually belongs to the full consumed upstream page.
- If a paginated provider bootstrap can span many pages, do not defer all durable progress until the entire scan succeeds. A later-page `429` or transient error can otherwise reset the bootstrap to zero forever. Persist a resumable checkpoint such as `next_page` plus the latest seen watermark before returning the failure.
- In response/body mappers, decode a byte payload to UTF-8 once and reuse the borrowed text across classification and logging helpers instead of repeating `from_utf8(...)` work.
- If parsed JSON string data is only compared against a static literal, keep it borrowed as `&str` and compare in place instead of allocating an owned `String` first.
- When a review-driven change upgrades a match-strength enum or ranking rule, grep the touched test module for old enum expectations before shipping. Production code and review replies can be correct while one stale assertion still expects the pre-change ranking.
- For external client write logging, prefer adapter-local body preview logging only on the specific POST/PUT paths that need it, and redact secret-bearing form keys before they hit logs. Do not broaden body logging for unrelated requests just to debug one provider write flow.
- For provider DTO scalars or collections, `#[serde(default)]` only handles missing fields, not explicit `null`. If upstream can send `null`, deserialize through `Option<T>` or a custom helper and add a regression with the real payload shape.

## Release Workflow Reliability

- When a GitHub Actions workflow grows bespoke version/tagging logic, extract it into a repository script with unit tests instead of leaving the logic inline in YAML.
- If a git tag is intended to mean "artifact is available", publish the artifact first and push the tag only after the publish step succeeds.
- Any workflow that uses `Swatinem/rust-cache` or Docker Buildx `cache-to/from: type=gha` must keep `actions` permission enabled explicitly when permissions are restricted.
- Scripts that shell out to `docker build` or similar filesystem-sensitive commands must anchor their working paths to the repo/script location instead of assuming the caller launched them from the repo root.

## Review-Driven Config Fixes

- When review feedback asks to make adapter constants environment-configurable, route the values through the centralized startup settings parser with explicit defaults instead of reading `std::env` inside the adapter.
- After adding new env-backed settings, update the env key loader, sample env file, and focused settings tests in the same change so the new configuration path is actually exercised.

## Canonical Parser Pairs

- When one parser reads canonical text produced by another serializer in the same repo, verify structural constructs end to end, not just token-level fields.
- For repeated workout blocks, add regression tests that assert expanded segment order and total duration so grouped semantics cannot silently collapse back into flat line parsing.
- If a grouped construct reuses child items parsed by the flat path, preserve child multiplicity too. Outer repeat support is still wrong if inline child repeats are emitted only once per parent iteration.

## OpenCode Plugin Wiring

- When a repo-local OpenCode plugin is configured in `opencode.json`, verify that the plugin file exports the module in the shape expected by the installed plugin API, not just a conveniently named helper like `GraphifyPlugin`.
- If a repo-level reminder is meant to influence normal code exploration, inject it into session/system context instead of relying only on `bash` command rewriting, because many exploration flows start with `glob`, `grep`, or `read`.
- If the plugin uses ESM syntax, make the tracked repo state explicitly ESM via a committed `.mjs` path or committed package metadata. Do not rely on ignored local files to make the plugin load cleanly.

## OAuth Callback Alignment

- For OAuth flows with separate `start` and `callback` endpoints, verify that the configured provider callback URL matches the actual backend router path exactly. Keep the callback route, example env, dev client shortcut, and focused auth/settings tests aligned in the same change.

## Distributed Worker Defaults

- When adding worker registries or claim/lease coordination, decide and document the `worker_id` source explicitly. Prefer stable env- or hostname-based ids when restart recovery should treat a restarted instance as the same logical worker.
- Before finalizing timeout defaults, compare them against the slowest realistic external operation, especially LLM calls and other long-polling network work.
- Model restart recovery separately from `timed_out`: if the old owner is gone or restarted and no longer reports the task as active, prefer automatic recovery to a reclaimable queue state instead of forcing manual retry.

## Scheduler Cancellation And Waiters

- If an abort-on-drop helper wraps a `JoinHandle` and also exposes an async `join`, do not `take()` the handle out before `.await`. Await it through `as_mut()` so dropping the wrapper during cancellation still aborts the child task instead of detaching it.
- If timeout or recovery code publishes in-memory task updates, every producer and waiter must share the same `TaskSchedulerService` instance or clones of it. Reconstructing a fresh service with the same repositories splits `task_waiters` state and breaks notifications.
- If a Mongo/task mapper validates retry invariants when reading documents back, mirror that validation at the write boundary too so invalid retry strategies cannot be persisted as poison rows.
- Process-scoped `OnceLock` temp fixtures with deterministic paths should proactively clear stale directories on initialization because `Drop` cleanup will not run at test-binary exit.

## Test Stability Diagnosis

- When diagnosing suite-level `SIGKILL` or memory-pressure failures, never launch multiple heavy `cargo test` targets in parallel. Those runs create artificial contention and make the results non-diagnostic.
- For flaky test-harness failures, prefer sequential reruns of the exact binary order from the failing suite, then inspect test helpers for leaked servers, background tasks, or retained global fixtures before changing production code.
- In parallel Rust test binaries, do not assert that a shared global capture registry is completely empty unless the helper truly owns every concurrent capture. Assert that the current test's capture was deregistered instead.
- Do not reuse `mongodb::Client` or similar async driver clients across separate `#[tokio::test]` runtimes via `OnceLock` or other process-global singletons. A client tied to a runtime that has already shut down can fail later with cancelled-task or runtime-shutdown errors.
- If this repo's broad Rust suites hit host-level `SIGKILL`s during verification, do not treat that alone as a product failure. Stop parallel test launches, switch to sequential targeted test filters for the touched behavior, and keep `cargo fmt --check`, `cargo clippy -D warnings`, and `bun run verify:arch` as the reliable completion gates.
- In async scheduler or worker tests, do not make the first assertion depend on eventually written worker-heartbeat projections. First synchronize on an owned signal like `Notify` or on primary task state, then poll the projected worker state without `expect(...)` until it catches up.

## Projection Window Semantics

- If a persisted snapshot exposes `start_date`, `end_date`, and a concrete `days` list, verify whether `start_date` is inclusive before writing bridge readers or tests. Do not silently encode `date > start_date` unless the model explicitly defines `start_date` as an anchor outside the visible plan.
- When a calendar/read-model row is missing, compare the durable source collection with the first canonical reader that reconstructs domain objects from it before changing refresh or cleanup logic. A missing read-model row can be caused upstream by an over-filtering root adapter, not by the projector itself.
- Planned-workout rebuild assertions must source sync metadata from `ExternalSyncStateRepository`, not from previously materialized `calendar_view` rows. Existing view sync may still be a fallback for other entry kinds, but planned entries intentionally clear stale view-only sync when authoritative external state is missing.

## Integration Test Scope

- Keep REST integration tests for transport-boundary behavior only: auth, user scoping, HTTP status mapping, request parsing/validation, body-size limits, and one simple happy path per endpoint.
- If a REST test is mostly checking domain decisions, DTO masking, normalization, merge logic, or repository fallback behavior, move that coverage into the relevant unit test module with fakes and delete the duplicate integration case.

## Test Memory Hygiene

- Test helpers must own every spawned background task. If a test starts `tokio::spawn(axum::serve(...))` or similar long-lived async work, keep the `JoinHandle` and abort or shut it down in `Drop`.
- Global test state must stay bounded. Never keep app fixtures, temp directories, or other per-test resources in an ever-growing `Vec` behind `OnceLock`, `Mutex`, or similar globals.
- If a test resource is expensive but safe to share and is not tied to a per-test async runtime, prefer a bounded per-binary singleton such as `OnceLock<FrontendFixture>` instead of recreating one instance per test. For async driver clients like `mongodb::Client`, first verify runtime safety; otherwise keep the client scoped to the test runtime.
- Sharing a client is not the same as sharing mutable data: keep per-test database names and mutable test records isolated even when the underlying client is reused.
- When a suite starts many HTTP mock servers or websocket apps, centralize that startup in a helper with cleanup semantics instead of open-coded `tokio::spawn` blocks in each test.
- When a suite gets `SIGKILL` only after many earlier test binaries pass, suspect retained test infrastructure first and inspect the binaries that run immediately before the failure point.
