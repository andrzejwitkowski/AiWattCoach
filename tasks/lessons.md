# Lessons

## 2026-05-29 - Long-lived websocket effects must not depend on unstable object props

- If a React effect owns a websocket, subscription, or other long-lived connection, do not include parent-created object references in its dependency list unless a new reference truly means a new connection target.
- In AI Coach, equivalent `aliasRange` objects and empty `cachedSummary` metadata updates retriggered the chat loading effect, which closed the active workout-summary websocket and opened a new one for the same workout repeatedly.
- Normalize dependencies to stable primitives such as `range.oldest`, `range.newest`, and the selected resource id. For cache-based fast paths, memoize only the subset that is actually usable by the effect; empty metadata cache updates should not reset realtime transports.
- Add rerender regressions around socket-owning hooks whenever a page-load optimization introduces caches, metadata summaries, or new parent-derived props.

## 2026-05-29 - Frontend schemas must match metadata DTO omissions

- If a backend DTO uses `skip_serializing_if = "Vec::is_empty"` for a field that full frontend screens parse through a shared schema, a missing field can surface as a raw Zod `invalid_type` before the user reaches the feature being debugged.
- For metadata endpoints that intentionally omit empty arrays such as workout-summary `messages`, either use a separate metadata schema or default the shared field to `[]` at the frontend boundary.
- When a screenshot shows a Zod path like `["messages"]` on page load, reproduce the exact GET/list response shape first. Do not assume the failing path is the later send/websocket action just because the user noticed it while trying to send.

## 2026-05-29 - Test fixtures with empty arrays must use real response types

- When a frontend test fixture starts with `messages: []`, do not build later async resolver types from `typeof fixture` unless the fixture itself is explicitly annotated. Otherwise TypeScript can infer `never[]` and make later realistic message objects fail only in `tsc -b` or production builds.
- For mocked API responses in tests, prefer the exported feature type such as `SendMessageResponse` over a hand-written local type stitched together from `typeof summaryFixture` plus overrides.
- After review-driven test edits in TypeScript-heavy frontend code, run the real `bun run --cwd frontend build`, not only Vitest, because stricter build-time checking can catch `never[]` inference that the test runner path misses.

## 2026-05-27 - Completed workout alias sets must include provider external ids

- If workout summaries are looked up from completed-workout ids coming from the UI, the alias set cannot stop at `source_activity_id` plus canonical completed-workout id. For imported workouts that exist in both Intervals and Wahoo identity spaces, the persisted summary may still be keyed by the provider external id.
- Before concluding that a recap was deleted, inspect the live `workout_summaries` document and compare it against the exact id sent by the frontend summary endpoint. A missing recap in the UI can be a read-side alias gap even when Mongo still has `workout_recap_text`.
- Reproduction for this class of bug should include dual identities for the same day/workout, for example one `intervals-activity:*` record and one `wahoo-workout:*`/external-id alias, then assert that reads from one id still find summaries persisted under the other.

## 2026-05-28 - Canonical matching inputs must not silently upgrade link strength

- If a completed-workout import needs an inferred `planned_workout_id` only to find the canonical completed workout, pass that id as lookup context instead of mutating the incoming workout before link selection finishes.
- In this flow, mutating `workout.planned_workout_id` too early caused `persist_legacy_planned_workout_link(...)` to create a synthetic `Explicit` link, which incorrectly outranked real `Token` and `Heuristic` matches and overwrote legacy planned ids.
- When canonical reuse and link persistence share one function, verify both outcomes explicitly after refactors: canonical record reuse and preserved `match_source` / legacy planned-id semantics.

## 2026-05-28 - Visible-range UIs must not batch hidden history summaries

