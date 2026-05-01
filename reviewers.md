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

### 2026-05-01 | user | Windows Obsidian vault path correction

- Problem: the repo instructions in `AGENTS.md` said to check the Windows Obsidian vault under `E:\obsidian\vault`, which is too broad and caused me to look in the wrong place for the OpenCode handbook links.
- Fix: narrowed the Windows path in `AGENTS.md` to the actual handbook location `E:\obsidian\vault\opencode` so future sessions open the correct note tree first.
- Prevention: when documenting platform-specific note locations, point to the exact subfolder that contains the required handbook entry, not just the vault root. If the first linked note is under a known subtree like `opencode`, encode that full path directly in the repo instructions.

### 2026-05-01 | user | sandbox rollout fix for external sync unique index

- Problem: the new `external_sync_states_user_provider_kind_external_id_unique` index was only partial on `external_id` type, so rollout tried to enforce `(user_id, provider, canonical_entity_kind, external_id)` uniqueness for every entity kind. Real sandbox data already contained historical duplicate `race` rows for the same external id, so Mongo index creation failed at startup with duplicate-key errors before the app could boot.
- Fix: narrowed that unique index in `src/adapters/mongo/external_sync_states.rs` to rows where `canonical_entity_kind = "planned_workout"` and `external_id` is a string, preserving the planned-workout explicit-link invariant without imposing a new global uniqueness requirement on older `race` history. Added Mongo regressions that assert the partial filter and that duplicate non-planned external ids still round-trip.
- Prevention: when adding a new unique index to an existing shared collection, scope the partial filter to the exact entity kind or rollout invariant that actually needs protection. Before shipping, compare the proposed uniqueness domain against plausible historical production rows so startup index builds do not fail on unrelated legacy duplicates.

### 2026-05-01 | Copilot/CodeRabbit | PR #172 calendar cleanup and explicit Intervals follow-up

- Problem: the first paired-event and calendar duplicate fix left three real review gaps. Calendar refresh recreated heuristic links with `matched_at_epoch_seconds = 0`, cleanup still loaded full planned-workout candidates across all history just to compute live ids, and the new Intervals explicit relink path still relied on an unscoped provider+external_id lookup that could collide with non-planned sync rows.
- Fix: changed calendar refresh to stamp heuristic relinks from the completed workout day, added a dedicated `list_visible_planned_workout_ids_by_user_id(...)` port with a Mongo implementation that reads only cleanup ids/dates plus sync keys instead of full workout payloads, and introduced `find_planned_workout_by_provider_and_external_id(...)` with a Mongo filter and unique index scoped by `canonical_entity_kind = planned_workout`. Added focused domain and Mongo regressions, including the case where a hidden imported duplicate id must be cleared before merging to the projected plan.
- Prevention: when a review points at a heavy cleanup path, verify that the replacement actually narrows the loaded document shape instead of just moving the same full scan behind a new method name. When adding provider-level explicit linking on a shared sync-state store, scope the lookup to the intended canonical entity kind in both the repository API and the backing index, then add a regression with a conflicting non-target entity kind.

### 2026-05-01 | Copilot | PR #172 follow-up on defaults and malformed heuristic timestamps

- Problem: after the first review-fix batch, `find_planned_workout_by_provider_and_external_id(...)` still had a trait default that silently fell back to the generic provider/external-id lookup, so any future repository that forgot to override it would lose the planned-workout scoping guarantee. Separately, `heuristic_link_timestamp(...)` still fell back to `0` for malformed `start_date_local`, reintroducing the 1970 sentinel through a different path. The repo also still exposed a `sandbox:docker` script pointing at a local ignored compose file, which makes fresh checkouts misleading.
- Fix: made the planned-workout-scoped lookup a required trait method, changed heuristic timestamp resolution to return `Option<i64>` and skip heuristic link-row creation when the completed date is malformed, added a regression for that malformed-date relink case, and removed the dead `sandbox:docker` package script.
- Prevention: when a review-driven API exists specifically to enforce a stronger invariant, do not leave a weaker default implementation behind in the shared trait. And when removing a repo-local file in favor of local ignored setup, grep package scripts/docs for stale references so fresh checkouts do not inherit a broken command.

## Entries

### 2026-05-01 | user | Intervals paired_event_id import follow-up

- Problem: after adding explicit Intervals planned-to-completed linking by `paired_event_id`, I changed shared backend shapes without sweeping every dependent test seam. The branch stayed red on missing `ExternalSyncStateRepository::find_by_provider_and_external_id(...)` implementations, missing `Activity.paired_event_id` fixture fields, and a new import helper that tripped clippy's argument-count limit.
- Fix: added the missing sync-state lookup method to every affected in-memory/test repository, added the lenient `paired_event_id` DTO deserializer, filled `intervals_paired_event_id: None` into older completed-import fixtures, updated remaining `Activity` fixture/merge constructors to preserve `paired_event_id`, and packed completed-workout planned-link lookup inputs into a small struct so `cargo clippy -D warnings` stays green.
- Prevention: whenever a shared trait or domain model gains a new method/field, immediately grep all impls, merge helpers, and test fixtures before chasing behavior bugs. Re-run clippy early after the shape change so helper-argument growth and test-double fallout are caught before broader verification.

### 2026-05-01 | user | sandbox docker workflow

- Problem: I added the sandbox runtime as a giant inline `bun -e` script in `package.json`, which buried Docker build/run configuration inside shell-escaped code instead of using the repo's existing compose-based Docker workflow.
- Fix: replaced the inline script with a dedicated `docker-compose-sandbox.yml` and changed `sandbox:docker` to run `docker compose -f docker-compose-sandbox.yml up --build`.
- Prevention: when a local runtime needs the same shape as the repo's existing Docker setup with only environment differences, add a dedicated compose file instead of embedding `docker build` and `docker run` orchestration inside `package.json`.

### 2026-05-01 | user | sandbox auth usability

- Problem: the first sandbox compose still used the generic dev-auth identity, which hit the normal whitelist/pending-approval flow for a first-time mock user and made the local sandbox effectively unusable despite `DEV_AUTH_ENABLED=true`.
- Fix: pointed sandbox dev auth at the real existing sandbox user identity already present in `aiwatt.app_users`, so the same-origin dev OAuth flow now signs into the actual sandbox account and lands in the app without needing a live Google callback.
- Prevention: when wiring a sandbox against a shared real database, verify not only that mock OAuth redirects locally but also that the configured dev-auth identity maps to a real allowed user in that database. A generic fake account can still fail product-level approval gates.

### 2026-04-30 | user | calendar coach summary-style context regression

- Problem: the new calendar coach reused too much of the athlete-summary flow: it built packed context through the synthetic `athlete-summary` target, regenerated athlete summaries before replies, and surfaced a websocket system message about summary generation. That biased the prompt toward a generic overview and contradicted the intended calendar-chat behavior with no summary side effects.
- Fix: changed `src/domain/coach_conversation/service/mod.rs` to request a dedicated calendar-overview training context instead of `build_athlete_summary_context(...)`, stopped calling athlete-summary regeneration in the calendar coach path, kept the append/reply flags hard-false for summary regeneration, removed the calendar websocket `system_message("First the summary is being generated - wait a moment")`, and added focused regressions for the new training-context entry point plus the calendar-coach service path.
- Prevention: when reusing an adjacent LLM workflow, verify which parts are truly shared behavior versus product-specific side effects. For calendar- or chat-only coach surfaces, inspect the actual prompt builder entry point and websocket status messages before shipping so summary-generation hints and synthetic `athlete-summary` focus do not leak into a non-summary conversation.

### 2026-04-30 | Copilot | PR #163 calendar AI coach review follow-up

- Problem: the calendar coach preview PR still had review gaps around modal accessibility and polish: opening the dialog did not move focus into it or restore focus on close, the dialog did not trap keyboard focus, a few Polish UI strings still used English/product-review wording, and the page test suite only reset the i18n language in `afterEach`, which left avoidable order sensitivity.
- Fix: added focus capture/restore plus a `Tab` trap in `frontend/src/features/calendar/components/CalendarCoachModal.tsx`, strengthened `frontend/src/pages/CalendarPage.test.tsx` with `beforeEach` language reset and focus-behavior assertions, and finished the remaining Polish copy in `frontend/src/locales/pl/translation.json`.
- Prevention: for modal-only preview UIs, verify keyboard behavior explicitly before sending the PR: focus should enter the dialog, remain trapped within enabled controls, and return to the trigger on close. When tests depend on a global i18n singleton, reset language before each test as well as during cleanup so order-dependent failures cannot hide.

