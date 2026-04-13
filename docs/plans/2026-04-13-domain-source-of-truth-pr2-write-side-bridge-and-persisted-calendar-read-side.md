# Domain Source of Truth PR2: Write-Side Bridge And Persisted Calendar Read Side Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Bridge the missing canonical write-side persistence needed in the current branch and deliver the persisted `CalendarEntryView` read model used for future local-first readers.

**Architecture:** Reuse the currently persisted local stores that already exist in this branch as the bridge into canonical roots where possible: `training_plan_projected_days` as the current local backing source for planned workouts and `intervals_activities` as the current local backing source for completed workouts. Add the missing local special-day persistence, build `CalendarEntryView` as a Mongo-backed projected read model, and update current write flows to maintain that read model synchronously where practical. Replace the temporary `MongoRaceCalendarSource` adapter with adapters backed by persisted `CalendarEntryView` rows.

**Tech Stack:** Rust 2021, MongoDB, Axum, existing hexagonal ports and adapters, current `training_plan` and `intervals` services.

## Scope Notes

- This plan supersedes the narrower PR2 plan because the current branch does not yet have complete canonical persistence and write-flow coverage for all roots assumed by that plan.
- Planned workouts are bridged from already persisted local training-plan projections in this branch.
- Completed workouts are bridged from already persisted local activities in this branch.
- Special days need new local persistence and a minimal write-side service in this branch.
- Full reader migration still remains a later step; this plan only introduces the persisted read model and the adapters needed to replace the temporary race calendar join.

---

### Task 1: Extend canonical ports for queryable local roots

**Files:**
- Modify: `src/domain/planned_workouts/ports.rs`
- Modify: `src/domain/completed_workouts/ports.rs`
- Modify: `src/domain/special_days/ports.rs`
- Create or modify tests under `src/domain/planned_workouts/`, `src/domain/completed_workouts/`, `src/domain/special_days/`

**Step 1: Write failing tests for repository contracts**
Cover:
- listing planned workouts by user and date range
- listing completed workouts by user and date range
- listing special days by user and date range

**Step 2: Add minimal query methods to the canonical ports**
Support:
- `list_by_user_id`
- `list_by_user_id_and_date_range`
- extra replacement helpers only where the current write flow truly needs them

**Step 3: Keep the ports small**
Do not add speculative commands or workflow abstractions.

**Step 4: Run targeted tests**
```bash
cargo test planned_workouts completed_workouts special_days -- --nocapture
```

### Task 2: Add write-side bridge adapters for planned and completed workouts

**Files:**
- Create: `src/adapters/mongo/planned_workouts.rs`
- Create: `src/adapters/mongo/completed_workouts.rs`
- Modify: `src/adapters/mongo/mod.rs`
- Create: tests under `tests/` for the new adapters

**Step 1: Write failing repository tests**
Cover:
- planned workouts read from current local training-plan persistence
- completed workouts read from current local activity persistence
- per-user scoping
- date-range sorting

**Step 2: Implement explicit mapping at the adapter boundary**
Rules:
- map `TrainingPlanProjectedDay` persistence into canonical `PlannedWorkout`
- map `Activity` persistence into canonical `CompletedWorkout`
- do not leak `domain::intervals` types into canonical modules

**Step 3: Add only the indexes or queries actually needed**
Reuse existing collections where possible; avoid introducing a second completed-workout store.

**Step 4: Run targeted tests**
```bash
cargo test planned_workouts completed_workouts -- --nocapture
```

### Task 3: Add missing special-day persistence and minimal write-side service

**Files:**
- Create: `src/adapters/mongo/special_days.rs`
- Create: `src/domain/special_days/service.rs`
- Modify: `src/domain/special_days/mod.rs`
- Create: tests under `src/domain/special_days/` and `tests/`
- Modify: `src/adapters/mongo/mod.rs`
- Modify: `src/main.rs`

**Step 1: Write failing tests for special-day persistence and service behavior**
Cover:
- upsert special day
- list special days by user and date range
- per-user scoping

**Step 2: Implement Mongo repository with separate document shape**
Add indexes for:
- `user_id + special_day_id`
- `user_id + date`

**Step 3: Implement minimal special-day service**
Keep it focused on local persistence only.

**Step 4: Wire the repository and service in `main.rs`**
Do not expose extra API surface unless needed by tests in this plan.

**Step 5: Run targeted tests**
```bash
cargo test special_days -- --nocapture
```

### Task 4: Add calendar view domain module

**Files:**
- Create: `src/domain/calendar_view/mod.rs`
- Create: `src/domain/calendar_view/model.rs`
- Create: `src/domain/calendar_view/ports.rs`
- Create: `src/domain/calendar_view/service.rs`
- Modify: `src/domain/mod.rs`

**Step 1: Write failing tests for mixed-type read-model reads**
Cover date-range reads returning:
- planned workout entries
- completed workout entries
- race entries
- special day entries