- If a page presents one visible week or other explicit date window, fetch summary/status metadata only for entities in that visible range unless there is a confirmed product requirement for broader prefetch.
- In `AI Coach`, preloading workout summaries for a 12-week history window caused `GET /api/workout-summaries` to send 84 ids even though the sidebar showed one week, which amplified backend alias-resolution cost and led to `524` timeouts.
- When switching from eager preload to visible-range fetches, keep a small client-side cache of already loaded summary ids so paging back to a previously visited week does not re-fetch needlessly.
- For batch endpoints that can be hit by UI regressions, add a hard server-side max-id guard even after fixing the client. UI correctness and backend blast-radius limits are separate protections.
- After adding a server-side batch limit, audit the client path for the same limit immediately. A visible-range fetch can still exceed the cap on dense weeks if the client sends the whole week in one batch.
- In scope-narrowing regressions, assert exact fetch count in addition to the requested ids so an extra hidden request cannot slip through while the argument assertion still passes.

## 2026-05-28 - Summary handlers must not pre-check through a narrower completed-workout reader

- If a summary endpoint already delegates target validation and alias resolution to `WorkoutSummaryService`, do not add an extra `CompletedWorkoutReadService` existence check in the handler.
- In this repo, `CompletedWorkoutReadService` can sit behind `AuthoritativeCompletedWorkoutRepository`, which intentionally hides same-day duplicate aliases. That is correct for canonical workout detail reads, but wrong as a gate for recap lookup by alias.
- When one domain path is alias-aware and another is canonical-only or visibility-filtered, the handler must use the domain path that matches the endpoint contract as the single source of truth.

## 2026-05-27 - LLM JSON envelope parsers must tolerate provider presentation noise

- When a prompt asks for JSON, models may still wrap valid JSON in markdown fences, surround it with explanatory prose, or append harmless top-level metadata. Parser regressions should use the exact logged assistant content shape, not only ideal payloads.
- Keep the app contract strict for required fields such as `plan`, but avoid failing a workflow on extra top-level metadata unless that metadata would change behavior.
- First try to recover the owned JSON payload from the provider response, and only if that still fails use a narrow repair retry that asks the model to restate the same content as a clean envelope. Do not broaden that retry to empty-plan or blank-response cases.
- Check every user prompt in the same request path after introducing a JSON envelope; a lingering "return raw text only" instruction can fight the system prompt and increase provider drift.

## Review Fix Logging Loop

- When observability logs drop a field for redaction or privacy, grep integration tests that assert on tracing JSON (`description_preview`, request previews, etc.) and update expectations in the same change that edits the `tracing::info!` fields.
- When I implement a fix based on feedback from the user, Copilot, or CodeRabbit, I must record it in `reviewers.md`.
- Each `reviewers.md` entry must state both the problem that was identified and the fix that was applied.
- The purpose of this loop is to reduce repeated PR and review mistakes over time.
- I must read `reviewers.md` before writing a plan and before starting implementation work.
- When a frontend Zod schema intentionally leaves API payload fields as `unknown` or JSON-like values, any new helper that inspects nested data must narrow with `Array.isArray(...)` or another explicit type guard before calling array methods. Do not rely on optional chaining like `stream?.data.some(...)` because TypeScript will still reject `unknown` in CI/release builds.
- When repo instructions point at an Obsidian vault on Windows, store the exact subtree that contains the handbook notes, not only the vault root. A too-broad path wastes time and leads to false "file not found" lookups.
- Before any commit or push, explicitly verify the current branch name against the user's requested target branch. Do not rely on conversational memory when the user references "this branch" or another already-existing branch by name.
- When debugging where provider credentials or tokens come from, start from the exact runtime call site and walk the code path back through the service, port, and repository layers before searching storage blindly. For Wahoo FIT issues specifically, trace from `load_file_url(...)` and `get_workout_summary(...)` back to `ensure_token(...)` and then to `user_settings` in Mongo before assuming the OAuth client secrets are also persisted there.
- When an adapter bug corrupts a canonical entity id, enumerate every persisted store keyed by that id before writing the repair. Fixing only the first failed task or the primary repository row can leave orphaned summaries, sync metadata, or workflow state behind.
- If local data cannot safely derive the corrected external id, make the repair script mapping-driven with explicit verified bad->good id pairs and dry-run as the default. Prefer blocking on unsupported downstream state over silently applying a partial rewrite that strands related records.
- When a transport-layer identity mapping changes, grep for old literal ids in REST/integration tests as well as unit tests. Adapter regressions alone are not enough if a higher-level webhook or endpoint test still encodes the pre-fix canonical id.
- When a login flow recomputes user roles from config on every sign-in, verify whether that write path can unintentionally downgrade already persisted privileged roles. If manual admin grants are part of operations, add a regression where an existing admin logs in after `ADMIN_EMAILS` changed and confirm the persisted `admin` role survives.