### 2026-04-30 | user | Dockerfile Cargo registry cache in container build

- Problem: after a frontend-only change, the dev container build failed in the Rust stage while downloading `strsim v0.11.1` with `no targets specified in the manifest`. The crate itself was valid; the failure came from a corrupted cached unpacked source tree under the shared BuildKit mount at `/usr/local/cargo/registry/src`.
- Fix: changed the Dockerfile cache mounts to persist only `/usr/local/cargo/registry/cache` and `/usr/local/cargo/registry/index`, leaving `registry/src` ephemeral inside each build stage so Cargo always re-extracts crate sources from the cached tarballs instead of reusing a potentially half-written unpacked directory.
- Prevention: when Docker/Podman Rust builds fail on a well-known crates.io package with an impossible manifest error, verify the same dependency builds locally before touching `Cargo.lock`. If local Cargo works, treat the failure as a corrupted container cache and avoid caching `cargo/registry/src` across builds.
### 2026-05-01 | Copilot/CodeRabbit | PR #166 alias resolution and storage-id follow-up

- Problem: PR #166 review still pointed at several real gaps after earlier batch-read fixes: alias resolution in `resolve_workout_summary_target` preferred the resolved `preferred_workout_id` over the caller's exact `workout_id`, mutation/reload paths in `read.rs`, `save.rs`, and `chat.rs` used `target.summary_workout_id` instead of `target.storage_workout_id` for existing-summary operations, Mongo batch lookup still used O(n²) scanned alias matching and `BTreeMap`-ordered fallback for missing ids, and the Wahoo sync-state migration doc had ambiguous duplicate headings.
- Fix: reordered candidate ids in `resolve_workout_summary_target` so the exact requested `workout_id` is checked first before falling back to preferred and equivalent aliases; changed all existing-summary mutations and reloads to use `target.storage_workout_id` in `use_cases/read.rs`, `use_cases/save.rs`, and `use_cases/chat.rs`; rewrote Mongo batch missing-detection to use HashSet-based O(1) lookups and replaced `BTreeMap.values().find(...)` alias fallback with candidate-order iteration via `current_lookup_ids_for_request`; disambiguated migration doc headings to `Field Cleanup Dry Run`, `Row Cleanup Dry Run`, and `Apply Row Cleanup`. Removed now-unused `matches_requested_workout_id` in the Mongo adapter.
- Prevention: when alias resolution produces both a `preferred_workout_id` and a `storage_workout_id`, every mutation path that targets an existing persisted entity must use the storage key for repository calls, and the resolver must try the exact caller-provided id first so the last alias in a rollout does not silently rebind the caller's conversation.

### 2026-05-01 | user | planned-workout Intervals sync history-backed revert

- Problem: I chased the remaining live Intervals `400 Bad Request` by switching planned-workout sync from `description` to canonical `workout_doc`, but the live failure persisted and the user pointed out that the repo history already contained a previously working Intervals push shape. Re-reading the older working calendar service at commit `4b7b545` showed planned-workout Intervals sync historically sent body-only workout text in `description`, left `workout_doc` empty, and preserved existing event metadata on updates.
- Fix: reverted the planned-workout Intervals sync path to that history-backed shape: `projected_event_sync_body(...)` now feeds `description`, `workout_doc` stays `None`, and update payloads merge projected text into the existing remote description while preserving existing `event_type`, `indoor`, and `color`. Updated the planned-workout sync regression and removed the incorrect adapter regression that had locked in the speculative `workout_doc` shape.
- Prevention: when a live provider failure persists after multiple payload tweaks, stop iterating on local theories and diff the current code against the last known working implementation in git history before making another semantic change. Prefer the repository's proven provider contract over a cleaner-looking abstraction when the upstream system disagrees.

### 2026-04-30 | user | planned-workout Intervals sync workout_doc grammar follow-up

- Problem: after fixing `workout_doc` placement and `start_date_local`, planned-workout Intervals sync still built the outbound `workout_doc` from a stripped body-only serializer in `src/domain/calendar/service/projected.rs`. Live request previews showed only `- 60m 50%` for `Active Recovery`, while adjacent canonical planned-workout paths in `calendar_view` and `training_context` serialize the full workout text including the title line. That left the remaining live `400 Bad Request` most likely caused by a grammar mismatch between the sync payload and the repo's canonical planned-workout text shape.
- Fix: changed the Intervals planned-workout sync path to send the canonical serialized workout text via `projected_event_sync_body(...)` while leaving the sync-state payload-hash semantics unchanged, updated the calendar sync regression to assert the full canonical `workout_doc`, and added an adapter regression that verifies the JSON create-event payload carries canonical `workout_doc` with no fallback `description`.
- Prevention: when one outbound provider payload carries canonical workout text, reuse the same serializer already used by the repo's other canonical planned-workout paths instead of maintaining a near-duplicate body-only formatter. If live previews show a shorter-than-expected `workout_doc`, compare its byte length against the canonical serializer output before debugging upstream grammar blindly.

### 2026-04-30 | user | planned-workout Intervals sync datetime follow-up

- Problem: after moving planned-workout sync text into `workout_doc`, the Intervals create/update payload still sent `start_date_local` as a bare `YYYY-MM-DD` date while the surrounding Intervals event flows in this repo use `YYYY-MM-DDT00:00:00` for projected all-day events. Live logs confirmed the backend was still sending `start_date_local="2026-05-01"`, and Intervals kept rejecting the request with `400 Bad Request`.
- Fix: introduced a shared `projected_event_start_date_local(...)` helper for calendar planned-workout sync, used it for both create and update Intervals event payloads, and extended the planned-workout sync regression to assert the outgoing `start_date_local` shape alongside `workout_doc`.
- Prevention: when a domain flow builds provider event payloads from projected calendar days, keep the datetime shape aligned with the repo's other Intervals event writers instead of mixing bare dates and midnight datetimes. When debugging upstream `400`/`422` responses, compare live request logs against adjacent successful payload builders before assuming the remaining issue is in the body content.

### 2026-04-30 | Copilot | PR #166 perf and migration-script follow-up

- Problem: the first batch-read restoration in `list_summaries_impl(...)` still deduped lookup ids with repeated `Vec::contains`, which turned large `workoutIds=...` requests into avoidable O(n^2) list building. Separately, `load_active_operation_date_range(...)` still loaded every active projected-day document for an operation key just to compute min/max dates, and the legacy Wahoo cleanup scripts compared BSON numeric wrapper values with strict identity semantics, which could report false mismatches and skip eligible cleanup rows.
- Fix: changed workout-summary lookup id dedupe to keep insertion order while using a `HashSet` for O(1) membership, rewrote `load_active_operation_date_range(...)` to fetch only the first and last active dates via two sorted `find_one(...)` queries, and normalized numeric/BSON wrapper comparisons in both legacy Wahoo cleanup scripts before diffing migrated rows. Re-ran focused Rust regressions plus `bun run verify:scripts`.
- Prevention: after restoring a batch path, re-check helper dedupe structures for hidden O(n^2) work on unbounded request lists. When a repository only needs min/max values, do not materialize the whole matching set. For Mongo migration scripts, treat BSON numeric wrappers as value types and normalize them before equality checks.

### 2026-04-30 | Copilot | PR #166 workout summary list/read follow-up

- Problem: the previous alias fix left two review-confirmed gaps. Mongo batch lookup still did not prefetch the `intervals-activity:{id}` alias when the request arrived as `wahoo-workout:{id}`, so a summary stored under the Intervals-prefixed alias could be missed entirely before alias matching ran. Separately, `list_summaries_impl(...)` had switched from one repository batch fetch to per-id `resolve_workout_summary_target(...)` point reads, which turned completed-workout alias listing into an avoidable N-times lookup path.
- Fix: extended `current_lookup_ids_for_request(...)` to include both provider-prefixed aliases derived from the stripped activity id, restored `list_summaries_impl(...)` to resolve requested/completed-workout aliases first and then reuse a single `find_by_user_id_and_workout_ids(...)` batch fetch, and added focused Mongo plus in-memory regressions covering the missing Intervals-prefixed alias and the restored batch-read path.
- Prevention: for alias-aware batch lookup, verify that prefetch ids cover every persisted alias family before relying on later normalization. When a list/read use case already has a batch repository method, preserve that batch boundary and layer alias resolution around it instead of falling back to repeated single-item reads.

### 2026-04-30 | Copilot/CodeRabbit | PR #165 alias + projection follow-up

