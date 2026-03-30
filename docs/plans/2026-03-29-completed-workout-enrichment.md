# Completed Workout Enrichment Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Enrich completed Intervals activities with dedicated intervals and streams data so the calendar modal can render real completed-workout metrics, charts, and interval sections for Strava-imported workouts.

**Architecture:** Keep calendar list queries lightweight and push completed-workout enrichment into the Intervals adapter and `get_activity()` use case only. The backend should merge the base activity payload with dedicated `/intervals` and `/streams?includeDefaults=true` sub-resources, persist the enriched activity, and return the existing `Activity` DTO shape so the frontend can render richer completed-only workouts without synthetic events.

**Tech Stack:** Rust, Axum, reqwest, MongoDB, React, TypeScript, Vitest

---

### Task 1: Add failing backend adapter tests for completed-activity enrichment

**Files:**
- Modify: `tests/intervals_adapters.rs`
- Reference: `src/adapters/intervals_icu/client.rs`
- Reference: `src/adapters/intervals_icu/dto.rs`

**Step 1: Write the failing test**

Add a test that mocks Intervals responses so:
- `GET /api/v1/activity/{id}` returns a sparse Strava-like activity stub
- `GET /api/v1/activity/{id}/intervals` returns interval/group data
- `GET /api/v1/activity/{id}/streams` returns stream data

Assert that `IntervalsIcuClient::get_activity(...)` returns:
- populated `details.intervals`
- populated `details.interval_groups`
- populated `details.streams`

**Step 2: Run test to verify it fails**

Run: `cargo test --test intervals_adapters completed_activity_detail_enrichment -- --nocapture`

Expected: FAIL because the client does not yet call `/intervals` or `includeDefaults=true` streams.

**Step 3: Write the minimal implementation**

Modify `src/adapters/intervals_icu/client.rs` to:
- add a dedicated fetch for `/api/v1/activity/{id}/intervals`
- fetch `/streams` with `includeDefaults=true`
- merge interval/group data and streams into the base activity

Add any DTOs needed in `src/adapters/intervals_icu/dto.rs`.

**Step 4: Run test to verify it passes**

Run: `cargo test --test intervals_adapters completed_activity_detail_enrichment -- --nocapture`

Expected: PASS

### Task 2: Add failing backend resilience test for partial enrichment

**Files:**
- Modify: `tests/intervals_adapters.rs`
- Modify: `src/adapters/intervals_icu/client.rs`

**Step 1: Write the failing test**

Add a test where:
- base activity request succeeds
- `/intervals` fails or returns an error
- `/streams` still succeeds, or vice versa

Assert that `get_activity(...)` still returns the base activity with whichever enrichment succeeded.

**Step 2: Run test to verify it fails**

Run: `cargo test --test intervals_adapters completed_activity_partial_enrichment -- --nocapture`

Expected: FAIL because one sub-resource error currently fails the whole request.

**Step 3: Write the minimal implementation**

Update `src/adapters/intervals_icu/client.rs` so sub-resource enrichment failures are logged and treated as partial enrichment, not total request failure.

**Step 4: Run test to verify it passes**

Run: `cargo test --test intervals_adapters completed_activity_partial_enrichment -- --nocapture`

Expected: PASS

### Task 3: Add failing backend service persistence test

**Files:**
- Modify: `tests/intervals_service.rs` or `tests/intervals_rest.rs`
- Reference: `src/domain/intervals/service.rs`
- Reference: `src/adapters/mongo/activities.rs`

**Step 1: Write the failing test**

Add a test that calls `get_activity(...)` with a fake API returning enriched completed activity details and verifies the resulting activity persisted to the repository includes:
- intervals
- interval groups
- streams

**Step 2: Run test to verify it fails**

Run: `cargo test --test intervals_service get_activity_persists_enriched_completed_activity -- --nocapture`

Expected: FAIL if repository persistence still reflects pre-enrichment shape.

**Step 3: Write the minimal implementation**

If needed, adjust merge timing so persistence happens after enrichment is fully assembled.

**Step 4: Run test to verify it passes**

Run: `cargo test --test intervals_service get_activity_persists_enriched_completed_activity -- --nocapture`

Expected: PASS

### Task 4: Add failing frontend test for completed interval sections

**Files:**
- Modify: `frontend/src/features/calendar/components/WorkoutDetailModal.test.tsx`
- Reference: `frontend/src/features/calendar/components/WorkoutDetailModal.tsx`

**Step 1: Write the failing test**

Add a modal test for a completed-only activity payload that includes:
- metrics
- streams
- `details.intervals`

Assert that the modal renders:
- completed summary metrics
- interval section rows/cards derived from `details.intervals`

**Step 2: Run test to verify it fails**

Run: `bun run --cwd frontend test src/features/calendar/components/WorkoutDetailModal.test.tsx`

Expected: FAIL if completed interval sections are not yet rendered from activity interval data.

**Step 3: Write the minimal implementation**

Update `frontend/src/features/calendar/components/WorkoutDetailModal.tsx` to render completed interval sections from enriched activity interval data while preserving current planned and planned-vs-actual flows.

**Step 4: Run test to verify it passes**

Run: `bun run --cwd frontend test src/features/calendar/components/WorkoutDetailModal.test.tsx`

Expected: PASS

### Task 5: Add failing frontend regression test for completed charts/metrics from enriched payloads

**Files:**
- Modify: `frontend/src/features/calendar/components/WorkoutDetailModal.test.tsx`
- Reference: `frontend/src/features/calendar/workoutDetails.ts`

**Step 1: Write the failing test**

Add a completed-only modal test using realistic enriched activity payloads and assert:
- title is no longer generic fallback when name/activity type exists
- duration is non-zero
- NP/TSS render from activity metrics
- chart area renders bars/paths from streams

**Step 2: Run test to verify it fails**

Run: `bun run --cwd frontend test src/features/calendar/components/WorkoutDetailModal.test.tsx`

Expected: FAIL if the completed path still ignores interval or stream-rich payloads.

**Step 3: Write the minimal implementation**

Adjust frontend completed-workout helpers/components only as needed to consume the richer existing activity DTO.

**Step 4: Run test to verify it passes**

Run: `bun run --cwd frontend test src/features/calendar/components/WorkoutDetailModal.test.tsx`

Expected: PASS

### Task 6: Run focused verification

**Files:**
- Verify touched backend and frontend files

**Step 1: Run backend focused tests**

Run: `cargo test --test intervals_adapters -- --nocapture`

Expected: PASS

**Step 2: Run backend service/integration tests**

Run one or both depending on where the persistence regression test lands:
- `cargo test --test intervals_service -- --nocapture`
- `cargo test --test intervals_rest -- --nocapture`

Expected: PASS

**Step 3: Run frontend focused tests**

Run: `bun run --cwd frontend test src/features/calendar/components/WorkoutDetailModal.test.tsx src/features/intervals/api/intervals.test.ts`

Expected: PASS

**Step 4: Run strict Rust verification**

Run: `cargo clippy --all-targets --all-features -- -D warnings`

Expected: PASS

**Step 5: Run frontend build**

Run: `bun run --cwd frontend build`

Expected: PASS