## PR Conflict Verification

- When resolving PR conflicts, fetch the current base branch ref and test the merge against that exact `origin/<base>` immediately before calling the PR conflict-free. A branch can be clean and synced with its remote head branch while still conflicting if the base branch advanced.
- When resolving conflicts in rolling logs like `reviewers.md` or `tasks/lessons.md`, preserve entries from both branches and restore newest-first ordering instead of picking one side and dropping history.
- After fetching the base branch, also verify `gh pr view <number> --json mergeStateStatus` before telling the user a PR is conflict-free. A prior local `git merge` result is stale as soon as the base branch moves.

## Signature Change Verification

- When changing a function signature, grep every call site including local `#[cfg(test)]` modules in the same file before calling the refactor done. `cargo clippy --all-targets` compiles test targets too, so a missed unit-test call site will still fail CI even if the runtime code builds.
- For constructors with many positional arguments of the same type, re-check test fixtures against the canonical signature before treating behavior regressions as product bugs. A misordered fixture can silently move a link/id into the wrong field and produce misleading CI failures.
- When I change candidate-selection logic to depend on sync ownership, I must verify both sides of the filter separately: the losing duplicate should disappear, but the authoritative candidate must still survive. Also, if I preload sync states to guide filtering, I should reuse that batch for later projection instead of paying for a second identical lookup.
- For duplicate filters with asymmetric precedence, test the winning branch explicitly after each refactor. It is easy to make the loser disappear while still leaving the winner behind an older generic collision rule.
- When I intentionally tighten cleanup behavior for stale links or ownership rows, I must re-read older regressions in the same behavior group and update any test whose name or assertions still encode the superseded preservation contract. A passing new regression does not prove the old expectation was still valid.

## Fixture Refactor Verification

- When converting test helpers to a shared fixture strategy, grep for removed helper names as well as the new shared helper. Adjacent test builders often retain one stale call site that only shows up once `--all-targets` compiles that test binary.

## Backfill Recompute Ranges

- When a backfill or reimport operation can change canonical record dates, I must derive recompute ranges from the refreshed upstream payload, not from the stale local record.
- Before finalizing batch recompute logic, verify that the chosen `oldest_changed` date still covers records whose timestamps may be corrected during import.

## Test Doubles And Shapes

- When a sync workflow adds a discovery or recovery step before the previous happy path, revisit the touched test doubles immediately. A fake that used to be sufficient can silently stop modeling the branch the test name claims to exercise.
- Before adding a regression for a review-reported fallback branch, verify that the branch is actually reachable in the current production control flow. Do not write a test that depends on entries magically surviving an empty rebuild just to exercise a helper that the real loop never calls.
- In tests, avoid tuple aliases for multi-field call records when the field meaning matters. Use named structs or named sub-structs so assertions stay self-explanatory.
- When a function grows past a few distinct phases, split it into small helpers named after each phase instead of leaving one long orchestration block.
- When adding background work to a service method, keep the public method as a readable high-level flow. Extract the background job payload, async worker body, and visible status/message mapping into small private helpers before review has to reason through one giant nested block.
- When a test file grows large, split it by behavior group and extract shared fakes/fixtures into a local `support` module.
- When a service method starts mixing validation, request building, persistence, orchestration, and result interpretation, split it immediately into small helpers. Long public methods in scheduler/service code are hard to review and hide control-flow bugs.
- When converting a single-task loop into a concurrent worker pool, keep worker-level state in one shared runtime structure. Per-task copies of active-task state lead to lost heartbeats and flaky concurrency behavior.
- When a use-case orchestration method starts owning validation, claim/recovery, provider I/O, checkpoint writes, persistence completion, and result hydration all at once, split it into named phase helpers before adding more behavior.
- When a domain service `mod.rs` starts mixing transcript helpers, provider request building, persistence internals, and use-case orchestration, keep the public type in `mod.rs` and split the phases into sibling modules before the file grows into a review bottleneck.
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

