# Reviewers Log

Records fixes made in response to review feedback. Read before planning and before implementation.

Scan newest entries first. Focus on entries matching the current task area or failure mode.

## Entry Format

- Date: `YYYY-MM-DD` | Source: user | Copilot | CodeRabbit
- Problem + Fix. Prevention: what to check next time.

## Entries

### 2026-05-31 | user | LLM coach prompts lack authoritative conversation timing
- Outbound LLM prompts carried only weak date context; transcripts omitted per-message timestamps. Model could guess a same-day reply was sent the next day.
- Added RFC3339 timing helpers + `conversation_timing` block in `volatile_context` across all four LLM surfaces. Prefixed transcript messages with `sent_at=...`. Added `created_at_epoch_seconds` to `TrainingPlanConversationMessage`. Refactored `AthleteSummaryLlmGenerator` to use injected `Clock`.
- **Prevention**: Keep timing in `volatile_context` to preserve reusable context caching. Audit fallback paths for impossible sentinel timestamps. Keep clock injection consistent across all adapter-generator types. Extend per-message timing to sibling LLM surfaces unless the domain model blocks it.

### 2026-05-29—30 | user + CodeRabbit | AI Coach stitch: websocket lifecycle, cache hydration, fallback send, message limits
- **Websocket reconnect loop**: Page-load optimization passed unstable object props (`aliasRange`, `cachedSummary`) as effect dependencies, closing sockets on every parent rerender. *Fix*: switched to stable primitive dependencies; separated cache hydration from socket lifecycle. *Prevention*: long-lived transport effects must depend on primitives, not parent-created objects.
- **Cache hydration overwriting websocket state**: Cache accepted any non-empty summary for the same workout, so stale tool-only cache could remove the final coach reply. *Fix*: compare freshness before applying cached data. *Prevention*: when realtime and cache both hydrate the same transcript, compare timestamps before overwriting.
- **Chat auto-scroll fighting user scroll**: `scrollIntoView` fired on every streamed update. *Fix*: scroll only when user is near the bottom. *Prevention*: auto-scroll must be conditional on scroll position.
- **REST fallback error flash**: Stale websocket error stayed visible during REST fallback. *Fix*: clear error before awaiting fallback. *Prevention*: clear stale transport errors before awaiting fallback requests.
- **Websocket fallback gap**: `useCoachChat` tried websocket-only sends with no REST degrade path. *Fix*: added REST fallback when socket connection fails. *Prevention*: keep REST path wired as degraded-mode fallback for websocket-backed chats.
- **Coach message cap too low**: 2000-char cap rejected realistic post-workout feedback. *Fix*: raised to 10,000 in backend domains and frontend schemas. *Prevention*: trace all enforcing layers before changing limits; align parallel surfaces.
- **Metadata summaries omitted `messages`**: Backend metadata endpoint omitted empty `messages`; frontend Zod required it. *Fix*: defaulted `messages` to `[]` in schema. *Prevention*: when backend DTOs omit empty arrays in lightweight responses, default the field or use a separate schema.
- **Large hook tests**: `useCoachChat.test.tsx` crossed 1k lines. *Fix*: split by behavior group. *Prevention*: split before crossing 1k lines.

### 2026-05-28 | user + Copilot + CodeRabbit | AI Coach visible-week range, handler pre-check, import matching
- **Visible-week batch limit**: Frontend fetched 84 summaries for a 12-week window while sidebar showed 1 week. *Fix*: fetch only visible-week ids; add server-side max-id guard of 31; client-side chunk to 31. *Prevention*: align request scope to the visible range; add a server-side cap even after client fix; chunk on the client when dense weeks can exceed the limit.
- **Summary handler pre-check hid cross-source recap**: Completed-workout pre-check in the handler used a narrower reader that hides duplicate aliases, rejecting valid recap reads. *Fix*: removed redundant pre-check; summary service is the single source of truth. *Prevention*: do not add a second transport-layer gate through a differently filtered read path.
- **Canonical matching upgraded link strength**: `resolved_link.planned_workout_id` pushed onto the incoming workout before canonical resolution synthesized a temporary `Explicit` link. *Fix*: pass as lookup context instead of mutating the entity. *Prevention*: do not mutate entities with inferred ids before ranking/merge steps.
- **Cross-source import reuse**: Intervals/Wahoo imports for the same workout could split into two canonicals. *Fix*: match by `planned_workout_id` and `external_id` before fingerprint fallbacks. *Prevention*: treat stable shared ids as stronger canonical anchors than fingerprint fallbacks.

### 2026-05-27 | user + Copilot | workout summary alias, training plan JSON parsing, repair retry
- **Workout summary alias gap**: Recap reads could 404 because `equivalent_workout_ids` missed the external alias. *Fix*: included `CompletedWorkout.external_id` in the alias set. *Prevention*: include every persisted identity that can legitimately key the same summary document.
- **Training plan JSON envelope parsing**: Provider responses wrapped valid JSON in fences and prose. Parser rejected it. *Fix*: hardened parser to extract fenced/embedded JSON; ignore harmless metadata; added narrow repair retry for syntax failures only. *Prevention*: test the exact logged assistant content shape. First recover the payload; only then fall back to a tightly scoped repair. Do not broaden repair to empty-plan cases.