- Problem: the completed-workout alias batch lookup still computed missing ids before alias remapping and only normalized the stored id side, so requests like `intervals-activity:...` could miss prefetched alias hits and do unnecessary legacy fallback work. The in-memory summary repository fallback also searched across all users, which could hide tenant-scoping bugs in tests. Separately, the in-memory training-plan projection repo derived superseded refresh ranges from unsorted same-operation dates, so replay tests could compute the wrong min/max window depending on insertion order.
- Fix: changed Mongo batch lookup to compute missing ids after alias-aware matching and normalize the requested id as well as the stored id, restricted the in-memory summary alias fallback to the requested user and aligned its normalization, and switched the in-memory training-plan projection repo to derive same-operation refresh bounds via min/max rather than unsorted first/last. Added focused regressions for prefetched alias requests, user-scoped alias fallback, and same-operation stale-leading-day supersedence.
- Prevention: when adding alias-aware batch lookup, verify both sides of normalization and compute fallback sets only after checking alias matches, not just exact keys. Any in-memory repo used to validate persistence semantics must preserve user scoping and ordering/min-max behavior from production rather than relying on incidental iteration order.

### 2026-04-30 | internal review loop | workout summary alias follow-up

- Problem: the first completed-workout summary alias patch still had four confirmed review gaps: missing-`source_activity_id` fallback collapsed canonical ids to stripped activity ids, saved-workout side effects used the preferred alias instead of the persisted storage key and could drift operation ids on retry, `list_summaries(...)` could return the same summary twice when both aliases were requested, and batch `find_by_user_id_and_workout_ids(...)` no longer matched equivalent completed-workout aliases needed by `training_context`.
- Fix: kept missing-`source_activity_id` targets on `completed_workout_id`, tracked the matched repository storage key inside `ResolvedWorkoutSummaryTarget`, drove saved-workout recap/plan side effects from that storage key, deduped `list_summaries(...)` by summary id after alias resolution, and taught batch summary lookup in both Mongo and in-memory test repositories to match equivalent completed-workout aliases.
- Prevention: when adding alias resolution above a repository boundary, verify three separate surfaces before review: fallback identity selection when canonical metadata is missing, side effects that derive operation keys from ids, and batch lookup/list APIs used by downstream read models. Single-item get/create coverage is not enough.

### 2026-04-29 | user | workout detail modal black screen after summary refactor

- Problem: after moving completed-workout summary loading into `WorkoutDetailModal`, the new `useCompletedWorkoutSummary(...)` call sat below `if (!selection) return null`. Opening workout details changes the modal from `selection = null` to a real selection, so React saw a different number of hooks between renders and crashed the page with a minified hook-order error.
- Fix: moved the derived detail state and `useCompletedWorkoutSummary(...)` above the early return, passed `activityId: null` when no selection exists, and added a focused frontend regression that renders the modal with `selection={null}` first and then rerenders with a completed workout.
- Prevention: whenever a modal or detail container can mount empty and later receive a selection, verify hook order across that exact transition. New hooks must stay unconditional across both renders, and the test suite should include a `null -> selected` rerender case.

### 2026-04-29 | user/Copilot | PR #162 completed workout summary review

- Problem: the completed-workout summary change put network fetching directly inside `CompletedWorkoutDetailModal`, drilled `apiBaseUrl` into a child view just for that fetch, removed the existing completed-only interval breakdown instead of adding the recap alongside it, relied only on `activity?.id` so summary loading broke when the detailed activity fetch failed but `actualWorkout.activityId` was still known, surfaced raw `HttpError` text to the UI, and used `{workout_id}` on the new completed-workout summary route even though the surrounding REST surface already names that path segment `{activity_id}`.
- Fix: extracted `useCompletedWorkoutSummary(...)` in `frontend/src/features/calendar/hooks/useCompletedWorkoutSummary.ts` and moved summary loading back to `WorkoutDetailModal`, made `CompletedWorkoutDetailModal` presentational again, restored `CompletedIntervalsSection` for activity-only completed workouts while keeping the AI summary section, resolved the summary target id from `event.actualWorkout.activityId ?? activity?.id`, mapped non-404 `HttpError` failures to the localized `calendar.workoutSummaryUnavailable` message, renamed the route/path field to `{activity_id}`, and added focused frontend regressions plus backend route verification.
- Prevention: when adding a new data fetch to a modal/detail flow, keep container components responsible for loading and keep child detail panels presentational. Before removing an existing detail section, verify the user explicitly wants replacement rather than addition, and re-check identifier source, friendly error copy, and REST path naming consistency against neighboring endpoints before sending the PR.

### 2026-04-29 | user | provider_poll_states parked sentinel crash

- Problem: parked provider poll states intentionally used `next_due_at_epoch_seconds = i64::MAX` as a disabled sentinel, but the Mongo write mapper still tried to mirror that value into BSON `next_due_at`, which exceeds the `DateTime` range and panicked the release on startup.
- Fix: taught `map_poll_state_to_document(...)` in `src/adapters/mongo/provider_poll_states.rs` to skip the readable `next_due_at` mirror when the parked sentinel is present, preserved the epoch sentinel for runtime behavior, and added a focused regression proving the parked state no longer attempts BSON datetime serialization.
- Prevention: when dual-writing readable BSON datetimes beside epoch sentinel fields, audit every sentinel value before mirroring it into `DateTime`. Values that encode "disabled" or "parked" state must stay in the epoch field only unless they are valid BSON timestamps.

### 2026-04-29 | Copilot | PR #150 readable Mongo dates follow-up

- Problem: the readable-date follow-up branch still had unresolved review gaps in Mongo timestamp migration paths and then picked up merge drift in `src/main.rs`, where old planned-workout sync repository wiring no longer matched the merged calendar/external-sync service signatures.
- Fix: kept the backfill startup path opt-in via `RUN_MONGO_READABLE_DATES_BACKFILL=true`, tightened the backfill updater to use compare-and-set filters, removed panic-based Mongo read paths in `provider_poll_states`, `task_workers`, and `tasks`, preserved dual-write BSON `*_at` lifecycle fields for task updates, fixed `main.rs` to use the current `CalendarEntryViewRefreshService`, `CalendarService`, and `ExternalImportService` wiring, and added focused regressions for missing `next_due_at` plus Mongo task `started_at` / `finished_at` datetime mirrors.
- Prevention: after merging `main` into a long-lived review branch, re-read the current constructor and builder signatures before reusing old composition-root wiring. For staged Mongo timestamp migrations, verify both read-side corrupt-data handling and write-side datetime mirrors with focused regressions instead of relying on compile success alone.

### 2026-04-29 | user | PR #158 conflict resolution

- Problem: the branch had diverged from the latest `origin/main`, so PR #158 still showed merge conflicts even though the feature branch itself was otherwise up to date. The conflict landed in `reviewers.md`, where both branches had added new top-of-file entries.
- Fix: fetched `origin/main`, merged it into `feature/planned-workout-provider-split`, resolved `reviewers.md` by preserving both branches' entries in newest-first order, kept the already auto-merged lesson updates, and verified the merged branch before pushing.
- Prevention: when a PR shows conflicts, do the real merge against the latest remote base branch and treat rolling logs like `reviewers.md` as append-only history that must preserve both sides in reverse-chronological order.

### 2026-04-27 | user | readable Mongo dates rollout PR2/PR3

- Problem: after the initial readable-date rollout reached `provider_poll_states` and `tasks`, some fields that were conceptually required still used non-optional legacy epoch document fields, so a truly `DateTime`-only migrated document could not deserialize through those adapters. The rollout also still lacked the promised idempotent backfill for pre-existing Mongo documents.
- Fix: changed the technical Mongo documents to dual-read required timestamps from either the new BSON `*_at` mirrors or legacy `*_epoch_seconds`, added focused regressions for `DateTime`-only reads in `provider_poll_states`, `task_workers`, and `tasks`, and implemented a startup backfill that idempotently populates readable BSON datetime mirrors across the PR1 and PR2 collections, including nested settings fields and array items like `messages[]` and `attempts[]`.
- Prevention: when doing a staged dual-read/dual-write timestamp migration, do not stop at adding mirror fields to writes. Re-check every required read path against a `DateTime`-only document shape, especially where the persistence struct still uses non-optional legacy epoch fields, and ship the promised backfill before calling the rollout complete.

### 2026-04-29 | user | frontend calendar mini chart workout bars

