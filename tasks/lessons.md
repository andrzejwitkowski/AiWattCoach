# Lessons

## Frontend

### Component, state, and hook architecture
- **File size**: Split test files by behavior group before they cross 1k lines. Split hooks before they mix transport, cache, and UI state.
- **State centralization**: When a hook ingests the same full read model from cache, websocket, REST, or reload paths, use one canonical state-application helper with an explicit freshness policy (`'force' | 'newer-only'`).
- **Effect dependencies**: Effects owning sockets or long-lived connections must depend on stable primitives, not parent-created object references. Keep cache hydration and transport lifecycle in separate effects.
- **Auto-scroll**: Conditional auto-scroll only on first render or when the user is near the bottom. Do not fire `scrollIntoView` on every streamed update.
- **Modal data loading**: If a component renders `selection={null}` then later real data, keep every hook call unconditional. Add a null→entity rerender regression. Fetch data in a dedicated hook/container, keep rendered panels presentational.
- **Prop threading**: Never thread `apiBaseUrl` through component props. Use `useApiBaseUrl()` or a feature-specific hook that wraps all backend HTTP calls.
- **Localized strings**: When adding new i18n keys covered by mocked translations, update the test mock in the same change.
- **Breakpoint controls**: Render only the active control per viewport. Do not rely on `hidden` classes when duplicate accessible labels would remain in the DOM.

### Schema and API contracts
- **Backend DTO omissions**: If a backend DTO omits empty arrays in metadata responses, the frontend Zod schema must default those fields or use a separate metadata schema.
- **Enum changes**: When a backend enum gains a new variant, grep every frontend Zod enum, TypeScript union, and UI branch for the old closed set.
- **Type inference traps**: When a frontend test fixture starts with `messages: []`, do not derive async resolver types from `typeof fixture` unless explicitly annotated. TypeScript can infer `never[]`. Prefer the exported feature type. Run `bun run --cwd frontend build` to catch these.
- **New API fields**: When adding a field to a shared schema, grep every serializer, extractor, match arm, and test fixture for the old field enumeration.

### Error and fallback handling
- **Fallback UX**: When a fallback path intentionally recovers from an earlier transport error, clear stale error state before awaiting the fallback request, not only after success.
- **Error mapping**: Do not surface raw `HttpError` transport strings in UI. Map expected failures to localized copy.
- **Input limits**: When adjusting user-facing limits, trace every enforcing layer (domain validation + frontend Zod) and align parallel coach surfaces.

## Backend Architecture

### Domain and adapter boundaries
- Domain code must not import from `adapters`. Keep Axum, Mongo, and provider SDK types in adapters; map at boundaries.
- REST handlers stay thin: validate input, delegate to services, map errors. No domain logic, no external API calls.
- Keep `spawn_task_worker(...)` and runtime loops in `config/` or test helpers. Domain task contracts stay in `src/domain/task_scheduler/**`.

### Error handling and recovery
- **Alias resolution**: If a summary endpoint delegates to an alias-aware domain service, do not add a second pre-check through a narrower reader. Use the domain path that matches the endpoint contract as the single source of truth.
- **Typed errors in schedulers**: Persist typed domain errors in task checkpoints, restore them in result handlers. Flattening everything to `Repository(error_message)` hides root causes.
- **Repair scripts**: Map-driven, dry-run by default. Revalidate preconditions immediately before destructive writes. Block on unsupported downstream state.

### Function and file structure
- Keep functions at or below ~100 lines. Refactor above ~130.
- Keep files under ~500 lines. Split early: `mod.rs` + siblings by concern (`dto`, `handlers`, `mapping`, `error`, `validation`).
- When a service method mixes validation, request building, persistence, orchestration, and result interpretation, split it into named phase helpers.
- Avoid placeholder struct field values that are unconditionally overwritten by a downstream orchestrator. Use `..Default::default()`.

### Clock and time
- Inject `Clock` (trait `Clock`) into every adapter-generator that needs current time. Do not use `SystemClock` directly.
- Put dynamic conversation timing in `volatile_context`, not `system_prompt`. `reusable_context_cache_key` derives from `system_prompt + stable_context`, so dynamic time in system prompt breaks cache reuse.
- Every fallback timestamp path must be safe: `unwrap_or(0)` is worse than no timestamp.

### Sync and idempotency
- Persist local state before external side effects. For LLM tool-loop checkpoints: write checkpoint before returning, prove failure blocks success, prove recovery avoids a second provider call.
- When extracting a retry helper for a read-merge-write flow, keep the entire attempt inside the retry closure including the fresh read.
- For in-memory waiter registries backed by `watch`, verify cleanup on both live-update and immediate-terminal replay paths.

## LLM / Prompt Engineering

### Prompt construction
- **Timing metadata**: Send authoritative conversation timing as `conversation_timing` in `volatile_context`, with RFC3339 format. Per-message `sent_at=...` prefixes in transcripts are a stronger signal than a single `today` field.
- **Surface parity**: After a prompt fix, audit every LLM surface (workout-summary, calendar coach, training-plan, athlete-summary) for timing parity.
- **Dynamic vs. static**: Keep prompt text that determines reusable context caching in `system_prompt` and `stable_context`. Keep dynamic timing in `volatile_context`.
- **Optional capabilities**: Do not write unconditional tool-usage instructions into the base prompt. Phrase fallback generically first, mention tools only when available.
- **Prompt tests**: Keep exact literal contract assertions near prompt construction. Use lighter transport-path assertions elsewhere.

