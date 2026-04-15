# Domain Source of Truth PR5: Final Reader Cutover

**Goal:** Fully complete the migration so that local canonical domain state and local persisted read models are the business source of truth for readers.

After this PR:

- `calendar` business reads must come from local canonical state and `calendar_entry_views`
- `training_context` must use canonical local models, not provider-shaped `domain::intervals` business models
- `workout_summary` read-side decisions must resolve through canonical completed workouts, not `intervals_activities`
- `calendar_labels` must read structured local metadata, not parse fallback text
- frontend calendar, coach, and workout detail reads must stop using `/api/intervals/events` and `/api/intervals/activities` as business read sources
- `Intervals` must remain only on provider adapter and command-side flows such as create, update, delete, upload, sync, polling, and provider-specific file download

## Boundary Rule

- If a reader still needs provider-shaped data, add the missing canonical field instead of reusing `domain::intervals` business types.
- Do not restore live provider reads to preserve old behavior.
- Keep external REST contracts as compatible as practical, but only if the fields can be built honestly from local state.
- Prefer the smallest change that removes a dependency completely over a broader refactor that still leaves a compatibility bridge in place.

## Why PR5 Exists

PR1 to PR4 established the canonical write side, persisted calendar read side, provider polling, and part of the reader migration.

What is still incomplete in the current codebase:

- `CalendarService` is not fully cut over in production wiring to canonical completed workouts.
- `workout_summary` still resolves completed targets and latest-completed identity through `intervals_activities`.
- `training_context` still reconstructs canonical local data into `domain::intervals::Activity` and `domain::intervals::Event` compatibility types.
- `CalendarEntryView` still requires text parsing and provider-shaped compatibility bridging for some reader fields.
- frontend calendar, coach, and workout detail screens still use `/api/intervals/*` reader endpoints for business state.

PR5 is the final cutover PR that removes those remaining reader-side dependencies.

## Acceptance

- `CalendarService` reads from `CalendarEntryView` and canonical repos only.
- `training_context` no longer uses `domain::intervals::{Activity, Event, EventCategory, ActualWorkoutMatch}` as business reader models.
- `workout_summary` no longer depends on `ActivityRepositoryPort` for read-side decisions.
- `calendar_labels` no longer parse race metadata from free text.
- frontend business reads for calendar, coach, and workout details no longer use `/api/intervals/events` or `/api/intervals/activities`.
- `Intervals` remains only a provider adapter and command-side integration surface.

## Scope

### Mandatory scope for PR5

1. Fill remaining canonical model and repository gaps.
2. Migrate and backfill existing local data needed for read-side cutover.
3. Cut over backend calendar reads to canonical local state.
4. Cut over workout summary read-side decisions to canonical completed workouts.
5. Cut over `training_context` to canonical local reader models.
6. Add local completed-workout read endpoints needed by frontend business flows.
7. Cut over frontend calendar, coach, and workout detail reads.
8. Remove remaining backend and frontend business-read dependencies on `/api/intervals/*`.

### Explicitly out of scope for PR5

- redesigning public DTOs for aesthetic cleanliness only
- replacing provider command-side APIs such as upload, update, delete, or `download.fit`
- broad unrelated refactors across the whole `intervals` slice
- speculative unification of every compatibility type in one giant rename-only change

Those can happen later only if a concrete need remains after the source-of-truth migration is complete.

## Checklist

### 1. Fill remaining canonical model and repository gaps

**Files:**
- `src/domain/completed_workouts/model.rs`
- `src/domain/completed_workouts/ports.rs`
- `src/adapters/mongo/completed_workouts.rs`
- `src/domain/calendar/model.rs`
- `src/domain/calendar_view/mod.rs`
- `src/domain/calendar_view/projection.rs`

**Scope:**
- Add missing read-side metadata to `CompletedWorkout` so readers do not need to fall back to provider-shaped `Activity`.
- Recommended fields:
  - `source_activity_id`
  - `external_id`
  - `device_name`
  - `trainer`
  - `commute`
  - `race`
  - summary-level elevation and pace or speed metadata if still needed by readers
  - `details_unavailable_reason` if detail fetch incompleteness needs to surface honestly
- Add canonical repository queries needed by reader cutover:
  - `find_by_user_id_and_completed_workout_id(...)`
  - `find_by_user_id_and_source_activity_id(...)`
  - `find_latest_by_user_id(...)`
- Move actual-workout match models out of `domain::intervals` and into `calendar` or `calendar_view` owned types.
- Add typed planned-workout projection data to `CalendarEntryView` so readers do not need to reparse `raw_workout_doc`.

**Done when:**
- canonical repos and read models carry all fields needed by `calendar`, `training_context`, `workout_summary`, and frontend business readers
- no backend reader still needs `intervals::Activity` just because canonical state is missing data