- When prompt guidance includes both a generic tool-description line and a scope-specific line for the same tool, tests must target the scope-specific discriminator too. Matching the first line that mentions the tool name will often grab the generic guidance and produce a false failure.
- For scope-specific tool guidance, do not key follow-up logic off duplicated raw tool-name string literals if the tool types are already in scope. Derive the names from the tool implementations so guidance stays aligned with future rename-safe changes.
- Prompt-guidance tests should assert the contract at the line or phrase level, not full sentence literals, when wording can legitimately tighten during review. Keep the assertions strong enough to prove fallback-vs-supplemental semantics without pinning every connective word.
- For tracing/log-capture tests, verify the real captured log serialization shape before writing substring assertions. Some helpers return raw JSON log lines, not escaped JSON strings, so matching `\"field\"` can fail even when the logged metadata is correct.
- For prompt contracts shared across tool-capable and no-tool providers, do not write unconditional instructions like "use workout tools" into the base prompt. Phrase the fallback generically first, then mention tools only when available so the prompt stays true in Gemini/no-data-port paths too.
- When testing prompt wording, keep the exact literal contract assertions in one place near prompt construction and use only representative transport-path assertions elsewhere. Duplicating every sentence across unit and adapter tests makes review-driven wording changes noisy and brittle.
- When extracting a shared retry helper for a read-merge-write flow, keep the entire retryable attempt inside the retry closure, including the fresh read. If `load_latest()` lives outside the retry loop, transient read failures will bypass the intended retry/backoff semantics and the helper no longer matches its callers' optimistic-retry contract.
- For recovery-critical LLM tool-loop checkpoints, verify three separate boundaries: the terminal state is checkpointed before returning, checkpoint failure prevents a false success, and recovery from the checkpoint avoids a second provider call.
- When restoring untracked files from `git stash -u`, use the stash's untracked parent (`stash@{n}^3`) and verify the restored file is non-empty before moving on.
- When sending GitHub issue or PR bodies through shell commands, do not embed Markdown backticks inside double-quoted or `$'...'` shell strings. Build the JSON payload with a safe encoder such as `jq --arg` or a file input first, or shell expansion will silently corrupt paths and inline-code text.
- Before relying on `gh pr create`, verify GitHub CLI auth explicitly and remember that repo hooks may still run extra verification commands. If auth is missing or the hook path is risky on this host, fall back to direct GitHub REST API calls with already-available credentials.


- Before changing Intervals event payload field semantics, inspect the OpenAPI schema for the exact endpoint and method. For event create/update, `description` is the string field used by the existing planned-workout sync flow, while documented `workout_doc` is an object and must not be populated with the repo's canonical workout text string just because local DTOs expose that name.