- Problem: I kept oscillating between sampling, equal-width bars, and capped raw-duration widths in `CalendarMiniChart`, which repeatedly broke either temporal order, duration proportion, or visible height differences. I also reverted planned workout heights back to raw `%FTP`, which made zone differences too subtle in the mini chart.
- Fix: restored planned workout bar heights to use `ZONE_VISUAL_HEIGHT_PERCENT` whenever `zoneId` exists, and changed the mini-chart renderer to preserve the full bar sequence while compressing `widthUnits` with a square-root scale for display only. Updated focused frontend tests to assert the compressed mini-chart widths instead of raw durations.
- Prevention: for dense sparkline-style charts, keep canonical bar data unchanged and do any visibility compression only in the renderer. Do not fix a width-visibility issue by changing bar order or by switching the renderer to equal-width bars. When a UX requirement says planned workouts should use visual zone heights, do not revert to raw `%FTP` just to satisfy stale tests.

### 2026-04-29 | user | completed workout chart height normalization

- Problem: completed workouts with many high-power intervals looked like nearly flat same-height bars in both overview and detail because `normalizeBarHeights(...)` stretched interval-based completed data relative to that workout's local min/max instead of preserving a visible absolute power scale.
- Fix: stopped applying `normalizeBarHeights(...)` to interval-based completed workouts and matched-workout intervals, keeping their heights on the existing absolute `0..1300W -> 0..100%` mapping while leaving fallback stream/skyline paths unchanged.
- Prevention: for completed intervals that already carry meaningful absolute watt targets, avoid per-workout min/max normalization. Use local normalization only for fallback dense streams where absolute heights would otherwise become unreadable.

### 2026-04-29 | user | completed workout interval visual scale follow-up

- Problem: removing local normalization was still not enough for older completed workouts because many real interval powers cluster in a narrow watt band, so absolute `0..1300W` heights still looked visually flat in both the overview card and the detail modal.
- Fix: switched interval-based completed bars and matched-workout bars to the same zone-visual height scale used for planned workouts when zone data exists or can be derived from FTP. Kept skyline and raw stream fallbacks on their existing normalization path.
- Prevention: for compact visual charts, absolute watts are often too compressed to be legible across normal cycling ranges. If the UI goal is quick visual differentiation, prefer a zone-based visual scale for interval bars and reserve absolute scaling for detailed numeric charts.

### 2026-04-28 | CodeRabbit | PR #158 second review follow-up

- Problem: the new Wahoo retry/dedupe recovery still only searched page 1 of `list_workouts(...)`, so an older remote workout with the planned-workout marker could be missed and recreated on retry. Separately, the legacy Wahoo sync test-seeding mapper still left `external_id` empty for pending/modified records even when `wahoo_workout_id` was present, reducing fidelity versus real `ExternalSyncState` rows.
- Fix: paginated `resolve_existing_workout(...)` across Wahoo workout pages until the marker is found or the remote list is exhausted, updated the Wahoo test double to paginate the seeded list, added a regression that finds the token on page 2, and populated `external_id` from `wahoo_workout_id` in the pending/modified legacy Wahoo sync-state mapper with focused mapper regressions.
- Prevention: when a lookup becomes the dedupe path for retries or stale-id recovery, verify it searches the full upstream result set rather than one convenient page. For legacy-to-current test mappers, mirror every derived identifier field that runtime persistence would populate, not just the provider-specific side fields.

### 2026-04-28 | user | CI follow-up for planned-workout sync split

- Problem: two Rust tests still assumed pre-refactor behavior after the provider-split sync changes. The Wahoo update-path test used a fake client with no discoverable remote workout, so the new recovery flow legitimately recreated instead of updating. The `calendar_view` rebuild test still expected planned-workout sync metadata to survive purely from existing `calendar_view` rows even though rebuild now intentionally sources planned sync only from authoritative `ExternalSyncState` records and clears stale view-only planned sync.
- Fix: seeded the Wahoo test with a listed remote workout carrying the expected marker so the update path is exercised under the new lookup flow, and changed the rebuild test to wire `TestExternalSyncStateRepository` with the planned/race sync states it expects instead of relying on stale persisted view sync.
- Prevention: when a workflow gains recovery/discovery steps, re-check whether test doubles still model the real upstream preconditions for the intended branch. For `calendar_view` rebuilds, planned-workout sync expectations must come from authoritative external sync fixtures, not from previously persisted read-model rows.

### 2026-04-28 | Copilot/CodeRabbit | PR #158 planned-workout provider split review follow-up

- Problem: the provider-split branch had several lingering review gaps after earlier fixes: provider-agnostic `find_by_canonical_entities(...)` still had no matching Mongo index shape, the Wahoo uniqueness indexes did not exclude `null` values in their partial filters, the planned-workout failure banner regressed to weak/incorrect copy, indoor predicted workouts still exposed an Intervals sync action even though the backend would rewrite them as outdoor, the Wahoo sync API test no longer asserted the positive-id contract, and the external-sync import test seeding path still replaced optional Wahoo metadata with sentinel defaults instead of mirroring persisted `ExternalSyncState` rows.
- Fix: added a supporting `{ user_id, canonical_entity_kind, canonical_entity_id }` Mongo index for batch canonical-entity lookups, tightened the Wahoo partial-unique indexes to exclude `Bson::Null`, restored the indoor sync guard in the modal until backend indoor preservation exists, introduced a neutral persistent sync-failure banner copy, strengthened the Wahoo sync API test to assert the parsed positive id, and updated the legacy Wahoo-sync test seeding helper to preserve optional ids, hashes, and timestamps without `unwrap_or_default()` sentinels.
- Prevention: when review feedback points at repository performance or uniqueness invariants, compare the exact query filters against the declared index prefixes and partial-filter semantics instead of assuming a nearby provider-scoped index is sufficient. For UI sync affordances, re-check the backend payload contract before exposing new provider paths, and keep test-seeded compatibility adapters semantically identical to runtime persistence so review-driven regressions do not hide in test-only helpers.

### 2026-04-28 | user | planned-workout sync split review loop follow-up

- Problem: the provider-split planned-workout sync work left a real read-model regression where `calendar_view` refresh/rebuild paths collapsed previously modified planned workouts back to coarse `synced/pending/failed` status, so the frontend could lose the `scheduleChanged` badge after a refresh. The planned-workout modal also still showed the Wahoo-only sync-window warning for indoor workouts even though the Wahoo button was intentionally hidden there.
- Fix: moved planned-workout `modified` detection into the shared `calendar_view` planned-workout projection logic, aligned `rebuild_for_user(...)` to reuse that projection behavior for planned entries, added focused calendar-view regressions for the modified case, and gated the Wahoo sync-window warning on the same `canSyncToWahoo` condition used to render the Wahoo button.
- Prevention: when a sync-state refactor changes how planned-workout status is aggregated, verify every read-model path that materializes planned entries, not only the live domain-event path. If a UI warning belongs to one provider action, key it off the exact render condition for that action so hidden controls do not leave orphaned helper text behind.

### 2026-04-28 | user | planned-workout sync-state cutover follow-up

- Problem: the provider-specific planned-workout sync cutover left several stale test and wiring paths on the removed per-provider sync repositories, and the failure path in `src/domain/calendar/service/sync.rs` persisted failed sync state without refreshing `calendar_view`, so failed Wahoo sync badges would stay stale until a later rebuild.
- Fix: migrated runtime and tests to the shared `ExternalSyncStateRepository`, updated REST/test wiring and Mongo sync-state fixtures for the new Wahoo metadata fields, converted calendar/calendar-view tests to provider-aware `SyncPlannedWorkout` requests plus shared sync-state fixtures, and refreshed the planned-workout day after both successful and failed sync attempts so persisted failure state shows up immediately in the calendar view.
- Prevention: when replacing a provider-specific persistence seam with a shared external-sync model, grep both runtime and test code for the removed repository types and constructor arities before calling the refactor complete. For sync workflows that persist local failure state, verify the read-model refresh runs on both success and failure paths, not only on the happy path.

### 2026-04-27 | CodeRabbit | PR #157 planned workout repeat parsing follow-up

- Problem: the first canonical repeat-block fix expanded only the outer repeat header count and ignored inline step-level repeat counts inside the block, so input like `Main Set 2x` plus `- 2x30s 120%` still undercounted duration and segment count inside calendar/workout summaries.
- Fix: updated repeat-block expansion to iterate each repeated step by its own `repeat_count`, keep per-step occurrence numbering across outer block iterations, and added a regression covering nested repeat expansion inside canonical repeat blocks.
- Prevention: when adding grouped expansion logic on top of an existing flat parser, audit whether child items already carry their own multiplicity. Outer repeat support is incomplete if nested inline repeats silently collapse back to one segment per child line.

### 2026-04-27 | user | planned workout calendar duration parsing