### 2. Migrate and backfill existing local data

**Files:**
- create a focused migration or backfill module under `src/domain/external_sync/` or `src/main_support/`
- `src/domain/calendar_view/rebuild.rs`
- `src/domain/workout_summary/service/*`
- Mongo repositories touched by data backfill

**Scope:**
- backfill newly added canonical completed-workout fields for existing rows
- rebuild `calendar_entry_views` for existing users where required
- migrate legacy workout-summary identity from planned or event ids to completed activity identity where a canonical link is now available
- if both event-based and activity-based summaries exist for the same logical workout:
  - prefer the activity-backed identity
  - merge deterministically
  - preserve messages, recap text, RPE, and saved state without silent loss
- if a legacy summary cannot be migrated unambiguously:
  - do not guess
  - leave it unmigrated
  - log or report it explicitly for manual follow-up

**Done when:**
- existing data can be served entirely from canonical local state without silent reader regressions
- summary identity no longer depends on planned or event ids for active flows

### 3. Cut over backend calendar to canonical local reads

**Files:**
- `src/main.rs`
- `src/domain/calendar/model.rs`
- `src/domain/calendar/ports.rs`
- `src/domain/calendar/service.rs`
- `src/domain/calendar_view/projection.rs`
- `src/adapters/rest/calendar/dto.rs`
- `src/adapters/rest/calendar/mapping.rs`
- `src/adapters/mongo/calendar_entry_view_calendar.rs`

**Scope:**
- wire `CalendarService` to canonical completed workouts in production
- replace `intervals::ActualWorkoutMatch` and related interval match types with calendar-owned equivalents
- keep `GET /api/calendar/events` response shape as compatible as practical
- build planned-workout structure from typed canonical local fields, not from `parse_workout_doc(...)` over text
- remove race label fallback parsing from `description`
- keep `sync_planned_workout` as command-side logic, but do not let read-side source of truth depend on provider reads

**Done when:**
- calendar read output is built honestly from local state
- `CalendarEntryView` plus canonical repos are sufficient for calendar readers

### 4. Cut over workout summary read-side decisions to canonical completed workouts

**Files:**
- `src/adapters/workout_summary_completed_target.rs`
- `src/adapters/workout_summary_latest_activity.rs`
- `src/domain/workout_summary/service/mod.rs`
- `src/domain/workout_summary/service/use_cases.rs`
- `src/main.rs`

**Scope:**
- replace `ActivityRepositoryPort` based summary target checks with canonical completed-workout lookups
- replace latest-completed lookup with canonical repository logic
- preserve external summary identity based on completed `activityId`, but resolve it through canonical completed-workout fields
- keep batch summary listing tolerant of stale ids, but only return summaries for completed workout targets

**Done when:**
- summary, chat, recap, RPE, and save behavior are all anchored to canonical completed workouts
- `MongoActivityRepository` is no longer a read-side dependency for summary decisions

### 5. Cut over `training_context` to canonical local reader models

**Files:**
- `src/domain/training_context/service/mod.rs`
- `src/domain/training_context/service/context.rs`
- `src/domain/training_context/service/history.rs`
- `src/domain/training_context/service/power.rs`
- `src/domain/training_context/service/dates.rs`
- create a narrowly scoped local reader model helper module if needed

**Scope:**
- stop reconstructing canonical local data back into `domain::intervals::Activity` and `domain::intervals::Event`
- make the service operate on canonical local models directly:
  - `CompletedWorkout`
  - `PlannedWorkout`
  - `Race`
  - `SpecialDay`
  - `CalendarEntryView` or a training-context-specific local projection when needed
- replace plan-to-completed matching with canonical link first
- keep heuristic fallback only where genuinely required for legacy rows
- stop loading summary or recap by planned or event ids; use completed activity identity only
- keep parser-based compatibility only at explicit edges if some old text field must still be tolerated temporarily

**Done when:**
- `training_context` no longer depends on provider-shaped business models
- recent and future context are built from local canonical state only

### 6. Add local completed-workout read endpoints for frontend business flows

**Files:**
- create `src/adapters/rest/completed_workouts/dto.rs`
- create `src/adapters/rest/completed_workouts/handlers.rs`
- create `src/adapters/rest/completed_workouts/mapping.rs`
- update REST router and app wiring
- add REST tests

**Scope:**
- add `GET /api/completed-workouts?oldest&newest`
- add `GET /api/completed-workouts/:activityId`
- if needed for workout detail flows, add a local detail endpoint for planned-workout entry detail instead of re-reading provider event detail
- keep DTOs compatible enough for current frontend usage so the cutover diff stays focused

**Done when:**
- frontend can fetch completed workouts and workout detail state without `/api/intervals/activities`

### 7. Cut over frontend calendar, coach, and workout detail reads