- When a shared logged-body helper is reused across LLM adapters, redact structured JSON before serialization and truncate by char boundary lookup instead of scanning the full string just to decide whether to cut it.
- For OAuth adapter logging, redact authorization `code` fields explicitly even if the generic sensitive-key helper does not catch that exact name, and avoid logging raw token/userinfo success bodies when size-plus-hash diagnostics are enough.
- If provider request logs still leave LLM failures opaque, instrument the shared tool loop itself instead of only the outer adapter. For human debugging you usually need the whole sequence: round start, provider assistant message, tool call, tool result, and final assistant turn. If that is too verbose for normal operation, put the full payload logging behind an explicit env flag rather than forcing operators to guess from message counts.
- If one preview model path keeps returning structurally empty completions after the existing retry for that exact provider error, prefer a narrow workflow-specific fallback to a more stable model on the same provider before adding a global fallback. Keep the trigger exact so normal tool, rate-limit, and prompt errors still surface without silent model switching.
- When a shared tool loop or orchestrator changes from replaying an error into another provider round to stopping immediately, update adapter tests to match the new round-count and transcript contract. Do not leave tests asserting a second request or an older tool-local error payload after the control-flow boundary has intentionally moved up a layer.
- For Intervals.icu client transport logging, keep `BodyLoggingMode::None` as the shared default. Do not switch the shared trace helper to full request/response body previews just because one debugging task needs more visibility; add any body-preview logging only on the narrow call paths whose tests and logging guide explicitly permit it.
- If provider debugging requires payload inspection, verify outbound adapter clients log both request and response body previews on success as well as failure. Metadata-only logs are usually not enough for LLM, OAuth, or third-party API incident diagnosis.
- If an LLM aggregator intermittently returns an assistant turn with neither text nor tool calls, do not immediately relax downstream final-text validation. First contain it at the adapter boundary with a narrowly targeted retry for that exact transient response shape, and add a regression that proves the retry happens once and no more.
- If the shared tool loop rejects a tool call because the tool is unavailable in the active scope or runtime context, do not continue into another LLM round by default. Record the diagnostic `tool` message for persistence/debugging, then stop so the caller sees a deterministic invalid-response path instead of burning through the max-round limit on the same impossible tool call.
- If a live provider bug reappears in the same shape as an earlier resolved incident, first diff the current code against the last verified provider contract in git history. Do not assume the current branch still carries the earlier hotfix just because the repo has tests for it.
- If an external workout format already accepts the repo's canonical repeat-header syntax like `Main Set 4x`, do not add a provider-specific serializer that splits the title and repeat count across lines. Syntax-preserving reuse is safer than speculative reshaping.
- When adding a unique index to a shared sync-state collection, scope the partial filter to the exact canonical entity kind that requires the invariant. A broader `external_id`-only partial filter can turn harmless historical duplicates in other kinds into a startup rollout failure.
- When a shared trait or widely used domain struct changes shape, grep all implementations, merge helpers, and test fixtures immediately. Do not wait for compile errors to surface one file at a time.
- After adding one more input to an already busy helper, run clippy early and prefer a small input struct over crossing the repo's function-argument limit.
- For local frontend preview environments that depend on browser auth state or websockets, prefer same-origin `/api` traffic through the Vite proxy instead of a cross-origin `VITE_API_BASE_URL`. Browser previews are less fragile when `credentials: include` and websocket traffic stay same-origin.
- When replacing a full-history cleanup read with a lighter port method, verify the new adapter really narrows the fetched Mongo document shape. A new method name is not enough if it still materializes full payloads or builds the same huge `$in` set behind the scenes.
- If provider sync rows are shared across multiple canonical entity kinds, never reuse a generic `provider + external_id` lookup for a workflow that expects one specific kind. Add a kind-scoped repository method or kind filter and a regression with a conflicting row from another entity kind.
- If a trait method exists only to guarantee stricter semantics than another method, make that method required instead of delegating to the weaker behavior by default. Otherwise one forgotten impl quietly reopens the original bug.
- When replacing a bad sentinel timestamp with a derived timestamp helper, check every parse-failure path too. A hidden `unwrap_or_default()` can reintroduce the same sentinel under malformed input.
- Do not compare whole JSON objects as raw strings in tests. Object key order is not a stable contract; parse the payload and compare structured JSON values instead.
- When serializing float series into JSON payloads, normalize `NaN` and `+/-inf` before calling `json!` or serde numeric conversion. JSON has no non-finite numbers, so one bad sample can turn a payload builder into a panic path.
- If a tool/input schema declares `additionalProperties: false`, enforce the same rule in serde with `#[serde(deny_unknown_fields)]`. Do not let the runtime silently accept fields the schema promised to reject.
- For capped series payloads, do not use `div_ceil + step_by` when the exact sample budget matters. An index-based evenly spaced sampler preserves the tail and avoids collapsing `len = limit + 1` into roughly half the intended points.
- When a local app runtime is just another environment variant of the existing Docker setup, express it as a dedicated `docker-compose*.yml` file and keep `package.json` scripts as thin wrappers around `docker compose`. Do not hide build/run orchestration in long inline `bun -e` scripts.
- For dev-auth against a shared sandbox database, do not stop after verifying the OAuth redirect is local-only. Also verify that the configured mock identity corresponds to an existing allowed user in that database, or the app can still land in `pending approval` and be unusable.
- For Intervals planned-event writes, keep projected all-day `start_date_local` values on the same `YYYY-MM-DDT00:00:00` shape used by the repo's other Intervals event flows. Do not silently send bare `YYYY-MM-DD` dates from one code path while create/update payloads elsewhere use midnight datetimes.
- For provider workout text payloads, do not keep a custom near-duplicate serializer beside an existing canonical planned-workout serializer. If `calendar_view`, prompt-building, and provider sync all represent the same planned workout, make the outbound payload reuse the same canonical text shape unless there is a verified provider-specific exception.
- If a live provider write keeps failing and the repo has older working code for the same flow, diff against that exact historical implementation before inventing a new payload shape. A provider-specific contract that already worked in production is stronger evidence than a cleaner local abstraction.
- For Intervals planned-workout repeat blocks, verify live provider behavior end to end before changing grammar. The current known-good payload keeps titled repeats on one line as canonical text (`Main Set 4x`); do not split the title and repeat count without new upstream evidence.
- When a provider migration moves structured content from one field to another, keep shared sync hashes on the canonical comparable payload and strip only the old auto-generated field content during updates. Preserve real user notes, but do not let legacy generated text block idempotent matching or linger after the migration.
- If a component can render once with `selection = null` and later with real data, keep every hook call unconditional across both renders. Do not put new hooks after an early return that is skipped when the modal opens, or React will crash with a hook-order error.
- When moving data loading into a modal/container, add a regression that renders the component with `selection={null}` first and then rerenders with a selected entity. That transition catches real open-modal hook order bugs that steady-state tests miss.
- For detail modals, keep data fetching in a dedicated hook or container-level component and keep the rendered detail panel presentational. Do not thread transport/config props like `apiBaseUrl` into a child just to power one late-added fetch.
- When adding a new section to an existing detail view, verify whether the requirement is additive or substitutive before deleting the previous section. If an existing section still carries distinct user value, preserve it and add the new section alongside it.
- For completed-workout recap loading, derive the lookup id from the most authoritative completed-workout source available (`actualWorkout.activityId` before a best-effort loaded activity object) so partial activity-load failures do not suppress recap data.
- Do not surface raw `HttpError` transport strings like `GET /api/... failed: 503` directly in UI state. Map expected HTTP failures to user-facing localized copy unless the raw message is explicitly intended for the user.
- When adding a REST sub-route beside an existing resource route, match the surrounding path parameter name (`activity_id` vs `workout_id`) unless there is a real semantic distinction. Mismatched names create avoidable confusion in handlers, tests, and debugging.

