# Intervals Settings Test Button Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a `Test Connection` action to the Settings Intervals.icu card, make both actions use the currently displayed draft values, and stop clearing the inputs after save.

**Architecture:** Keep persistence and validation responsibilities separate. The frontend Intervals card owns a local draft state derived from persisted settings, calls `POST /api/settings/intervals/test` for validation-only feedback, and calls `PATCH /api/settings/intervals` for persistence. The backend stays unchanged because the existing test endpoint already supports draft-or-saved credential merging.

**Tech Stack:** React 19, Vitest, Testing Library, TypeScript, existing shared `httpClient`, Rust/Axum backend endpoint already in place.

---

### Task 1: Add failing frontend tests for the new Intervals behavior

**Files:**
- Create: `frontend/src/features/settings/api/settings.test.ts`
- Create: `frontend/src/features/settings/components/IntervalsCard.test.tsx`

**Step 1: Write the failing API test**

Add a test proving `testIntervalsConnection()` sends a POST request to `/api/settings/intervals/test`, includes credentials, and returns parsed JSON for both `200` and handled failure statuses.

**Step 2: Run test to verify it fails**

Run: `bun run --cwd frontend test src/features/settings/api/settings.test.ts`
Expected: FAIL because `testIntervalsConnection` does not exist yet.

**Step 3: Write the failing component tests**

Add tests for:
- saving Intervals credentials does not clear the visible inputs
- the test button uses the currently displayed draft values
- a successful test renders an `OK` status panel
- a failed test renders a `FAILED` status panel
- editing a field after a test clears the stale test result

**Step 4: Run component tests to verify they fail**

Run: `bun run --cwd frontend test src/features/settings/components/IntervalsCard.test.tsx`
Expected: FAIL because the component does not support the new behavior yet.

### Task 2: Add the new settings API helper and response types

**Files:**
- Modify: `frontend/src/features/settings/types.ts`
- Modify: `frontend/src/features/settings/api/settings.ts`

**Step 1: Add the failing response schema usage path**

Define a schema and exported type for the Intervals test response with `connected`, `message`, `usedSavedApiKey`, `usedSavedAthleteId`, and `persistedStatusUpdated`.

**Step 2: Implement `testIntervalsConnection()` minimally**

Use the currently shared fetch conventions (`Accept`, `Content-Type`, `credentials: 'include'`). Parse JSON on `200`, `400`, and `503`, throw `AuthenticationError` for `401`, and throw `HttpError` for unexpected statuses or invalid JSON.

**Step 3: Re-run the API test**

Run: `bun run --cwd frontend test src/features/settings/api/settings.test.ts`
Expected: PASS.

### Task 3: Refactor the Intervals card around draft-driven form state

**Files:**
- Modify: `frontend/src/features/settings/components/IntervalsCard.tsx`

**Step 1: Implement draft state derived from persisted settings**

Track visible input values locally and sync them from `settings.intervals` on settings refresh. Treat the masked persisted API key as a display baseline so unchanged masked values are not re-sent to the backend.

**Step 2: Implement separate `Test Connection` and `Connect Intervals` actions**

Make both actions read from the visible draft values. `Test Connection` calls the new API helper and only updates local feedback state. `Connect Intervals` calls the existing save API, keeps the inputs populated, and triggers the existing refresh callback.

**Step 3: Add inline action feedback**

Render a compact status panel above the buttons with:
- neutral copy while an action is in progress
- green `OK` feedback when the test succeeds
- red `FAILED` feedback when the test fails
- save success/failure messaging without using toasts

**Step 4: Clear stale test results on edit**

If the user edits either field after a completed test, clear the last test result so the UI never shows `OK` or `FAILED` for outdated draft values.

**Step 5: Re-run the component tests**

Run: `bun run --cwd frontend test src/features/settings/components/IntervalsCard.test.tsx`
Expected: PASS.

### Task 4: Run focused and broader verification

**Files:**
- Verify only

**Step 1: Run the touched frontend tests together**

Run: `bun run --cwd frontend test src/features/settings/api/settings.test.ts src/features/settings/components/IntervalsCard.test.tsx`
Expected: PASS.

**Step 2: Run the full frontend verification**

Run: `bun run verify:frontend`
Expected: frontend tests pass and the frontend build succeeds.

**Step 3: If Rust files were changed, run Rust verification**

Run: `cargo fmt -- --check && cargo clippy --all-targets --all-features -- -D warnings`
Expected: PASS. Skip only if no Rust files changed.