**Files:**
- `frontend/src/features/calendar/hooks/useCalendarData.ts`
- `frontend/src/features/calendar/dayItems.ts`
- `frontend/src/features/calendar/components/WorkoutDetailModal.tsx`
- `frontend/src/features/coach/hooks/useWorkoutList.ts`
- `frontend/src/features/intervals/api/intervals.ts`
- `frontend/src/features/intervals/types.ts`
- create feature-local completed-workout API and types if that keeps the diff cleaner

**Scope:**
- calendar must use local calendar events plus local completed workouts
- workout detail modal must use local detail endpoints instead of `loadEvent(...)` and `loadActivity(...)`
- coach workout list must use `/api/calendar/events` plus local completed-workout reads, not `/api/intervals/events` plus `/api/intervals/activities`
- preserve linked planned or completed rendering behavior
- remove frontend heuristics only where backend now provides authoritative local data

**Done when:**
- no production frontend business-read flow depends on `/api/intervals/events` or `/api/intervals/activities`

### 8. Restrict `/api/intervals/*` to provider and command-side responsibilities

**Files:**
- `src/adapters/rest/intervals/handlers.rs`
- routing or docs updates if needed
- frontend call sites and tests

**Scope:**
- keep create, update, delete, upload, sync-supporting, and `download.fit` behavior
- stop using provider read endpoints as application read models
- if read endpoints remain exposed, treat them as provider integration APIs rather than business source-of-truth APIs

**Done when:**
- no core app reader depends on `/api/intervals/events` or `/api/intervals/activities`

### 9. Remove compatibility bridges and text parsing fallbacks made unnecessary by the cutover

**Files:**
- `src/domain/calendar_view/projection.rs`
- `src/adapters/mongo/calendar_entry_view_calendar.rs`
- `src/domain/training_context/service/*`
- `src/adapters/workout_summary_*`
- any other remaining reader-only compatibility helpers discovered during the cutover

**Scope:**
- remove `parse_race_description(...)` fallback
- remove planned-workout reparsing from `raw_workout_doc` when typed data is present
- remove summary lookup or recap lookup paths that still consider planned or event ids
- remove provider-shaped internal reconstruction where canonical data fully replaces it

**Done when:**
- the remaining compatibility code is only for explicit command-side or external-contract reasons, not because business readers still depend on it

## Execution Order

1. Fill canonical model and repository gaps.
2. Implement data migration and backfill.
3. Cut over backend calendar.
4. Cut over workout summary.
5. Cut over `training_context`.
6. Add local completed-workout and detail endpoints.
7. Cut over frontend reads.
8. Remove compatibility bridges and reader fallbacks.
9. Run final verification.

## Verification

### Targeted checks during implementation

- `cargo test completed_workouts -- --nocapture`
- `cargo test calendar_view -- --nocapture`
- `cargo test calendar_labels -- --nocapture`
- `cargo test workout_summary -- --nocapture`
- `cargo test training_context -- --nocapture`
- `cargo test llm_rest workout_summary_flow -- --nocapture`
- targeted calendar REST and completed-workout REST tests
- targeted frontend tests for calendar, coach, and workout detail flows

### Final checks

- `cargo fmt --all --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test`
- `bun run --cwd frontend test`
- `bun run --cwd frontend build`
- `bun run verify:all`

## Review Loop Requirement

This PR is not done until it passes the repo review loop:

- 4 iterations total
- in each iteration run 3 phases in order:
  - strict reviewer
  - very strict reviewer
  - nitpicker
- after each iteration:
  - confirm findings against current code
  - implement only the real fixes
  - rerun the most relevant verification before the next iteration

## Risks

- migrating legacy workout summaries from event or planned identity to completed activity identity without losing messages or recap state
- preserving current frontend expectations while changing backend data sources
- hidden remaining helper paths that still depend on `domain::intervals` business types
- existing users with incomplete or stale `calendar_entry_views`

## Definition Of Done

PR5 is complete when all of the following are true:

- local canonical state is the only business source of truth for calendar, training context, workout summary, and labels
- frontend calendar, coach, and workout detail screens no longer use `/api/intervals/events` or `/api/intervals/activities` for business reads
- `Intervals` remains only a provider adapter and command-side integration path
- legacy planned or event summary identity is no longer part of the active product flow
- all required verification commands were run and their output was read

## What Should Still Remain After PR5

These are acceptable after PR5 and do not mean the migration is incomplete:

- provider command-side APIs for create, update, delete, upload, and `download.fit`
- compatibility fields that remain only to keep a stable external REST contract
- explicit provider identifiers preserved as metadata

If a future cleanup only simplifies naming or removes harmless compatibility wrappers without changing the source-of-truth boundary, that cleanup is optional and should not block declaring the migration complete.