- In a dual-read/dual-write timestamp migration, every field that is required at the domain level must be verified against a `DateTime`-only persisted document, not just legacy-plus-new mixed documents. If the persistence struct still uses a non-optional legacy epoch field, the migration is incomplete even when normal writes succeed.
- When promising a staged migration with a later backfill step, complete the backfill before calling the rollout done. Readability improvements are not actually delivered for existing Mongo rows until the historical documents are updated too.
- For dense UI mini-charts, keep the canonical sequence intact and apply any width compression only in the rendering layer. Sampling or equal-width fallbacks are likely to destroy either temporal order or duration meaning.
- When a UI bugfix intentionally removes one visualization in favor of another, grep sibling test files for stale assertions of the old visual contract before pushing. In React Testing Library, configure async API mocks before `render(...)` whenever the component fetches in `useEffect`, or the new regression test can become order-dependent and flaky.
- When frontend UX explicitly wants to use stylized planned-workout zone heights, do not silently revert to raw `%FTP` heights to make old tests pass. Update the tests to the intended visual contract instead.
- If a Docker/Podman Rust build fails on a normal crates.io dependency with an impossible manifest error, suspect a corrupted BuildKit cache before changing dependencies. Cache `cargo/registry/cache` and `cargo/registry/index`, but do not persist `cargo/registry/src` across builds because partially extracted crate sources can poison later container builds.

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
- When a scheduler or worker test waits for `Notify`, `watch`, or channel startup signals, never block on the raw await without a timeout. Wrap the wait in `tokio::time::timeout(...)` or a helper so broken startup synchronization fails fast instead of hanging the suite.
- Test helpers with names like `only_task()` or `only_worker()` must assert their singular-state contract instead of returning the first `HashMap` entry silently. Otherwise later tests can pass while exercising the wrong fixture shape.
- In-memory repositories used by scheduler/worker tests must not depend on `HashMap` iteration order for claim/selection behavior. Choose explicitly by the same ordering the production path intends, and preserve stored fields like `active_task_ids` when updating heartbeat metadata unless the real implementation is meant to clear them.