### Structured output and parsing
- Models may wrap JSON in markdown fences, prose, or metadata. First recover the payload; fall back to a narrow repair retry only for syntax failures, not semantic ones.
- If a repair prompt embeds prior model output, never wrap it in the same fence syntax the provider may have used. Use dedicated literal-content markers.
- Keep user prompts consistent with system-level JSON schema instructions.

### Tool loops and provider behavior
- When a shared tool loop rejects an unavailable tool, stop immediately instead of burning through max rounds.
- If a provider intermittently returns neither text nor tool calls, contain it at the adapter boundary with a narrow retry for that exact shape. Do not broaden downstream validation.
- For multi-provider shared clients, every provider identifier (response metadata, logs, errors) must use the runtime config, not hardcoded literals.
- When adding a new provider field to cache invalidation or config comparisons, add it everywhere.
- **Plan→actual alignment IDs**: Resolve `align_id` / focus matching against the **post-dedupe** packed activity set, not the first Mongo list match. Intervals+Wahoo share `source_activity_id` but pack different legacy `activity_id`s; pre-dedupe first-match makes `focus_kind=summary` and leaves raw `ps`/`cs`.
- **Plan link on Wahoo winner**: Plans sync to Intervals so `planned_workout_id` often lives only on the Intervals completed sibling; Wahoo wins prompt dedupe for FIT. When preferring Wahoo, merge the loser's `planned_workout_id` onto the winner (`or`), same idea as import `merge_completed_workout`. Otherwise packed context keeps raw `ps` and never emits `sa`.

## Testing

### Test scope and design
- REST integration tests: auth, user scoping, HTTP status mapping, body/query validation, request size limits, one happy path per endpoint. Move domain-rule tests to unit test modules with fakes.
- One assertion per test when practical. Name tests descriptively.
- Avoid comparing whole JSON objects as raw strings. Parse and compare structured values.
- In date-sensitive tests, capture one baseline time and derive related keys from it; do not call `new Date()` repeatedly.

### Fakes and doubles
- When function signatures change, grep every call site including local `#[cfg(test)]` modules.
- For constructors with many same-type positional arguments, re-check test fixtures against the canonical signature.
- Repository helpers that imply singular state must assert that explicitly.
- In-memory repositories must not depend on `HashMap` iteration order for claim/selection.
- Heartbeat-style updates must preserve previously stored active state unless the contract explicitly clears it.
- When a sync workflow adds a discovery step, revisit touched test doubles—a fake that was sufficient before may silently stop modeling the new branch.

### Test infrastructure and hygiene
- Test helpers must own every spawned background task (`JoinHandle`, abort on `Drop`).
- Global test state must stay bounded. Do not accumulate per-test resources in global `Vec`s behind `OnceLock` or `Mutex`.
- Do not reuse `mongodb::Client` across separate `#[tokio::test]` runtimes via process-global singletons.
- Wrap `Notify`/channel awaits in `tokio::time::timeout`. Broken synchronization must fail fast.
- For timer-driven behavior, use paused Tokio time, not real wall-clock waits.
- On this machine: do not run multiple `cargo test` targets in parallel. Do not run the full `cargo test --lib` binary as routine verification (known `SIGKILL` risk). Prefer `fmt --check`, `clippy -D warnings`, `verify:arch`, and focused integration tests.

## Operations

### Deployments and migrations
- Login flows must not silently downgrade persisted admin roles on redeploy or env drift.
- When migrating Mongo documents to dual timestamp storage, remove `expect(...)` from mappers for collections with legacy rows.
- For BSON `DateTime` to epoch conversion, use euclidean division on milliseconds for correct negative timestamp floors.
- If a new collection has no legacy rows, keep required fields as required—do not weaken to `Option<T>`.

### Provider and data fixes
- When an adapter bug corrupts a canonical entity id, enumerate every persisted store keyed by that id before writing the repair.
- When a transport-layer identity mapping changes, grep both unit and integration tests for old literal ids.
- For provider-auth debugging, trace from the runtime call site back through services, ports, and repositories before searching storage heuristically.

### Scripts and tooling
- For operational repair scripts: fail fast on malformed env flags, revalidate preconditions before destructive writes.
- When posting Markdown to GitHub from shell, never embed backticks inside shell string literals. Use `jq --arg` or a file.
- Shell scripts that shell out to `docker build` must anchor working paths to the repo/script location.

## Development Process

### Review and verification
- Read `reviewers.md` and `tasks/lessons.md` before planning and before implementation.
- After any correction from the user, record the lesson in an appropriate category above.
- After implementing a non-trivial change, verify: relevant test suite, `cargo fmt --all --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `bun run verify:arch`, `./scripts/rebuild_graphify.sh`.
- **This Linux dev box is RAM-limited:** use `CARGO_BUILD_JOBS=1` / `cargo -j 1`, run only one `cargo` process at a time, and prefer narrow `cargo test <filter> -- --test-threads=1` instead of full-workspace `cargo test`. Parallel `rustc` can OOM the host and kill the IDE.
- Do not run multiple heavy verification commands in parallel on this machine.

### Git and PR workflow
- Before committing or pushing, verify the current branch name against the requested target.
- After every push, confirm with `git fetch origin` + local vs remote SHA comparison.
- When resolving PR conflicts: fetch the base branch ref and test the merge against `origin/<base>`. Verify `gh pr view --json mergeStateStatus`.
- When resolving conflicts in rolling logs, preserve entries from both branches in newest-first order.