- Problem: `parse_workout_doc(...)` treated canonical planned-workout repeat headers like `Main Set 2x` as standalone lines, so the following step lines were not expanded as a repeated block and calendar planned-workout summaries undercounted total duration.
- Fix: taught the workout parser to recognize canonical repeat-header lines without inline durations, expand the following contiguous timed steps as a repeated block when building segments, and added a regression test for the full repeated duration/segment labels.
- Prevention: when one parser consumes text emitted by another canonical serializer, add regression coverage for structural constructs like repeat headers instead of assuming a flat line-by-line parser preserves grouped semantics.

### 2026-04-27 | Copilot/CodeRabbit | PR #148 readable Mongo dates review follow-up

- Problem: the readable-Mongo-dates branch had several real review gaps: multiple Mongo read mappers (`settings`, `athlete_summary`, `external_observations`) still used `expect(...)` on required timestamps and could panic on malformed dual-format documents; `workout_summary` lock enforcement and update filters only checked legacy `saved_at_epoch_seconds`, so a DateTime-only `saved_at` mirror could leave a locked summary editable; BSON-to-epoch conversion truncated toward zero for negative millisecond values; debug output in `settings` omitted the new readable DateTime mirrors; and the branch still conflicted with `origin/main` in regenerated `graphify-out` artifacts.
- Fix: changed the affected read mappers to return repository/storage errors instead of panicking, added focused regressions for missing required timestamps, taught `workout_summary` to treat either saved timestamp representation as locked and to require both mirrors to stay null in edit filters, changed BSON timestamp conversion to use `div_euclid(1000)` with a regression, added the missing safe DateTime fields to `WahooDocument` and `CyclingDocument` debug output, kept `coach_reply_operations` epoch fields required because that collection is not legacy-backfilled, and resolved the merge conflict by regenerating `graphify-out` from the merged tree.
- Prevention: when introducing backward-compatible dual timestamp storage, audit every read-path `expect(...)`, lock predicate, and optimistic-update filter that previously relied on the legacy epoch field being the single source of truth. If a collection has no legacy data, keep required epoch fields required instead of weakening serde guarantees to `Option<i64>` just for symmetry.

### 2026-04-27 | user | Wahoo planned-workout sync invalid file

- Problem: Wahoo planned-workout sync sent `POST/PUT /v1/plans` as JSON with a raw base64 string in `plan.file`, while the Wahoo Plans API expects form-encoded fields and a file value wrapped as `data:application/json;base64,...`. That made valid generated `plan.json` payloads fail upstream with `422 Unprocessable Entity (oauth_error=Invalid file)`.
- Fix: changed the Wahoo client plan create/update paths to send form bodies with `plan[file]`, `plan[filename]`, and provider metadata exactly as documented, wrapped the base64 payload in a JSON data URI, removed the now-dead plan JSON request DTOs, and added focused client tests for both create and update plan form construction.
- Prevention: when an upstream API describes file uploads as `resource[file]` parameters, verify both the transport encoding (`form` vs `json`) and the exact file wrapper format (`data:<mime>;base64,...` vs raw base64) before assuming a nested JSON request body is acceptable.

### 2026-04-27 | Copilot/CodeRabbit | PR #152 manual calendar refresh review follow-up

- Problem: PR review found five confirmed issues in the self-service calendar refresh feature: the rebuild range still used a full-table scan with a "9999-12-31" sentinel instead of two indexed date lookups, the frontend 401-redirect test restored `window.location` without `try/finally`, the backend handler leaked raw repository error messages and user identifiers in logs, the backend error mapping did not cover the new `InvariantViolation` variant from race/credential errors, and no REST test verified user scoping on the refresh endpoint.
- Fix: replaced `list_existing_view_dates_for_user` with `find_oldest_date_by_user_id` and `find_newest_date_by_user_id` on the `CalendarEntryViewRepository` trait, implemented both in Mongo (indexed, sorted), in-memory (ports, unit tests, integration test app), and domain test repos; added `CalendarEntryViewError::InvariantViolation(String)` and mapped it to a generic `"failed to refresh calendar view"` response in the REST handler alongside `Repository` errors; extracted `pseudonymize_user_id` as `pub(crate)` and used it in the refresh handler log lines; wrapped the frontend `window.location` mock/restore in `try/finally`; added integration tests for user-scoping and invariant-violation error responses.
- Prevention: when a new endpoint returns error details, audit both response bodies and log lines for raw identifiers or provider messages before sending for review. When an error enum grows new variants, update all exhaustive match sites including REST error mapping and cross-domain `From` impls. When mocking `window.location` in frontend tests, always restore it in `finally`. When adding repository methods for range computation, prefer targeted index-friendly lookups over full-table scans with sentinel values.

### 2026-04-27 | Copilot/CodeRabbit/internal review loop | manual calendar refresh review follow-up

- Problem: the first self-service calendar refresh version still hard-capped the rebuild range at `today`, so future calendar entries already stored in `calendar_view` could survive a manual rebuild unchanged and future-only data could collapse into an inverted or incomplete range. The Settings card also surfaced raw `500` fallback text because the endpoint returned a bare status with no structured message, and the frontend POST helper still sent a dummy JSON body to a body-less refresh endpoint.
- Fix: changed manual refresh range resolution to derive `oldest..newest` from all calendar sources plus persisted `calendar_view` dates, keeping future entries inside the rebuild window; added focused regressions for future-inclusive and future-only existing-view cases; changed `POST /api/calendar/refresh` to return a JSON `{ message }` body on repository failures; taught `CalendarRefreshCard` to redirect on `AuthenticationError`; and made the shared `post(...)` helper accept body-less POST requests so the refresh call no longer sends `{}`.
- Prevention: for any manual projection rebuild, compute the full effective range from every persisted source that can contribute to the projection, including already-materialized read-model rows when they may extend beyond "today". For frontend-triggered maintenance endpoints, keep the transport contract user-friendly on failure and do not force empty JSON payloads onto body-less routes.

### 2026-04-27 | Copilot/CodeRabbit/user | PR #151 follow-up review fixes

- Problem: follow-up review on PR #151 found a few real gaps and one readability issue: power-detail authority ignored `CompletedWorkoutStream.all_null`, Wahoo FIT enrichment still attempted calendar refreshes for malformed `start_date_local`, the sparse-Wahoo calendar regression hid its intent behind mutating an originally detailed fixture, and regenerated graphify artifacts wrote `Graph Report - .` instead of the repo name.
- Fix: made completed-workout power-detail detection require `!all_null` with a regression, changed Wahoo FIT day refresh to skip malformed dates with a warning instead of calling refresh on an invalid range, introduced a `sample_completed_basic_workout()` fixture plus the inverse calendar authority regression where detailed Wahoo wins back the day, added the asymmetric prompt-dedupe fallback test, and updated the graphify rebuild script to default the project name to `AiWattCoach` before regenerating artifacts.
- Prevention: when authority rules depend on canonical detail flags, use every semantic field that already exists on the model instead of inferring from sample presence alone. For projection refresh helpers, do not turn malformed local dates into retryable side effects; short-circuit safely and log the invariant break. In tests, prefer fixtures whose names encode the behavioral distinction under review instead of mutating a richer baseline fixture into a sparse one inline.

### 2026-04-27 | user | calendar/LLM completed-workout selection and Wahoo FIT refresh

- Problem: the calendar and training-context read path still treated Wahoo as authoritative for a whole day even when the Wahoo completed workout was only a sparse summary and Intervals already had richer power details. Separately, successful Wahoo FIT enrichment updated the completed workout but did not automatically refresh `calendar_view`, so the day could stay stale until a manual rebuild.
- Fix: extracted shared completed-workout day selection that treats "detailed" as a non-empty `watts` stream, wired `AuthoritativeCompletedWorkoutRepository` to use that day-level selector, kept prompt-side dedupe limited to collapsing true duplicate logical activities, and connected `WahooFitEnrichmentService` to `CalendarEntryViewRefreshPort` so a successful enrichment refreshes that workout day automatically. Added focused regressions for authoritative day selection, training-context fallback, calendar-view refresh behavior, and scheduler wiring for the new generic refresh dependency.
- Prevention: when two read paths must agree on source authority, centralize the authority rule in the repository or shared selector instead of re-encoding provider preference in each consumer. For enrichment flows that materially change read models, verify whether the write path must trigger the corresponding projection refresh immediately after persistence.

### 2026-04-26 | user | Wahoo bootstrap poller loses all progress on rate limit

