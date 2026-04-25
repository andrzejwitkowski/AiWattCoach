# Wahoo-First Completed Workout Implementation Plan

**Goal:** Implement a two-phase Wahoo-first completed-workout integration where Wahoo workout summaries become the authoritative completed-workout source per day, external providers import completed workouts only, and Wahoo FIT enrichment runs as a durable background workflow linked to canonical completed-workout ids.

**Scope:**
- phase 1: Wahoo completed-workout polling, canonical summary import, authoritative completed-workout filtering, planned-workout hiding, and disabling external calendar imports
- phase 2: durable Wahoo FIT download, raw FIT persistence, parse, and canonical workout enrichment through the shared task scheduler

**Non-goals:**
- replacing the existing Wahoo OAuth connect flow
- introducing Wahoo webhooks in this change
- redesigning the whole provider-polling architecture if a small Wahoo extension is sufficient
- importing planned workouts, races, or special days from Wahoo or Intervals
- speculative cross-provider merge logic beyond the current canonical import path

## Business Rules

- Wahoo is authoritative per day for completed workouts.
- If any day has at least one Wahoo completed workout, all non-Wahoo completed workouts on that day are hidden from business reads.
- Hidden non-Wahoo completed workouts must resolve as missing on direct lookup and return `None` or `404` through reader paths.
- External providers are completed-workout sources only.
- Planned workouts, races, and special days come only from this app.
- Planned workouts linked to an authoritative completed workout must disappear from business reads.
- Phase 1 uses Wahoo summary data only. Full stream and interval enrichment belongs to phase 2.
- Persist local durable state before external side effects in every new polling or FIT-enrichment workflow.

## Implementation Shape

- Prefer a parallel Wahoo bootstrap path over a broad provider-bootstrap refactor.
- Extend `ProviderPollingService` with a Wahoo completed-workout branch instead of rewriting the whole service into a generic provider framework first.
- Keep Wahoo provider DTOs inside `src/adapters/wahoo/**` and map them to canonical internal models at the adapter boundary.
- Use canonical completed-workout ids shaped as `wahoo-workout:<remote_id>`.
- Store the raw Wahoo workout id in `CompletedWorkout.source_activity_id` so existing calendar and completed-workout detail navigation can keep resolving by activity id.
- Mark phase-1 Wahoo imports with a user-facing `details_unavailable_reason` such as `"Detailed Wahoo workout data is still being processed. Please check back soon."`.
- Implement authoritative business filtering in shared read wrappers, not only in `calendar_view`, because `training_load`, `training_context`, `workout_summary`, and other readers also read canonical repositories directly.
- Add `wahoo.updated_at_epoch_seconds` to settings so Wahoo poll reset logic does not get coupled to unrelated settings edits.
- Prefer external-import metadata to distinguish legacy externally imported entities from app-owned entities. Do not treat `planned_workout_syncs` as proof of external ownership because local push uses that path.

## Task 1: Extend The Wahoo Boundary For Workout Summary Reads

**Files:**
- `src/domain/wahoo/ports.rs`
- `src/domain/wahoo/model.rs`
- `src/domain/wahoo/service.rs`
- `src/adapters/wahoo/client.rs`
- `src/adapters/wahoo/dto.rs`
- `src/adapters/wahoo/adapter.rs`
- `src/adapters/wahoo/dev_client.rs`

**Work:**
- Keep the existing OAuth exchange and refresh flow intact.
- Add Wahoo data methods for:
  - list workouts
  - get workout
  - get workout summary
- Keep provider DTOs in the adapter layer and expose typed adapter-facing methods that return mapped internal values.
- Reuse `WahooService::ensure_token()` so outbound Wahoo reads always go through the existing token lifecycle.
- Keep trace propagation and existing outbound logging patterns.

**Done when:**
- the Wahoo adapter can fetch workout list and workout-summary data without changing the existing connect flow

## Task 2: Add Wahoo Poll Bootstrap And Reset State

**Files:**
- `src/main_runtime.rs`
- `src/adapters/mongo/settings.rs`
- `src/domain/settings/model.rs`
- `src/main.rs`

