# Calendar Mini Chart Colors Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make calendar day-cell mini charts preserve per-zone planned workout colors.

**Architecture:** Keep workout bar derivation in `frontend/src/features/calendar/workoutDetails.ts`, extend `CalendarMiniChart` to render either numeric bars or structured bars, and update `CalendarDayCell` to pass structured bars when available. Verify the behavior with a focused React test before changing production code.

**Tech Stack:** React, TypeScript, Vitest, Testing Library

---

### Task 1: Add a failing UI test for colored planned bars

**Files:**
- Modify: `frontend/src/features/calendar/components/CalendarDayCell.test.tsx`

**Step 1: Write the failing test**

Add a test that renders a day with a planned workout whose `segments` contain at least two different `zoneId` values and asserts the mini-chart bars expose at least two different background colors.

**Step 2: Run test to verify it fails**

Run: `bun run --cwd frontend test frontend/src/features/calendar/components/CalendarDayCell.test.tsx`

Expected: the new assertion fails because the chart currently applies one tone color to every bar.

### Task 2: Add structured-bar support to the mini chart

**Files:**
- Modify: `frontend/src/features/calendar/components/CalendarMiniChart.tsx`
- Modify: `frontend/src/features/calendar/components/CalendarDayCell.tsx`

**Step 1: Write minimal implementation**

- Extend `CalendarMiniChart` to accept `Array<number | { height: number; color: string }>`.
- Use inline `backgroundColor` only for structured bars; keep the tone class for numeric bars.
- Update `CalendarDayCell` to pass structured workout bars directly and preserve numeric fallback bars only for synthetic fallback rendering.

**Step 2: Run the focused test to verify it passes**

Run: `bun run --cwd frontend test frontend/src/features/calendar/components/CalendarDayCell.test.tsx`

Expected: PASS.

### Task 3: Verify touched calendar tests

**Files:**
- Verify: `frontend/src/features/calendar/components/CalendarDayCell.test.tsx`
- Verify: `frontend/src/features/calendar/components/CalendarGrid.test.tsx`
- Verify: `frontend/src/features/calendar/workoutDetails.test.ts`

**Step 1: Run targeted verification**

Run: `bun run --cwd frontend test src/features/calendar/components/CalendarDayCell.test.tsx src/features/calendar/components/CalendarGrid.test.tsx src/features/calendar/workoutDetails.test.ts`

Expected: PASS with no new warnings or failures.