**Step 2: Define `CalendarEntryView`**
Include:
- stable local entry identity
- entry kind
- date and optional `start_date_local`
- title, subtitle, description
- canonical root references
- optional linked external Intervals event metadata where already locally persisted
- locally derived summaries only

**Step 3: Add the read-model repository port**
Support:
- list by user and date range
- upsert or replace helpers needed by projection maintenance

**Step 4: Run targeted tests**
```bash
cargo test calendar_view -- --nocapture
```

### Task 5: Add projectors, rebuild support, and integrity checks

**Files:**
- Create: `src/domain/calendar_view/projection.rs`
- Create: `src/domain/calendar_view/rebuild.rs`
- Create: `src/domain/calendar_view/integrity.rs`
- Create: `src/domain/calendar_view/tests.rs`
- Modify: `src/domain/calendar_view/mod.rs`
- Modify: `src/domain/calendar_view/service.rs`

**Step 1: Write failing tests for each projector**
Cover:
- `PlannedWorkout -> CalendarEntryView`
- `CompletedWorkout -> CalendarEntryView`
- `Race -> CalendarEntryView`
- `SpecialDay -> CalendarEntryView`

**Step 2: Implement deterministic mapping helpers**
Only derive fields from local canonical state.

**Step 3: Write failing rebuild tests**
Cover:
- rebuild from empty view store
- rebuild replaces stale entries
- repeated rebuilds stay idempotent

**Step 4: Implement rebuild orchestration**
Use the canonical local root repositories and replace the view store deterministically.

**Step 5: Write failing integrity tests**
Cover:
- missing rows
- duplicates
- type mismatches
- orphaned rows

**Step 6: Implement integrity helpers**
Return explicit, inspectable mismatches.

**Step 7: Run targeted tests**
```bash
cargo test calendar_view rebuild integrity -- --nocapture
```

### Task 6: Add Mongo repository for calendar entry views

**Files:**
- Create: `src/adapters/mongo/calendar_entry_views.rs`
- Modify: `src/adapters/mongo/mod.rs`
- Modify: `src/main.rs`
- Create: tests under `tests/`

**Step 1: Write failing Mongo repository tests**
Cover:
- date-range reads
- sorting by date
- mixed entry kinds
- per-user scoping
- uniqueness by local entry id

**Step 2: Implement document mapping and indexes**
At minimum:
- `user_id + date`
- `user_id + entry_kind + date`
- `user_id + entry_id`

**Step 3: Wire the repository in `main.rs`**
Construct it alongside the other Mongo repositories.

**Step 4: Run targeted tests**
```bash
cargo test calendar_entry_views -- --nocapture
```

### Task 7: Wire synchronous calendar-view updates from local write flows

**Files:**
- Modify: `src/domain/training_plan/service/mod.rs`
- Modify: `src/domain/intervals/service/activities.rs`
- Modify: `src/domain/races/service.rs`
- Modify: `src/domain/special_days/service.rs`
- Modify: `src/domain/calendar_view/service.rs`
- Modify: `src/main.rs`

**Step 1: Write failing tests for write-side projection updates**
Cover:
- training-plan projection write updates planned calendar entries
- activity persistence updates completed-workout calendar entries
- race create or update updates race calendar entry
- special-day upsert updates special-day calendar entry

**Step 2: Keep projection updates synchronous and simple**
No async queue in this PR.

**Step 3: Preserve durable-local-first behavior**
Persist canonical local state before any provider side effect when a flow includes both.

**Step 4: Run targeted tests**
```bash
cargo test calendar_view -- --nocapture
```

### Task 8: Replace temporary `MongoRaceCalendarSource` with calendar-view-backed adapters

**Files:**
- Delete: `src/adapters/mongo/race_calendar.rs`
- Modify: `src/adapters/mongo/mod.rs`
- Create or modify: adapters that expose `CalendarLabelSource` and `HiddenCalendarEventSource` from `CalendarEntryView`
- Modify: `src/main.rs`
- Modify: calendar-label and race-related tests

**Step 1: Write failing tests for race labels and hidden IDs backed by the persisted view**
Cover:
- race labels still render correctly
- only linked race event IDs are hidden
- user scoping remains correct

**Step 2: Implement read adapters over `CalendarEntryView`**
Keep the existing REST contract intact.

**Step 3: Remove the temporary compatibility adapter and its wiring**

**Step 4: Run targeted tests**
```bash
cargo test races_mongo calendar_labels -- --nocapture
```

### Task 9: Final verification

**Step 1: Run formatter check**
```bash
cargo fmt --all --check
```

**Step 2: Run clippy**
```bash
cargo clippy --all-targets --all-features -- -D warnings
```

**Step 3: Run focused backend tests**
```bash
cargo test calendar_view planned_workouts completed_workouts special_days races_mongo -- --nocapture
```

**Step 4: Run full backend tests**
```bash
cargo test
```

**Step 5: Rebuild graphify as required by repo instructions**
```bash
python3 -c "from graphify.watch import _rebuild_code; from pathlib import Path; _rebuild_code(Path('.'))"
```