**Work:**
- Add `updated_at_epoch_seconds` to `WahooConfig` and the Mongo settings document.
- Set that timestamp when Wahoo connect state changes in a way that should reset polling.
- Add a Wahoo bootstrap query alongside the existing Intervals bootstrap query.
- Add `reconcile_wahoo_poll_states(...)` next to `reconcile_intervals_poll_states(...)`.
- Bootstrap only `ExternalProvider::Wahoo` plus `ProviderPollStream::CompletedWorkouts`.
- Keep disabled Wahoo polling parked with `next_due_at_epoch_seconds = i64::MAX` like the existing Intervals bootstrap path.

**Done when:**
- users with active Wahoo connectivity get a durable completed-workout poll state without resetting on unrelated settings edits

## Task 3: Add Wahoo Completed-Workout Polling

**Files:**
- `src/config/provider_polling/mod.rs`
- `src/main.rs`
- create a focused Wahoo import-mapping helper under `src/adapters/wahoo/`

**Work:**
- Extend `ProviderPollingService` with the Wahoo dependencies it needs.
- Branch `poll_state(...)` on `ExternalProvider::Wahoo` and support `CompletedWorkouts` only.
- Keep the current durability rule: persist `mark_attempted(...)` before any Wahoo HTTP call.
- Use a 3-hour success interval for Wahoo completed-workout polling.
- Keep failure backoff behavior aligned with the existing provider polling flow.
- Do not assume Wahoo supports the same date-range polling model as Intervals.
- Add Wahoo-specific cursor serialization instead of forcing Wahoo into the current date-only cursor parser if the Wahoo API requires page or watermark state.
- Refresh training load after successful imports using the earliest authoritative imported workout date.

**Done when:**
- due Wahoo completed-workout poll states are processed durably and independently of the Intervals calendar stream

## Task 4: Map Wahoo Summary Payloads Into Canonical Completed Workouts

**Files:**
- create a focused Wahoo mapping module under `src/adapters/wahoo/`
- `src/domain/external_sync/import/mod.rs`
- `src/domain/completed_workouts/model.rs` only if a small canonical field addition is genuinely required

**Work:**
- Map Wahoo summary payloads into canonical `CompletedWorkout` records.
- Use canonical ids shaped as `wahoo-workout:<remote_id>`.
- Set `source_activity_id` to the raw Wahoo workout id.
- Carry summary-level fields such as duration, distance, average power, cadence, normalized power, and TSS when the Wahoo summary exposes them.
- Store any raw file-download pointer only where it is needed for phase 2; do not leak provider DTOs into canonical models.
- Set `details_unavailable_reason` to the same user-facing pending-processing message until phase-2 enrichment succeeds.
- Let `ExternalImportService` keep owning canonical persistence, observations, sync-state updates, and best-effort calendar refresh.

**Done when:**
- Wahoo summary imports create canonical completed workouts that can be read by the existing completed-workout and calendar flows

## Task 5: Add Authoritative Completed-Workout Reads

**Files:**
- create `src/domain/completed_workouts/authoritative.rs`
- `src/domain/completed_workouts/mod.rs`
- `src/domain/completed_workouts/service.rs`
- `src/adapters/workout_summary_latest_activity.rs`
- `src/adapters/workout_summary_completed_target.rs`

**Work:**
- Introduce a wrapper that composes the canonical completed-workout repository with external-sync metadata and implements authoritative read behavior.
- Prefer a wrapper that still implements `CompletedWorkoutRepository` so existing reader wiring can be switched with a small diff.
- On list and range reads:
  - load canonical completed workouts
  - identify which canonical workouts are Wahoo-owned
  - if a date contains any Wahoo workout, drop non-Wahoo workouts from that date
- On `find_latest_by_user_id(...)`, return the latest authoritative workout rather than the latest raw canonical row.
- On direct lookup by source activity id or canonical id, return `None` if the target workout is hidden by the Wahoo-first rule.
- Generalize current Intervals-only helper assumptions such as `canonical_completed_workout_id(...)` and `intervals-activity:` stripping so Wahoo canonical ids and source ids remain first-class.

**Done when:**
- every completed-workout business read can enforce Wahoo-first visibility rules consistently

## Task 6: Add Authoritative Planned-Workout Reads

**Files:**
- create `src/domain/planned_workouts/authoritative.rs`
- `src/domain/planned_workouts/mod.rs`
- `src/domain/calendar_view/refresh.rs`
- `src/domain/training_context/service/mod.rs`
- `src/main.rs`