### 2026-05-26 | user + Copilot + CI | training plan JSON envelope follow-up, completed workout visualization
- **Prompt contract drift**: Adapter test still asserted pre-envelope grammar after refactor. Shared LLM stub switched on scenario prose. Day-block flush cloned needlessly. *Fix*: updated assertions, keyed stub on schema, used `std::mem::take`. *Prevention*: when prompt contracts change, update transport-path assertions. Key structured-response stubs off the schema, not prose. Prefer moving with `take()` over clone-and-clear.
- **Training plan description field dropped**: Service still used old convenience setters that dropped the new `description` field. *Fix*: switched to payload-aware setters. *Prevention*: grep the persistence path for older setters when adding fields.
- **Completed workout visualization**: Hidden bars also removed them from views without overlay context. Test had order-dependent mocks. *Fix*: narrowed the condition, moved mocks before render, updated sibling assertions. *Prevention*: check sub-modes before removing shared visualizations. Configure mocks before `render(...)` when the component fetches in `useEffect`.

### 2026-05-25 | user + Copilot + CodeRabbit | mobile preview, auth flow, power trace
- **Local preview auth**: Cross-origin `VITE_API_BASE_URL` broke browser auth. *Fix*: switched to same-origin Vite proxying. *Prevention*: prefer same-origin proxying for local previews that depend on cookies or websockets.
- **Mobile calendar controls**: Duplicate buttons mounted in DOM hidden by CSS. *Fix*: render exactly one control per viewport. *Prevention*: do not rely on `hidden` classes when duplicate accessible labels remain.
- **SVG clipPath**: Raw `useId()` output broke `url(#...)` lookups in some browsers. *Fix*: sanitized colon characters from ids. *Prevention*: sanitize `useId()` output before using in `url(#...)` refs.
- **Power trace preview guard**: Full power extraction ran just to choose preview bars. *Fix*: added cheap presence check. *Prevention*: when a decision only needs stream presence, do not run the full extraction path.

### 2026-05-24 | user + Copilot + CodeRabbit | identity roles, Wahoo canonical id, admin scheduler
- **Login role overwrite**: `handle_google_callback` recalculated roles on every login, silently removing manual admin grants after redeploy. *Fix*: preserved existing `Role::Admin` when refreshing known users. *Prevention*: verify login flows cannot downgrade persisted privileged roles.
- **Wahoo summary-only canonical id**: `workout_summary.id` contaminated every store keyed by canonical workout id. *Fix*: corrected DTO, added mapping-driven repair script, dry-run by default. *Prevention*: enumerate every persisted store keyed by the corrupted id. Make repair scripts mapping-driven when local data cannot derive the corrected id.
- **Admin scheduler page**: Hardcoded labels, stale detail responses, no frontend API hook, default limit used max instead of `DEFAULT_TASK_LIST_LIMIT`. *Fix*: added hook, localized, guarded async selection, fixed default. *Prevention*: for new feature APIs, expose a hook wrapper. Guard async selection with request sequencing. Verify default-limit constants.

### 2026-05-14—22 | user + Copilot + CodeRabbit | workout-summary questionnaire, split-brain detector, tool guidance
- **Save workflow status drift**: Backend returned `processing`; frontend Zod allowed only terminal states. *Prevention*: when a backend enum gains a variant, grep every frontend Zod enum and status-branch assertion.
- **Questionnaire UI**: Allowed partial multi-question submits; stayed disabled on reject. *Prevention*: verify submit gate matches intended completeness contract; add failure-path regression.
- **Coach reply parser**: Treated plain LLM replies as invalid. *Fix*: made non-JSON replies parse as summary-only. *Prevention*: define explicit fallback for usable plain-text model replies.
- **LLM JSON schema in prompts**: Hand-written contract drifted from parser DTO. *Fix*: derived schema from `schemars`. *Prevention*: derive prompt schema from the exact parser DTO when feasible; keep it feature-scoped.
- **Split-brain detector**: Invalid env values silently treated as `false`; repair deleted rows from snapshot without revalidation. *Prevention*: fail fast on malformed env flags. Revalidate preconditions before destructive writes.
- **Workout-summary tool guidance evidence hierarchy**: Tool guidance did not clearly demote aggregate metrics or make the fallback hierarchy explicit. *Prevention*: mirror tool-level clarifications in the narrowest scope that needs them.
- **Prompt test brittleness**: Duplicated literal contract across source-file and adapter tests. *Prevention*: keep exact wording checks near prompt construction; lighter transport-path assertions elsewhere.

