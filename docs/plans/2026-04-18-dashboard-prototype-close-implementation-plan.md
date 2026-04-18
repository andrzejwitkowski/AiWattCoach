# Dashboard Prototype-Close Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Restyle the existing training load dashboard to match issue `#103` more closely without changing its data contract or restoring the removed deload CTA.

**Architecture:** Keep `AppHomePage` and the dashboard API untouched, and implement the approved polish inside the existing `frontend/src/features/dashboard/components/*` files. Treat this as a presentation-only pass backed by the same dashboard response payload and the same route behavior.

**Tech Stack:** React, TypeScript, Tailwind CSS, inline SVG, Vitest, Testing Library

---

### Task 1: Lock the expected prototype-close UI in a failing test

**Files:**
- Modify: `frontend/src/pages/AppHomePage.test.tsx`

**Step 1: Write the failing test**

- Extend the existing happy-path dashboard test so it also expects the prototype-close structure and copy, for example:
  - `Understanding Form (TSB)`
  - `Coach Insight`
  - `Latest snapshot`

**Step 2: Run test to verify it fails**

Run: `bun run --cwd frontend test src/pages/AppHomePage.test.tsx`

Expected: FAIL because the current dashboard UI does not render the new structure yet.

### Task 2: Restyle the dashboard report shell and range switch

**Files:**
- Modify: `frontend/src/features/dashboard/components/TrainingLoadReport.tsx`
- Modify: `frontend/src/features/dashboard/components/TrainingLoadRangeSwitch.tsx`

**Step 1: Write minimal implementation**

- Update the header layout and copy to feel closer to the prototype.
- Keep the same `onRangeChange` behavior.
- Restyle the segmented control only; do not change labels or state semantics.

**Step 2: Run test to verify progress**

Run: `bun run --cwd frontend test src/pages/AppHomePage.test.tsx`

Expected: still failing until the chart and insight components are updated.

### Task 3: Restyle the chart cards with clearer axes, zones, and latest-point context

**Files:**
- Modify: `frontend/src/features/dashboard/components/TrainingLoadCharts.tsx`

**Step 1: Write minimal implementation**

- Keep the current SVG line rendering.
- Add helpers for date labels and latest-point context.
- Add visible y-axis labels, bottom timeline labels, and a highlighted latest snapshot marker.
- Strengthen TSB zone backgrounds and labels.

**Step 2: Run test to verify progress**

Run: `bun run --cwd frontend test src/pages/AppHomePage.test.tsx`

Expected: may still fail until the right-hand panel is updated.

### Task 4: Replace the summary card feel with the prototype-close insight/explainer panel

**Files:**
- Modify: `frontend/src/features/dashboard/components/TrainingLoadInsightCard.tsx`

**Step 1: Write minimal implementation**

- Render a TSB explanation block with three zones.
- Render a darker coach insight block using existing summary data.
- Keep the panel informational only and do not add any CTA.

**Step 2: Run test to verify it passes**

Run: `bun run --cwd frontend test src/pages/AppHomePage.test.tsx`

Expected: PASS.

### Task 5: Run targeted and broader verification

**Files:**
- No code changes expected

**Step 1: Run targeted frontend tests**

Run: `bun run --cwd frontend test src/pages/AppHomePage.test.tsx src/features/dashboard/api/dashboard.test.ts`

Expected: PASS.

**Step 2: Run broader frontend verification**

Run:
- `bun run --cwd frontend test`
- `bun run --cwd frontend build`

Expected: PASS.

### Task 6: Run graph refresh and the required review loop

**Files:**
- No code changes expected unless review finds issues

**Step 1: Refresh graphify**

Run: `bash ./scripts/rebuild_graphify.sh`

**Step 2: Perform four review iterations**

- In each iteration, review the changed frontend files in three passes:
  - strict reviewer
  - very strict reviewer
  - nitpicker
- Convert confirmed findings into fixes and rerun the most relevant verification after each iteration.