**Work:**
- Introduce a wrapper around `PlannedWorkoutRepository` for business reads.
- Hide planned workouts linked to an authoritative completed workout.
- Use both:
  - `CompletedWorkout.planned_workout_id`
  - `PlannedCompletedWorkoutLinkRepository`
- Hide legacy externally imported planned workouts so planned workouts become app-owned only.
- Prefer external-import metadata for that legacy filter and fall back to stable import-id patterns only where the stored metadata is insufficient.
- Do not treat `planned_workout_syncs` as external ownership because local app-created planned workouts may also have sync records there.

**Done when:**
- business readers only see app-owned planned workouts that are not already completed

## Task 7: Stop Importing External Calendar Entities

**Files:**
- `src/config/provider_polling/mod.rs`
- `src/main_runtime.rs`
- `src/adapters/intervals_icu/import_mapping.rs`
- `src/main.rs`

**Work:**
- Stop bootstrapping new external `ProviderPollStream::Calendar` states.
- Stop polling external calendar events as a source of canonical planned workouts, races, and special days.
- Remove or bypass `UpsertPlannedWorkout`, `UpsertRace`, and `UpsertSpecialDay` from the external polling flow.
- Keep Intervals and Wahoo external polling for completed workouts only.
- Decide explicitly how to hide or clean up historical externally imported races and special days so they do not continue to appear as if they were app-owned.
- Default to visibility-only cleanup first: keep the canonical rows for auditability and rollback, but hide historical externally imported races and special days through authoritative wrappers backed by external-import metadata.
- Consider hard delete or migration only if retained hidden rows create real product confusion, storage pressure, or policy issues.
- If a stronger cleanup path is chosen later, document the migration, rollback path, and how to preserve enough provenance to restore the old behavior safely.

**Done when:**
- external providers no longer create or refresh canonical planned workouts, races, or special days

## Task 8: Rewire All Reader Surfaces To Authoritative Wrappers

**Files:**
- `src/main.rs`
- `src/domain/calendar_view/refresh.rs`
- `src/domain/calendar_view/service.rs`
- `src/domain/calendar/service/mod.rs`
- `src/domain/training_load/use_cases.rs`
- `src/domain/training_context/service/mod.rs`
- `src/adapters/workout_summary_latest_activity.rs`
- `src/adapters/workout_summary_completed_target.rs`

**Work:**
- Wire `CompletedWorkoutReadService` to the authoritative completed-workout wrapper.
- Wire `CalendarEntryViewRefreshService` and `CalendarService` to authoritative planned and completed reads so calendar projections reflect the Wahoo-first rules.
- Wire `TrainingLoadRecomputeService` to authoritative completed-workout reads so hidden non-Wahoo workouts do not keep affecting training load on Wahoo-authoritative days.
- Wire `DefaultTrainingContextBuilder` to authoritative planned and completed reads.
- Wire `LatestCompletedActivityAdapter` and `CompletedWorkoutTargetAdapter` to authoritative reads so workout-summary flows also hide non-authoritative workouts.

**Done when:**
- calendar, training load, training context, and workout-summary readers all agree on the same Wahoo-first visibility model

## Task 9: Phase-1 Tests

**Minimum coverage:**
- Wahoo client DTO parsing and error mapping
- Wahoo bootstrap and reset behavior
- Wahoo polling success cadence and failure backoff
- Wahoo-specific cursor behavior if pagination or watermark state is needed
- authoritative completed-workout range filtering
- hidden non-Wahoo direct lookup returns `None`
- planned-workout hiding when linked to an authoritative completed workout
- training-load recompute uses authoritative completed workouts only
- training-context reads hide non-authoritative workouts and linked plans

## Task 10: Add Raw FIT Storage Linked To Canonical Completed Workouts

**Files:**
- create `src/domain/wahoo_fit_files/mod.rs`
- create `src/domain/wahoo_fit_files/model.rs`
- create `src/domain/wahoo_fit_files/ports.rs`
- create `src/adapters/mongo/wahoo_fit_files.rs`
- `src/main.rs`

**Work:**
- Add a raw FIT storage collection keyed to canonical `completed_workout_id`.
- Store:
  - canonical completed-workout id
  - Wahoo remote workout id
  - download metadata
  - file hash
  - raw FIT bytes or an equivalent durable binary representation
