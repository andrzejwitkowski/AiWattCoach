# Workout Review Fixes Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Fix the reviewed workout backend/frontend regressions and add focused regression tests.

**Architecture:** Move workout enrichment orchestration from the REST adapter into `IntervalsService`, make detailed activity hydration robust to sparse list payloads, and keep the frontend modal resilient when only part of the detail fetch succeeds. Preserve minimal diffs in the calendar UI by reverting rest-day interaction and tightening FIT-action gating.

**Tech Stack:** Rust, Axum, TypeScript, React, Vitest, Testing Library

---

### Task 1: Add failing backend regression tests

**Files:**
- Modify: `tests/intervals_rest.rs`
- Modify: `tests/intervals_workout_analysis.rs`

**Step 1: Write failing tests**

- Add a REST test proving `get_event` can still hydrate `actualWorkout` from detailed same-day activity lookup even when the listed activity lacks matchable intervals/streams.
- Add a domain test proving invalid/null stream samples are ignored instead of converted to zeros.

**Step 2: Run tests to verify they fail**

Run: `cargo test --test intervals_rest get_event_hydrates_actual_workout_from_detailed_activity_lookup_without_list_match -- --nocapture && cargo test --test intervals_workout_analysis ignores_invalid_stream_samples_when_extracting_actual_workout_data -- --nocapture`

Expected: FAIL for the newly added assertions.

### Task 2: Add failing frontend regression tests

**Files:**
- Modify: `frontend/src/features/calendar/components/WorkoutDetailModal.test.tsx`
- Modify: `frontend/src/features/calendar/components/CalendarDayCell.test.tsx`

**Step 1: Write failing tests**

- Add a modal test where `loadEvent` succeeds and `loadActivity` fails, and assert planned/event content still renders.
- Add a completed modal test that renders from `event.actualWorkout` without a loaded `activity` and asserts actual metrics/compliance are still shown.
- Add a day-cell test proving a rest day is not rendered as a button.
- Add a modal test proving `Download FIT` is hidden in completed mode.

**Step 2: Run tests to verify they fail**

Run: `bun run --cwd frontend test src/features/calendar/components/WorkoutDetailModal.test.tsx src/features/calendar/components/CalendarDayCell.test.tsx`

Expected: FAIL for the new assertions.

### Task 3: Implement backend fixes

**Files:**
- Modify: `src/domain/intervals/service.rs`
- Modify: `src/domain/intervals/mod.rs`
- Modify: `src/domain/intervals/workout.rs`
- Modify: `src/adapters/rest/intervals.rs`

**Step 1: Write minimal implementation**

- Introduce a service-layer method that returns an enriched event DTO input model or equivalent domain result.
- Remove enrichment orchestration from the REST handler path.
- Change same-day matching to hydrate candidate activities before final best-match selection.
- Make stream extraction skip invalid/non-numeric samples.

**Step 2: Run backend regression tests to verify they pass**

Run: `cargo test --test intervals_rest get_event_hydrates_actual_workout_from_detailed_activity_lookup_without_list_match -- --nocapture && cargo test --test intervals_workout_analysis ignores_invalid_stream_samples_when_extracting_actual_workout_data -- --nocapture`

Expected: PASS.

### Task 4: Implement frontend fixes

**Files:**
- Modify: `frontend/src/features/calendar/components/WorkoutDetailModal.tsx`
- Modify: `frontend/src/features/calendar/components/CalendarDayCell.tsx`

**Step 1: Write minimal implementation**

- Make modal fetches resilient to partial failure.
- Render completed charts and metrics from `event.actualWorkout` when `activity` is unavailable.
- Hide FIT action in completed mode.
- Render rest days with a non-interactive container while preserving clickable training days.

**Step 2: Run focused frontend tests to verify they pass**

Run: `bun run --cwd frontend test src/features/calendar/components/WorkoutDetailModal.test.tsx src/features/calendar/components/CalendarDayCell.test.tsx`

Expected: PASS.

### Task 5: Verify touched areas

**Files:**
- Verify: `tests/intervals_rest.rs`
- Verify: `tests/intervals_workout_analysis.rs`
- Verify: `frontend/src/features/calendar/components/WorkoutDetailModal.test.tsx`
- Verify: `frontend/src/features/calendar/components/CalendarDayCell.test.tsx`
- Verify: `frontend/src/features/calendar/components/CalendarGrid.test.tsx`
- Verify: `frontend/src/features/intervals/api/intervals.test.ts`

**Step 1: Run verification**

Run: `cargo test --test intervals_rest -- --nocapture && cargo test --test intervals_workout_analysis -- --nocapture && bun run --cwd frontend test src/features/calendar/components/WorkoutDetailModal.test.tsx src/features/calendar/components/CalendarDayCell.test.tsx src/features/calendar/components/CalendarGrid.test.tsx src/features/intervals/api/intervals.test.ts`

Expected: PASS with no new failures.
