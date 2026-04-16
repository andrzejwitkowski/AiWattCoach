# Domain Source of Truth PR4: Reader Migration And Cleanup

**Goal:** Move business readers to canonical local state while keeping the current `GET /api/calendar/events` contract as compatible as practical during this PR.

## Rules

- Keep external calendar response shape stable unless a field cannot be built honestly from local state.
- Do not restore provider reads just to preserve old behavior.
- If a reader still needs provider-shaped data, add the missing canonical field instead.
- `Intervals` may remain on command-side and provider adapter paths.

## Current Gaps

- `SpecialDay` is too thin for `sick_note` and special-day reader semantics.
- `CompletedWorkout` is missing reader-friendly metadata.
- There is no durable planned-to-completed link in canonical local state.
- `calendar` domain is still provider-shaped.
- `CalendarEntryView` still encodes some typed data in text fields.

## Checklist

### 1. Fill canonical model gaps

Files:
- `src/domain/special_days/model.rs`
- `src/adapters/mongo/special_days.rs`
- `src/domain/completed_workouts/model.rs`
- `src/adapters/mongo/completed_workouts.rs`

Scope:
- Add text fields to `SpecialDay`.
- Add read-side metadata to `CompletedWorkout`.
- Add optional `planned_workout_id` to `CompletedWorkout`.

Done when:
- Local state carries everything `calendar` and `training_context` need without provider read fallbacks.

### 2. Refresh `CalendarEntryView` projections

Files:
- `src/domain/calendar_view/projection.rs`
- read-model refresh paths touched by PR1 to PR3

Scope:
- Use canonical metadata for titles, descriptions, and summaries.
- Remove obvious placeholder fallbacks where canonical values now exist.

Done when:
- `CalendarEntryView` is sufficient as the local read source for calendar readers.

### 3. Migrate domain `calendar` to local-first

Files:
- `src/domain/calendar/model.rs`
- `src/domain/calendar/service.rs`
- `src/domain/calendar/ports.rs`

Scope:
- Replace `domain::intervals::Event` in `CalendarEvent`.
- Serve `list_events` from `CalendarEntryView`.
- Keep `sync_planned_workout` as command-side logic.

Done when:
- Calendar business reads do not call Intervals list/get event APIs.

### 4. Keep calendar REST contract compatible

Files:
- `src/adapters/rest/calendar/dto.rs`
- `src/adapters/rest/calendar/mapping.rs`
- `src/adapters/rest/calendar/handlers.rs`
- calendar REST tests

Scope:
- Preserve current response shape where practical.
- Build response fields from local canonical state and read models.
- Keep provider identifiers only as metadata.

Done when:
- `GET /api/calendar/events` stays practically compatible without live provider reads.

### 5. Migrate `training_context` to local repositories

Files:
- `src/domain/training_context/service/mod.rs`
- `src/domain/training_context/service/context.rs`
- `src/domain/training_context/service/history.rs`
- `src/domain/training_context/service/power.rs`
- `src/domain/training_context/service/dates.rs`

Scope:
- Build recent and upcoming context from canonical local state.
- Use `CompletedWorkoutRepository`, `PlannedWorkoutRepository`, `SpecialDayRepository`, `RaceRepository`, and `CalendarEntryView` as needed.
- Replace plan-to-completion matching with canonical link first, heuristic fallback only for legacy rows.

Done when:
- Training context business reads no longer depend on Intervals event reads.

### 6. Treat `calendar_labels` as cleanup

Files:
- `src/domain/calendar_labels/service.rs`
- `src/adapters/mongo/calendar_entry_view_calendar.rs`

Scope:
- Keep labels local-first.
- Align label mapping with updated `CalendarEntryView` fields.
- Remove text parsing hacks if the diff stays small.

Done when:
- Labels remain local and do not pull architecture backward.

### 7. Remove legacy business reads from `domain::intervals`

Files:
- calendar and training-context call sites
- any remaining thin wrappers that only exist for old read flows

Scope:
- Leave `Intervals` only for polling, import, sync command-side, and provider-specific adapter responsibilities.

Done when:
- `domain::intervals` is no longer the read-side source of truth for business flows.

### 8. Verify in layers

Order:
- model and repository tests for `special_days`, `completed_workouts`, `calendar_view`
- `calendar` domain and REST tests
- `training_context` tests
- full Rust verification
- frontend tests and build only if contract-facing behavior changed

Commands:
- `cargo fmt --all --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test`
- `bun run verify:rust`
- `bun run --cwd frontend test`
- `bun run --cwd frontend build`

## Execution Order

1. Fill canonical model gaps.
2. Refresh `CalendarEntryView` projections.
3. Migrate domain `calendar`.
4. Keep calendar REST contract compatible.
5. Migrate `training_context`.
6. Cleanup `calendar_labels`.
7. Remove legacy business reads.
8. Run final verification.

## Acceptance

- `GET /api/calendar/events` remains practically compatible.
- `calendar` and `training_context` do not perform business reads from Intervals.
- `sync_planned_workout` still works.
- Local canonical state is the source of truth for readers, not just writers and importers.