- Problem: the Wahoo completed-workouts bootstrap scanned every `/v1/workouts` page before importing anything. For accounts with long history, the poller could get many successful `200` pages and then hit `429 Too Many Requests` before the scan finished, which left `cursor = null`, `last_successful_at = null`, and zero Wahoo observations/completed workouts even though dozens of pages had already been read successfully.
- Fix: changed the Wahoo poller to persist a resumable page checkpoint in the poll cursor on partial scan failure and to import the workouts already gathered before returning the rate-limit error. Added regressions proving the poller now stores `next_page`/`newest_seen` after a later-page failure and resumes from that checkpoint on the next run instead of restarting from page 1.
- Prevention: when a provider poll loop paginates large remote histories, do not postpone all durable progress until after the full scan succeeds. If later pages can fail or rate-limit independently, persist a resumable checkpoint and keep already-read pages importable so one late `429` does not reset the entire bootstrap to zero.

### 2026-04-26 | user | Wahoo reconnect null boolean payload

- Problem: after reconnecting Wahoo, the background `completed_workouts` poll could fail on `GET /v1/workouts` with `invalid type: null, expected a boolean` because Wahoo sometimes returns explicit `null` values inside the workouts payload. The DTO still relied on `#[serde(default)]` for `workout_summary.manual`, `workout_summary.edited`, `plan_ids`, and the top-level `workouts` list, but that only tolerates missing fields, not explicit `null`.
- Fix: changed the Wahoo workouts DTO to deserialize nullable booleans and vectors through lenient helpers that map missing or `null` values to `false` / empty vectors, and added focused regressions covering the real workouts-list payload shape.
- Prevention: when a provider field is documented or observed as nullable, do not rely on `#[serde(default)]` alone for scalars or collections because it does not accept explicit `null`; use `Option<T>` or a custom deserializer and add a regression with the real payload shape.

### 2026-04-26 | CodeRabbit | PR #143 Wahoo import duration fallback

- Problem: `src/adapters/wahoo/import_mapping.rs` still fell back from `summary.duration_total_seconds` straight to `workout.minutes`, but `minutes` is the planned duration, not the actual elapsed duration. When Wahoo omitted total duration but still provided summary active duration, the canonical completed workout could record a wildly inflated elapsed time.
- Fix: changed the mapping to prefer `summary.duration_active_seconds` before the planned `workout.minutes` fallback, kept the planned-duration fallback only as a last resort with an explicit comment, and added a focused regression proving the importer now records the actual summary duration.
- Prevention: when provider payloads expose both planned and completed-workout timing fields, derive canonical elapsed duration from summary/activity result fields first and use planned duration only as a clearly documented last-resort fallback.

### 2026-04-26 | user | Wahoo workout request body logging

- Problem: po domknięciu review fixów klient Wahoo nadal logował write requesty bez body preview, więc przy debugowaniu `create_workout` / `update_workout` brakowało widoczności faktycznie wysyłanego form payloadu.
- Fix: rozszerzyłem `src/adapters/wahoo/client/logging.rs` o tryb `BodyLoggingMode::Full` dla requestów, dodałem bezpieczny preview dla form-encoded payloadów z redakcją pól wrażliwych takich jak `workout_token`, i podpiąłem ten tryb tylko pod `create_workout` / `update_workout` w `src/adapters/wahoo/client.rs`.
- Prevention: gdy użytkownik prosi o lepszą obserwowalność na adapter write path, najpierw sprawdź `docs/logging.md` i ogranicz body logging do konkretnych requestów z redakcją sekretów, zamiast rozszerzać je globalnie na cały klient.

### 2026-04-26 | user | PR #144 follow-up stale external-sync expectation

- Problem: after promoting `wahoo_workout_token` matches to `PlannedCompletedWorkoutLinkMatchSource::Explicit`, I updated the production code and nearby review discussion but missed the existing domain regression `import_completed_workout_falls_back_to_wahoo_workout_token_when_plan_id_missing`, which still expected `Token` and failed the full Rust test run.
- Fix: updated the external-sync test to assert `Explicit` for Wahoo workout-token-backed planned-workout links, matching the intended ranking behavior already implemented in `src/domain/external_sync/import/mod.rs`.
- Prevention: whenever a review-driven change alters enum/ranking semantics, grep all focused tests for the old enum variant and rerun the full touched test module before calling the patch complete.

### 2026-04-26 | Copilot/CodeRabbit | PR #144 Wahoo planned-workout review follow-up

- Problem: the PR review surfaced several real gaps around Wahoo planned-workout sync and linking: Wahoo plan mapping could swallow repeat blocks past text separators, planned-workout sync re-queried Wahoo plans unnecessarily and lacked stable REST error codes for frontend handling, external-sync linking treated `wahoo_workout_token` matches as weaker token matches instead of explicit Wahoo identities, Wahoo plan lookup silently accepted duplicate `external_id` rows, and one attempted REST test covered the wrong layer for the not-connected path.
- Fix: made Wahoo repeat parsing stop at text delimiters, simplified existing-plan resolution to avoid the duplicate lookup, added stable calendar sync error codes for invalid date / sync window / missing FTP / Wahoo-not-connected responses and updated the modal to prefer `error.body.code` with message fallback, mapped `wahoo_workout_token` link resolution to `Explicit`, rejected duplicate Wahoo plans with a focused regression, wired planned-workout Wahoo sync records into `ExternalImportService`, moved Wahoo-not-connected coverage to a domain test instead of the miswired REST harness, and kept the frontend sync-window helper aligned with the backend's current UTC-based contract.
- Prevention: when review feedback touches transport errors, verify that the test harness actually wires the dependency path being asserted before adding or keeping a REST integration case. For provider-owned identifiers, prefer stable machine-readable codes and explicit match-source semantics instead of UI string matching or generic token classification. If a provider lookup is expected to be unique by `external_id`, treat duplicate upstream rows as an error and add a regression immediately.

### 2026-04-26 | Copilot/CodeRabbit | PR #143 Wahoo second review pass

- Problem: the follow-up review still pointed at a few real gaps after the first cleanup: Wahoo motorcycling was still normalized as `Ride`, partial Wahoo batch imports still skipped training-load recompute when `imports.import(...)` failed after earlier successes, planned-workout authoritative reads could miss cross-boundary completed workouts and still did per-workout link lookups, and the Wahoo FIT enrichment scheduler still trusted payload `user_id` instead of the scheduled task tenant key.
- Fix: removed `BikingMotocycling` from Wahoo `Ride` classification and added a regression, added the same partial recompute fallback for Wahoo import failures with a focused test, expanded the completed-workout date-range window by one day on planned-workout visibility reads and bulk-loaded planned-completed links through a new repository method to eliminate the N+1 path, and changed the Wahoo FIT enrichment task payload/handler to use `task.user_id` as the source of truth with a scheduler test. While touching the same area, I also made `decode_json` synchronous, documented the same-source assumption in training-context prompt dedupe with extra tests, removed the unused FIT-file repository generic from the queue-side scheduler wrapper, and collapsed the immediate downloaded+stored FIT-file upserts into one persisted stored checkpoint while preserving the original download timestamp field semantics.
- Prevention: when provider-specific enums distinguish human-powered and motorized activities, re-check both `activity_type` and `trainer` mappings together. Any batch import loop that can persist a prefix of records must run the same partial recompute or recovery path on every later failure edge, not only on post-import enrichment steps. For authoritative visibility filters, avoid date windows that exactly match one source if linked records can drift across boundaries, and batch bridge/link lookups before adding per-item async checks on hot read paths. For scheduler tasks, always treat the persisted `ScheduledTask.user_id` as the tenant source of truth rather than duplicating user scope inside untrusted payload JSON.

### 2026-04-26 | Copilot/CodeRabbit | PR #143 Wahoo review follow-up

- Problem: the Wahoo-first branch still had several review-confirmed gaps: indoor Wahoo rides were classified with `BikingMotocycling` instead of `BikingIndoor`, the completed-workout poller stopped pagination on the first stale `updated_at` even though the API is sorted by `starts` and could therefore skip recently edited older workouts, parse failures in FIT enrichment were retried even when stored bytes made them deterministic, persisted legacy Intervals calendar poll states were not being parked after the completed-workouts-only transition, and training-context prompt dedupe used lexicographic completed-workout-id ordering so `...:9` could beat `...:10`.
- Fix: switched Wahoo trainer detection to `BikingIndoor`, changed the Wahoo poller to continue scanning later pages while still filtering imported workouts by the `updated_at` watermark and added regressions for both the realistic ordering case and a later-page edited workout, marked `WahooFitEnrichmentError::Parse` as non-retryable with a focused test, parked existing Intervals calendar poll states both in settings-update sync and runtime reconciliation paths, tightened the auth-handler `NotFound` mapping into an explicit branch with rationale, added minimal FIT download URL validation, and changed training-context dedupe to prefer numeric completed-workout ids before falling back to timestamp/string comparison.
- Prevention: when a watermark key does not match the upstream API sort order, do not use it for early pagination termination; either scan all pages or rebase the cursor to the actual sort key. When deprecating or sidelining a poll stream, explicitly park any already-persisted state in both user-settings sync paths and startup reconciliation. When tie-breaking provider ids with numeric suffixes, parse and compare the numeric part instead of relying on lexicographic string order.