- For websocket-backed LLM chats, a single initial typing/progress frame is not enough when the backend can spend tens of seconds waiting on the provider. If the handler then blocks silently on the long-running reply, proxies can drop the socket even though the server eventually persists the final answer. Use a keepalive loop that emits periodic progress frames until the reply task completes.
- If a test exists only to verify timer-driven behavior like websocket keepalives or delayed retries, do not make it wait on real wall-clock time. Use paused Tokio time (`#[tokio::test(start_paused = true)]`) and explicit `tokio::time::advance(...)` so the test is fast and deterministic.
- If the behavior under test is a timer loop inside a larger websocket or network flow, prefer extracting that loop into a small helper and fake-time testing the helper directly. Advancing virtual time through full websocket I/O adds unrelated scheduling machinery and can still leave the test brittle.

## Test Stability Diagnosis

- When diagnosing suite-level `SIGKILL` or memory-pressure failures, never launch multiple heavy `cargo test` targets in parallel. Those runs create artificial contention and make the results non-diagnostic.
- On this machine, do not launch multiple verification commands that compile the same Rust workspace in parallel. Shared Cargo package/artifact locks can turn normal checks into long `Blocking waiting for file lock` stalls and misleading timeout failures.
- For flaky test-harness failures, prefer sequential reruns of the exact binary order from the failing suite, then inspect test helpers for leaked servers, background tasks, or retained global fixtures before changing production code.
- In parallel Rust test binaries, do not assert that a shared global capture registry is completely empty unless the helper truly owns every concurrent capture. Assert that the current test's capture was deregistered instead.
- Do not reuse `mongodb::Client` or similar async driver clients across separate `#[tokio::test]` runtimes via `OnceLock` or other process-global singletons. A client tied to a runtime that has already shut down can fail later with cancelled-task or runtime-shutdown errors.
- If this repo's broad Rust suites hit host-level `SIGKILL`s during verification, do not treat that alone as a product failure. Stop parallel test launches, switch to sequential targeted test filters for the touched behavior, and keep `cargo fmt --check`, `cargo clippy -D warnings`, and `bun run verify:arch` as the reliable completion gates.
- In async scheduler or worker tests, do not make the first assertion depend on eventually written worker-heartbeat projections. First synchronize on an owned signal like `Notify` or on primary task state, then poll the projected worker state without `expect(...)` until it catches up.
- On this machine, do not run the full `cargo test --lib` unit-test binary as a routine verification step because it is known to get killed with host-level `SIGKILL`. Prefer narrower verification such as `cargo check --lib`, `cargo clippy`, focused integration tests, and only attempt the broad lib binary when the user explicitly wants that risk.

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