- Keep raw FIT storage separate from canonical completed-workout documents.

**Done when:**
- the app can durably store the full Wahoo FIT file without bloating canonical completed-workout documents

## Task 11: Add Durable Wahoo FIT Enrichment Tasks

**Files:**
- create `src/domain/wahoo_fit_enrichment/mod.rs`
- create `src/domain/wahoo_fit_enrichment/model.rs`
- create `src/domain/wahoo_fit_enrichment/service.rs`
- create `src/domain/wahoo_fit_enrichment/scheduler.rs`
- `src/domain/task_scheduler/**` only where small shared helpers are genuinely repeated
- `src/main.rs`

**Work:**
- Add a dedicated task type for Wahoo FIT enrichment.
- Enqueue that task immediately after a successful Wahoo summary import.
- Use a dedupe key based on canonical `completed_workout_id` so repeated summary imports do not create duplicate enrichment tasks.
- Persist progress checkpoints by stage, for example:
  - queued
  - downloaded
  - stored
  - parsed
  - enriched
- Reuse the existing shared task worker and task-runner patterns instead of inventing a one-off scheduler flow.

**Done when:**
- Wahoo summary import can schedule an idempotent, retryable enrichment task without losing track of progress across restarts

## Task 12: Implement The FIT Download, Parse, And Enrichment Handler

**Files:**
- `src/domain/wahoo_fit_enrichment/service.rs`
- `src/domain/wahoo_fit_enrichment/scheduler.rs`
- `src/adapters/activity_file_identity.rs` only if a small reusable FIT helper is genuinely shared
- `src/main.rs`

**Work:**
- Download the FIT file from the Wahoo summary file url.
- Persist the raw FIT file before parse and canonical enrichment side effects.
- Parse the FIT with `fitparser`.
- Map parsed data into canonical `CompletedWorkout.details`, streams, intervals, and richer metrics.
- Preserve idempotency on retries so a partial failure after storage does not create duplicate binary rows or corrupt canonical completed workouts.
- Clear or replace `details_unavailable_reason` only after the canonical workout is actually enriched.

**Done when:**
- a Wahoo completed workout can move from summary-only to richly detailed through a durable background workflow

## Task 13: Phase-2 Tests

**Minimum coverage:**
- raw FIT storage round-trip
- task enqueue dedupe by canonical completed-workout id
- task retry after transient download failure
- task retry after parse failure with stored raw FIT preserved
- repeated task execution remains idempotent
- enriched completed workout replaces the temporary Wahoo processing message with real details

## Operational Safety

- Rollback:
- keep the Wahoo-first behavior isolated to the authoritative wrapper wiring and Wahoo polling branch so rollback is a small code revert or wiring revert, not a storage migration.
- if production behavior is wrong, first disable Wahoo poll bootstrap / runtime wiring and switch readers back to the canonical repositories before considering any data cleanup.
- Observability:
- log Wahoo poll attempts, successes, failures, cursor updates, and partial-import recompute fallbacks with `user_id`, provider, stream, and workout identifiers where available.
- log FIT enrichment stage transitions and failures with `user_id`, `completed_workout_id`, and `wahoo_workout_id`.
- during rollout, watch for enrichment backlog growth, repeated parse/download failures, and unexpected spikes in hidden-workout behavior on Wahoo-authoritative days.

## Final Verification

Run at minimum before calling the implementation done:

```bash
bun run verify:arch
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
bun run --cwd frontend test
./scripts/rebuild_graphify.sh
```

If the final implementation splits phase 1 and phase 2 across separate PRs, run the same baseline verification plus the most relevant targeted tests for the phase being shipped.

## Exit Criteria

- Wahoo completed-workout polling is durable and runs every 3 hours.
- Wahoo summary imports create canonical completed workouts with Wahoo-first visibility rules.
- non-Wahoo completed workouts on Wahoo-authoritative days are hidden from all business readers and direct lookup paths.
- planned workouts disappear when linked to an authoritative completed workout.
- external providers no longer create canonical planned workouts, races, or special days.
- Wahoo FIT enrichment is durable, idempotent, and linked to canonical completed-workout ids.
- the implementation remains within current hexagonal boundaries and does not leak provider DTOs into domain code.
