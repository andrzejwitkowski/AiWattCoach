# Workout Detail Auto-Scroll Removal Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Remove workout detail interval-list auto-scroll triggered by chart hover/selection while preserving the existing highlight and selection behavior.

**Architecture:** Remove the `scrollIntoView` side effect from the modal panel that renders interval sections. Keep interval activation state unchanged so the chart, chips, and rows still stay synchronized visually. Cover the change with a focused interaction regression test.

**Tech Stack:** React, TypeScript, Vitest, Testing Library

---

### Task 1: Add a failing regression test for no auto-scroll

**Files:**
- Modify: `frontend/src/features/calendar/components/WorkoutDetailModal.interaction.test.tsx`

**Step 1: Write the failing test**

Add a focused test that renders the modal with interval rows, stubs `HTMLElement.prototype.scrollIntoView`, hovers the power chart into another ride, and asserts that `scrollIntoView` is not called while the active row/chip still updates.

**Step 2: Run test to verify it fails**

Run: `bun run --cwd frontend test src/features/calendar/components/WorkoutDetailModal.interaction.test.tsx`

Expected: FAIL because the current implementation still calls `scrollIntoView`.

### Task 2: Remove the auto-scroll side effect

**Files:**
- Modify: `frontend/src/features/calendar/components/WorkoutDetailModalPanels.tsx`

**Step 1: Write minimal implementation**

Delete the effect/ref logic that calls `scrollIntoView` for the active interval row. Leave the active row/chip calculation and rendering untouched.

**Step 2: Run test to verify it passes**

Run: `bun run --cwd frontend test src/features/calendar/components/WorkoutDetailModal.interaction.test.tsx`

Expected: PASS.

### Task 3: Verify nearby modal behavior still works

**Files:**
- No code changes expected
- Test: `frontend/src/features/calendar/components/WorkoutDetailModal.charts.test.tsx`

**Step 1: Run neighboring chart/modal coverage**

Run: `bun run --cwd frontend test src/features/calendar/components/WorkoutDetailModal.interaction.test.tsx src/features/calendar/components/WorkoutDetailModal.charts.test.tsx`

Expected: PASS.
