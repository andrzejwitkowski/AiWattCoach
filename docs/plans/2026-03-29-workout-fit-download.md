# Workout FIT Download Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add an event FIT download action to the workout detail modal.

**Architecture:** Reuse the existing frontend `downloadFit()` helper from the modal, keep the button scoped to workouts that have an event id, and perform the browser download client-side using a temporary object URL. Verify the behavior first with a focused modal test, then run the touched modal and intervals API tests.

**Tech Stack:** React, TypeScript, Vitest, Testing Library

---

### Task 1: Add a failing modal test for the FIT download action

**Files:**
- Modify: `frontend/src/features/calendar/components/WorkoutDetailModal.test.tsx`

**Step 1: Write the failing test**

Add a test that renders a modal with an event selection, clicks a `Download FIT` button, and expects `downloadFit('', eventId)` to be called.

**Step 2: Run test to verify it fails**

Run: `bun run --cwd frontend test src/features/calendar/components/WorkoutDetailModal.test.tsx`

Expected: FAIL because the button and handler do not exist yet.

### Task 2: Implement the modal action

**Files:**
- Modify: `frontend/src/features/calendar/components/WorkoutDetailModal.tsx`

**Step 1: Write minimal implementation**

- Import `downloadFit`.
- Add a secondary action button when `event?.id` exists.
- Track a minimal `downloadingFit` state.
- Call `downloadFit(apiBaseUrl, event.id)`, create a blob/object URL, click a temporary anchor, then revoke the URL.

**Step 2: Run the focused test to verify it passes**

Run: `bun run --cwd frontend test src/features/calendar/components/WorkoutDetailModal.test.tsx`

Expected: PASS.

### Task 3: Verify touched tests

**Files:**
- Verify: `frontend/src/features/calendar/components/WorkoutDetailModal.test.tsx`
- Verify: `frontend/src/features/intervals/api/intervals.test.ts`

**Step 1: Run targeted verification**

Run: `bun run --cwd frontend test src/features/calendar/components/WorkoutDetailModal.test.tsx src/features/intervals/api/intervals.test.ts`

Expected: PASS with no new failures.