### 2026-05-06—14 | user + Copilot + CodeRabbit | task scheduler, planned-workout sync, calendar coach, LLM logging, Gemini cache, provider identity
- **Planned/completed split-brain**: Stale Intervals sync owners and stale links kept completed workouts attached to superseded planned ids. *Prevention*: when same-day planned workouts can be superseded, verify all three stores move together: projected ids, completed-to-planned links, and provider ownership rows.
- **Shared task scheduler**: Duplicated `TaskHandler` glue across five workflows. *Fix*: extracted shared domain primitives; moved runtime worker spawning to config wiring. *Prevention*: when multiple features repeat the same adapter shape, extract shared glue before touching features individually.
- **Scheduler error propagation**: Calendar coach flattened typed `Llm(...)` failures to `Repository(...)`. *Prevention*: persist typed domain errors in task checkpoints; restore them in result handlers.
- **Planned-workout Intervals update payload alignment**: Proposed payload incorrectly populated `workout_doc` (an object) with a string. Known-working path uses `description`. *Prevention*: inspect OpenAPI schema for the exact endpoint before changing provider field semantics.
- **Planned-workout sync status stale**: Frontend synced but calendar grid still showed stale status. *Prevention*: when a modal mutates data rendered in a parent collection, patch parent store/cache in the same flow.
- **LLM tool-loop debug logs**: Failures opaque because logs showed outbound requests but not the tool-loop progression. *Fix*: added shared `run_tool_loop` tracing. *Prevention*: make sure logs can reconstruct the full round trip. Gate full payloads behind a debug flag if too noisy.
- **Body log preview scope**: Intervals client was broadened to full body previews for one debugging task. *Prevention*: keep transport logging at `None` as shared default; add body preview only on narrow call paths.
- **OAuth adapter logging**: Token exchange logs emitted authorization codes and raw success bodies. *Prevention*: redact `code` fields explicitly; prefer body size/hash diagnostics for sensitive responses.
- **LLM log redaction**: Shared `serialize_logged_body` serialized raw secret-bearing payloads before truncation. *Prevention*: redact structured JSON before serialization; truncate by char boundary.
- **Gemini context cache**: Cache-create success logs computed timestamps from stale pre-assignment state. *Prevention*: compute logged values after the state they describe is assigned.
- **Multi-provider client identity**: Shared OpenAI-compatible client hardcoded `"openai"` in logs, errors, and response metadata. *Prevention*: every provider identifier must use runtime config. Grep for hardcoded literals when promoting from single-provider to multi-provider.
- **Cache token shape per provider**: DeepSeek cache tokens came at top-level `usage`, not nested in `prompt_tokens_details`. *Prevention*: add focused adapter regression for each provider's cache-token shape.
- **Empty-turn retry**: Gemini flash preview returned consecutive empty assistant turns. *Fix*: narrow fallback to Pro model for that exact signature. *Prevention*: contain provider-specific transient failures at the adapter boundary with narrow retries.
- **REST body preview limit**: Default 10 KB was too low for current debugging. *Fix*: raised to 200 KB. *Prevention*: when changing a default also restated in route wiring and docs, grep the raw byte value everywhere.
- **Long-reply websocket disconnect**: Coach replies >40s stayed silent after initial typing frame; proxies dropped the socket. *Fix*: keepalive loop with periodic progress frames; test timer helper under fake time. *Prevention*: do not await long-running LLM replies in silence. Test timer behavior with fake time, not real wall-clock.
- **Workout-summary save workflow**: Background recap/training-plan generation with unbounded concurrent provider work. *Fix*: process-level semaphore limiting active workflows; test synchronization with timeout polling, not `yield_now()`. *Prevention*: verify active concurrency and test synchronization explicitly for background workflows.

### 2026-05-02—05 | user + Copilot + CodeRabbit | earlier foundation fixes
- **Canonical-explicit-link cleanup regression**: Stale regression still expected orphaned explicit links to survive after intentional cleanup. *Prevention*: when a bugfix changes stale-data cleanup semantics, grep neighboring regressions for old preservation wording.
- **Provider transcript merge retry**: Extracted helper left `load_latest()` outside the retry loop. *Prevention*: keep every fallible step of one retry attempt inside the retry closure.
- **Public tool materialization**: Duplicate persisted ids survived forever; recovery paths ignored returned normalized id set. *Prevention*: normalize both incoming and persisted state. Audit every caller for whether it persists the returned updated value.
- **Task scheduler worker test infrastructure**: In-memory repos depended on `HashMap` iteration order; `only_task()` silently returned first entry; heartbeat updates recreated empty `active_task_ids`. *Prevention*: make claim/selection deterministic. Assert singular-state contracts. Preserve stored active state in heartbeat updates.
- **DeepSeek integration review loop**: Missed enum fields in every layer: frontend payload builders, cache invalidation, REST integration tests, mapping tests. *Prevention*: when adding a new field to a shared schema, grep every serializer, extractor, match arm, and test fixture. A schema update alone is not enough.
- **Misleading struct literal placeholders**: `tools: Vec::new()` and `tool_choice: LlmToolChoice::None` in `LlmChatRequest` passed to `run_tool_loop` were unconditionally overwritten. *Prevention*: when a downstream stage owns a field, use `..Default::default()` instead of placeholder values.