## Misleading Struct Literal Placeholders

- Never set a struct field to a placeholder value in a builder or request constructor when that value is unconditionally overwritten later by an orchestrator, loop, or mapper. Examples include `tools: Vec::new()` and `tool_choice: LlmToolChoice::None` in an `LlmChatRequest` that is immediately passed to `run_tool_loop`, which replaces both fields based on scope. The placeholder makes every reader think the request is sent without tools, which hides the real behavior and wastes debugging time.
- If a downstream stage owns a field, omit it from the upstream builder. Use `..Default::default()` or a dedicated builder helper so the struct literal only contains values the current layer is actually responsible for.
- When reviewing code that builds a request and then passes it to an orchestrator, check whether every field in the literal survives to the wire unchanged. If a field is reassigned before the wire call, remove it from the literal and let the orchestrator set it.

## Scheduler Error Propagation

- When a scheduler-backed workflow already has typed domain errors like `Llm`, `Validation`, or `NotFound`, persist that typed error in the task checkpoint and restore it in the result handler. Do not collapse every failed task back into `Repository(error_message)`, or websocket/REST layers will surface the wrong public error and hide the real root cause.

## Shared Adapter Clients And Provider Identity

- When a shared adapter client serves multiple providers (e.g., an OpenAI-compatible client used for both OpenAI and DeepSeek), every piece of code that identifies the provider must use the runtime config, not a hardcoded literal. Specifically: response `provider` metadata must come from `config.provider`, log messages must use `config.provider.as_str()` instead of a hardcoded string literal, and error messages must format the provider dynamically.
- Before promoting an adapter from single-provider to multi-provider, grep for hardcoded provider identifiers in the module: response struct fields, log statements, error message strings, and response metadata.
- When adding a new provider whose cache tokens come in a different JSON shape than existing providers, add a focused adapter regression that proves the correct path is exercised for that specific provider. Do not assume the existing test covers it.

## New Field Enumeration In Shared Schemas

- When a backend response enum grows a new serialized variant like `processing`, grep every frontend Zod enum, TypeScript test union, and UI branch that hardcodes the old closed set. Backend DTO parity is not real until the frontend parser and its focused tests accept the new wire value.

- When adding a new field to a shared API schema (backend DTO, frontend Zod schema, or request/response type), grep for every place the old fields are enumerated and add the new one. Common places to miss: frontend payload builders, frontend API extraction functions, TypeScript discriminated unions, backend match arms, backend `apply_field_update` calls, test fixture builders, and mock data objects.
- A schema update alone is not enough if the code that serializes, extracts, or transforms the payload still iterates over the old field set. Add a focused end-to-end test that exercises the new field through the full stack (UI → API → backend → response → UI) to catch gaps in payload plumbing.

## 2026-05-13 - Verify remote branch before claiming push succeeded

- Problem: I said the branch was pushed even though `origin/feat/shared-public-tool-materialization` was still behind the local commit.
- Rule: After every push, confirm with a fresh remote comparison such as `git fetch origin` plus local vs remote SHA check before telling the user it is on GitHub.
- Reusable check: `git push origin HEAD:<branch> && git fetch origin && printf "LOCAL %s\nREMOTE %s\n" "$(git rev-parse HEAD)" "$(git rev-parse origin/<branch>)"`