### 2026-04-25 | user | intervals calendar poll cursor regression

- Problem: the simplified `advance_calendar_cursor(...)` helper in `src/config/provider_polling/mod.rs` stopped looking at whether Intervals actually returned any calendar events. On incremental polls with an existing cursor and new events, it kept the old cursor instead of advancing to the end of the current window, so the service could keep rereading the same calendar range forever.
- Fix: restored event-aware cursor advancement in `poll_intervals_calendar_stream(...)` so any non-empty event page advances the cursor to `range.newest`, while empty responses still preserve the existing cursor; added a regression test for `state.cursor != None` plus returned events.
- Prevention: when simplifying polling cursor helpers, re-check the state-transition contract for both "new data arrived" and "no new data" paths before removing input parameters, and add an explicit regression for incremental sync with an existing cursor.

### 2026-04-25 | user | Wahoo poll cursor stall without workout summaries

- Problem: `src/config/provider_polling/mod.rs` advanced the Wahoo completed-workout cursor only while iterating `workouts_to_import`, so a page of workouts newer than the watermark but missing `workout_summary` produced no imports and left the cursor unchanged. The next poll could fetch the same page again forever.
- Fix: track the newest seen Wahoo cursor across all workouts above the watermark, not just the subset that becomes import commands, and added a regression test for a newer workout with `workout_summary = None`.
- Prevention: if polling filters upstream items before import, keep cursor advancement based on all consumed source records unless the product explicitly wants to revisit skipped records on every poll.

### 2026-04-25 | user | planned-workout authoritative test fixture regression

- Problem: after the Wahoo-first authoritative-read changes, `src/domain/planned_workouts/authoritative.rs` still had a local test fixture that passed `planned_workout_id` into the wrong `CompletedWorkout::new(...)` argument slot. The test intended to prove hiding a planned workout when an authoritative completed workout linked to it, but the fixture actually left `planned_workout_id = None` and placed the planned id in the `name` field instead, so CI failed with a false-negative regression.
- Fix: corrected the fixture argument order so the constructed completed workout really carries `planned_workout_id`, then reran the focused planned-workout authoritative test module.
- Prevention: when a constructor has many same-typed positional arguments, verify the touched test fixtures against the canonical constructor signature after refactors or new wrappers. For link-driven behavior, assert the linking field itself in the fixture path before trusting the final visibility assertion.

### 2026-04-25 | user | Wahoo review follow-up and polling regression cleanup

- Problem: the follow-up Wahoo review patch still had a compile-risky delegation cleanup in `src/adapters/wahoo/adapter.rs` and the in-progress `ProviderPollingService` edits had accidentally dropped Intervals calendar event imports while wiring the new Wahoo completed-workout path, which would have advanced the calendar cursor without importing planned workouts, races, or special days.
- Fix: kept the Wahoo adapter delegation cleanup but moved the shared `delegate!` macro into a valid scope for the impls, restored the Intervals calendar import loop with the existing `map_event_to_import_command(...)` behavior, and updated the calendar polling regression to assert import plus cursor advancement instead of the accidental no-import behavior.
- Prevention: when touching shared polling code for one provider, reread the full existing code path for neighboring streams before sending the patch; for review cleanups that introduce macros, compile-check their definition scope immediately so a style refactor does not turn into a build break or a behavior regression.

### 2026-04-25 | user | OpenCode Graphify plugin activation

- Problem: the repo already had `graphify-out/` artifacts and a repo-local OpenCode plugin path in `opencode.json`, but the plugin file exported only `GraphifyPlugin` and relied on prepending `echo` to `bash` commands. That made the integration brittle and easy to miss in normal search-driven sessions that start with `glob`, `grep`, or `read` instead of `bash`, and a plain `.js` module shape would still depend on local Node module-mode heuristics.
- Fix: rewrote the repo-local Graphify plugin to export an OpenCode `server` module with `id = "graphify"`, moved the reminder into `experimental.chat.system.transform`, pointed it at `graphify-out/GRAPH_REPORT.md`, `graphify-out/wiki/index.md`, and `./scripts/rebuild_graphify.sh`, and renamed the plugin file to `.mjs` while updating `opencode.json` so the repo-tracked plugin stays self-contained without relying on ignored local package metadata.
- Prevention: for repo-local OpenCode plugins, verify the module export shape against the installed plugin API and make the tracked file format self-describing (`.mjs` or committed package metadata) instead of relying on local shell mutations or ignored Node module settings.

### 2026-04-24 | CodeRabbit | PR #115 training plan scheduler retry follow-up

- Problem: `training_plan.generate_for_saved_workout` still used `RetryStrategy::Never` even though the wrapped workflow is deduped by durable `operation_key` state and can replay persisted output, so retryable scheduler failures like handler panics became terminal instead of rerunning once the pending operation was reclaimable.
- Fix: shared the training-plan stale-pending timeout constant, changed the scheduled task to `RetryStrategy::Fixed { max_attempts: 3, delay_seconds: 300 }` aligned to that reclaim window, and added a panic-once scheduler regression that proves the task enters `RetryScheduled` and completes on the second attempt.
- Prevention: when a scheduler task wraps a durable `claim_pending` workflow, align automatic retry delay with the same stale/reclaim window; otherwise a retry can re-enter too early, hit `already in progress`, and collapse a recoverable scheduler failure into a terminal error.

### 2026-04-24 | Copilot/CodeRabbit | PR #115 unresolved scheduler review follow-up

- Problem: PR #115 still had unresolved scheduler review gaps after the earlier merge pass: `AbortOnDropHandle::join()` consumed its inner `JoinHandle`, so dropping the wrapper during cancellation could no longer abort the child task; the training-plan task runner still swallowed completed-checkpoint serialization failures with `.ok()`; the maintenance loop rebuilt a fresh `TaskSchedulerService`, which split in-memory task waiters from the shared worker/service instance; Mongo task writes still accepted invalid retry strategies even though reads rejected them; and the existing regressions did not prove clone-shared timeout notifications or worker shutdown abort behavior.
- Fix: changed `AbortOnDropHandle::join()` to await through `as_mut()`, converted completed training-plan checkpoint serialization failure into explicit non-retryable task failure, reused `shared_task_scheduler.clone()` for the maintenance loop wiring, validated retry strategies in `map_task_to_document(...)`, documented the `was_regenerated` deduplicated-wait semantics and extracted athlete-summary retry constants, cleared the deterministic frontend temp fixture root on init, and added focused regressions for timeout-waiter notifications across scheduler clones plus worker shutdown aborting an in-flight handler.
- Prevention: if an abort-on-drop wrapper also exposes `join()`, never move the inner handle out before awaiting it; whenever scheduler behavior depends on in-memory waiters or watch channels, all producers and waiters must share the same service instance or clones of it; and if persistence reads validate scheduler invariants, enforce the same invariants at the write boundary so poison rows cannot be stored in the first place.

### 2026-04-24 | user | auth test fixture helper follow-up

- Problem: after the latest merge, `tests/auth_rest/shared.rs` still called a removed `keep_frontend_fixture(...)` helper in the Wahoo auth test app builder, so `cargo clippy --all-targets --all-features -- -D warnings` failed while compiling the `auth_rest` test target.
- Fix: switched the Wahoo auth test helper to reuse the existing `shared_frontend_fixture()` path just like the neighboring auth test builders and reran clippy.
- Prevention: when a test fixture strategy is refactored to a shared helper, grep for both the new helper and any removed helper names to catch stale call sites in adjacent test builders before pushing.

### 2026-04-24 | user | settings DTO signature follow-up

- Problem: after extending `map_settings_to_dto(...)` with the new `wahoo_available` argument, I updated the runtime handler call sites but missed the unit test in `src/adapters/rest/settings/mapping.rs`, so CI failed in `cargo clippy --all-targets --all-features -- -D warnings` during the lib test build.
- Fix: updated the remaining unit-test call site to pass the explicit Wahoo availability flag and reran clippy on the full workspace.
- Prevention: whenever a helper signature changes, grep all call sites including `#[cfg(test)]` modules in the defining file before treating the refactor as complete; clippy on `--all-targets` builds tests too.

### 2026-04-24 | user | PR conflict verification

- Problem: I treated the branch as conflict-free after syncing it with `origin/feature/task-scheduler-core-pr1`, but did not verify it against the latest `origin/main`. The base branch had advanced, so the open PR still showed unresolved merge conflicts.
- Fix: fetched the latest `origin/main`, reproduced the merge locally, resolved the new conflicts in `src/main.rs`, `reviewers.md`, and `tasks/lessons.md`, and regenerated `graphify-out` from the merged tree.
- Prevention: when resolving PR conflicts, always fetch the current base branch ref and test the merge against that exact remote ref immediately before calling the PR conflict-free. A clean worktree or synced head branch is not sufficient if the base branch moved.

### 2026-04-24 | user | Wahoo OAuth callback route alignment

- Problem: the code still exposed the Wahoo OAuth callback on `/api/auth/wahoo/callback`, while the deployed callback configured for the app was `https://sandbox.wattly.pl/api/wahoo/callback`. That mismatch would let the OAuth start succeed but cause the provider redirect to miss the real backend route unless the env used a legacy path.
- Fix: changed the backend callback route to `/api/wahoo/callback`, updated the dev Wahoo OAuth client callback URL, updated `.env.example`, and aligned the auth/settings tests with the new callback path while leaving the connect-start endpoint unchanged.
- Prevention: when an OAuth integration uses separate start and callback endpoints, verify the deployed provider callback against the real router path before shipping; do not assume the callback should stay under the same URL prefix as the start endpoint.

### 2026-04-24 | CodeRabbit/Copilot | PR #141 Wahoo OAuth review follow-up

- Problem: the first Wahoo OAuth connect version duplicated security-sensitive `returnTo` sanitization, used a misleading `NotConfigured` error for missing per-user Wahoo credentials, built the live reqwest client with `.expect(...)`, leaked Wahoo tokens through Mongo `Debug`, discarded all token-endpoint error detail, and accepted callback state consumption without binding it to the authenticated app user.
- Fix: extracted shared `returnTo` sanitization into `src/domain/return_to.rs` and loosened it to allow timestamp-style query parameters, renamed the per-user credential error to `NotConnected`, propagated reqwest client build errors from `main`, redacted `WahooDocument` token fields in `Debug`, summarized Wahoo OAuth error payloads via parsed `error`/`error_description` or a size/hash fallback, and changed the callback flow to require the authenticated user and consume connect state scoped to that user.
- Prevention: when adding another OAuth-style integration, reuse shared redirect sanitization, keep server-config vs per-user credential errors distinct, never use panic-style startup wiring for HTTP clients, redact adapter persistence models as well as domain models, preserve bounded upstream error detail, and make callback state consumption explicitly user-bound if the browser callback returns to an authenticated app session.

### 2026-04-24 | user | Wahoo OAuth endpoint/scope configuration

- Problem: the first Wahoo OAuth client version kept authorize URL, token URL, and scope as hard-coded adapter constants, so the review request to make them env-configurable with defaults was not addressed through the repo's normal config path.
- Fix: added optional `WAHOO_OAUTH_AUTHORIZE_URL`, `WAHOO_OAUTH_TOKEN_URL`, and `WAHOO_OAUTH_SCOPE` settings with Wahoo defaults in centralized settings parsing, wired those values into `WahooOAuthClient`, updated `.env.example`, and added focused settings tests for both default and override behavior.
- Prevention: when a review asks for env-driven behavior, first check whether the repo already has a startup settings seam and implement the override there instead of adding ad hoc environment reads in leaf adapters.

### 2026-04-23 | user/CodeRabbit | PR #115 merge and review follow-up

- Problem: PR #115 had diverged from `main` and still carried real review gaps after earlier follow-up passes: `heartbeat_worker` still raced with active-task updates because `set_worker_state` updated the cache after the upsert, panicking task handlers were logged but not converted into persisted scheduler failure state, athlete-summary task completion silently downgraded checkpoint serialization errors to `Completed { checkpoint: None }`, recovery tests did not prove repository-write idempotency, and `tasks/lessons.md` contradicted itself about `OnceLock<mongodb::Client>` reuse across separate `#[tokio::test]` runtimes.
- Fix: merged the branch with `main` while preserving both repo verification script flows, changed `set_worker_state` to hold the worker-state lock across persistence with rollback on failure and added heartbeat-specific regressions, converted panicking task handlers into retryable failed task outcomes before releasing worker activity and updated the panic regression, changed athlete-summary completion to fail explicitly when checkpoint serialization fails, added repository idempotency assertions to the workout-summary recovery tests, and narrowed the lessons guidance so `OnceLock` stays recommended only for runtime-safe per-binary fixtures.
- Prevention: after review feedback claims a path is fixed, reread the current code and verify the full behavior instead of assuming the earlier patch was enough; any full-snapshot worker heartbeat must share the same lock/persist discipline as incremental worker-state updates; never swallow persisted-checkpoint serialization errors with `.ok()` when downstream result handlers require that checkpoint contract; and recovery tests must assert the absence of duplicate writes, not only the final returned object.

### 2026-04-23 | Copilot | PR #138 Intervals Strava 422 logging follow-up

- Problem: the first version of the Intervals Strava-422 classifier decoded the same response body to UTF-8 twice inside `map_error_response_from_logged_response(...)` and allocated a temporary `String` just to compare the parsed `error` field with a static message.
- Fix: reused one decoded `Option<&str>` for both the known-422 classifier and the hashed log-summary path, and changed the parsed JSON `error` extraction to stay borrowed as `&str` so the comparison avoids an unnecessary allocation.
- Prevention: when a review fix adds lightweight response classification, reread the hot-path helper for duplicate decoding/parsing work and for avoidable temporary allocations before sending it back for review.

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

### 2026-04-22 | CodeRabbit/Copilot | PR #128 release workflow follow-up

- Problem: the first registry-release version left version-resolution logic embedded inline in GitHub Actions without tests, pushed release tags before image publication could fail, kept cache permissions too narrow for `rust-cache` and Buildx `type=gha`, and let the fallback publish script depend on the caller's current working directory.
- Fix: extracted release version resolution into `scripts/resolve-release-version.mjs` with unit tests, refactored the fallback publish helper into testable functions with unit tests and repo-root-based Docker context, moved git tag creation/push to after the image publish step succeeds, and added `actions: write` where the workflow uses GitHub Actions cache APIs.
- Prevention: when a workflow introduces custom versioning or release orchestration, move the logic into a testable script instead of inline bash; if a release tag is meant to imply a deployable artifact, publish the artifact first and push the tag only after success; any workflow using `rust-cache` or Buildx `type=gha` must keep `actions` token permissions explicit; CLI helpers that shell out should anchor filesystem context to known paths instead of assuming the caller's cwd.

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

- Problem: the dedicated `workout_summary` task runner embedded the whole claim, idle wait, task heartbeat, completion, and failure persistence loop inside feature code, which made the critical scheduler flow difficult to reason about and tied generic worker behavior to one LLM-specific use case.
- Fix: extracted the shared worker loop into `src/domain/task_scheduler/runner.rs` with a small generic `TaskRunnerHandler` contract and `TaskRunOutcome`, then reduced the workout summary runner to payload parsing and coach-reply-specific success/error mapping only.
- Prevention: when adding another scheduled workflow, first ask whether the logic is generic worker orchestration or feature-specific task handling; keep claim/lease/heartbeat/complete/fail mechanics in `task_scheduler`, and let feature runners provide only payload parsing plus domain outcome mapping.

### 2026-04-20 | user | scheduler result waiting must stay generic

- Problem: `SchedulerBackedWorkoutSummaryService` still had its own task-status polling loop for waiting on completed/failed/timed-out results, so the scheduler orchestration was split between `task_scheduler` and feature code.
- Fix: added generic `ResultTaskHandler`, `enqueue_result_task(...)`, `wait_for_result_task(...)`, and `enqueue_no_result_task(...)` to `src/domain/task_scheduler/service.rs`, then rewired the workout summary wrapper to provide only checkpoint/error parsing and final result hydration.
- Prevention: for background workflows that return a caller-visible result, keep enqueue/retry/poll/result orchestration inside `task_scheduler`; feature wrappers may build the task and map terminal scheduler state into domain output, but must not own custom polling loops.

### 2026-04-20 | user | single scheduler worker loop and smaller service methods

- Problem: the scheduler still had a per-feature worker spawn shape and `TaskSchedulerService` accumulated large orchestration methods that were difficult to review; the result path also still looked like a custom loop instead of a generic scheduler-owned mechanism.
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
