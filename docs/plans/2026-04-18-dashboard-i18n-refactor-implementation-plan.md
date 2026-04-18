# Dashboard I18n Refactor Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Refactor the training load dashboard into smaller pieces, localize dashboard and `/app` shell copy, and replace backend TSB threshold magic numbers with named constants without changing the dashboard API or behavior.

**Architecture:** Keep the current page and API flow intact: `AppHomePage` still loads the dashboard payload and `TrainingLoadReport` still renders the dashboard experience. Implement the frontend work inside the existing dashboard feature and `/app` layout, and keep the backend change limited to the training-load use-case module so the pass remains structurally cleaner but behaviorally neutral.

**Tech Stack:** React, TypeScript, react-i18next, Tailwind CSS, inline SVG, Vitest, Testing Library, Rust

---

## Task 1: Lock the localized dashboard and shell expectations in tests

**Files:**
- Modify: `frontend/src/pages/AppHomePage.test.tsx`
- Modify: `frontend/src/App.test.tsx`

**Step 1: Write the failing dashboard assertions**

- Update `frontend/src/pages/AppHomePage.test.tsx` so the existing dashboard tests still cover:
  - loading state
  - range switching
  - keyboard range changes
  - TSB hover tooltip behavior
  - latest-load marker fallback when CTL is null
  - empty state
- Change the assertions to rely on the localized English copy that the implementation will render instead of current hardcoded strings embedded in component code.

**Step 2: Write the failing `/app` shell assertions**

- Update `frontend/src/App.test.tsx` or the most relevant existing app-shell tests so they assert localized nav/page-title behavior for `/app` paths where appropriate.

**Step 3: Run the focused frontend tests to verify failure**

Run:
- `bun run --cwd frontend test src/pages/AppHomePage.test.tsx src/App.test.tsx`

Expected: FAIL because the dashboard and `/app` shell do not use translation keys yet.

## Task 2: Add the translation keys needed for dashboard and `/app`

**Files:**
- Modify: `frontend/src/locales/en/translation.json`
- Modify: `frontend/src/locales/pl/translation.json`

**Step 1: Add `/app` shell translation keys**

- Add keys for:
  - dashboard nav label
  - races nav label if needed for consistency with shell usage
  - page titles used by `AuthenticatedLayout`
  - notifications button label
  - brand subtitle if it is meant to be localized

**Step 2: Add dashboard translation keys**

- Add keys for:
  - dashboard report header copy
  - range switch labels and legend
  - empty, loading, and error states
  - chart legend labels
  - tooltip labels
  - chart zone labels
  - chart ARIA narration
  - insight-card headings, descriptions, and metric labels
  - coach-insight headlines and detail text interpolation

**Step 3: Keep the key shape feature-local and readable**

- Group the new keys under a dedicated top-level namespace such as `dashboard` and extend `nav` only where the shell already uses it.

## Task 3: Localize the `/app` shell and dashboard shell states

**Files:**
- Modify: `frontend/src/app/AuthenticatedLayout.tsx`
- Modify: `frontend/src/pages/AppHomePage.tsx`
- Modify: `frontend/src/features/dashboard/components/TrainingLoadReport.tsx`
- Modify: `frontend/src/features/dashboard/components/TrainingLoadRangeSwitch.tsx`
- Modify: `frontend/src/features/dashboard/components/TrainingLoadEmptyState.tsx`

**Step 1: Write the minimal implementation for `/app` shell localization**

- Add `useTranslation()` to `AuthenticatedLayout.tsx`.
- Replace hardcoded page-title and nav-label strings with `t(...)` lookups.
- Localize the notifications button `aria-label`.

**Step 2: Write the minimal implementation for page-level dashboard states**

- Add `useTranslation()` to `AppHomePage.tsx`.
- Localize the loading and error states while preserving current fetch and fallback behavior.

**Step 3: Localize dashboard shell components**

- Add `useTranslation()` to `TrainingLoadReport.tsx`, `TrainingLoadRangeSwitch.tsx`, and `TrainingLoadEmptyState.tsx`.
- Replace the current hardcoded header, segmented-control, and empty-state strings with translation keys.
- Keep behavior and layout semantics unchanged.

**Step 4: Run focused frontend tests to verify progress**

Run:
- `bun run --cwd frontend test src/pages/AppHomePage.test.tsx src/App.test.tsx`

Expected: some tests may still fail until chart and insight-card localization is implemented.

## Task 4: Split chart helpers out of `TrainingLoadCharts.tsx`

**Files:**
- Create: `frontend/src/features/dashboard/components/trainingLoadChartUtils.ts`
- Create: `frontend/src/features/dashboard/components/trainingLoadFormatters.ts`
- Modify: `frontend/src/features/dashboard/components/TrainingLoadCharts.tsx`

**Step 1: Move pure chart math into a helper module**

- Extract pure helpers from `TrainingLoadCharts.tsx` into `trainingLoadChartUtils.ts`, such as:
  - clamp
  - point building
  - line and area path building
  - latest-point and hover-index helpers
  - axis/timeline helper builders

**Step 2: Move formatting helpers into a dedicated formatter module**

- Extract formatting logic into `trainingLoadFormatters.ts`, including:
  - metric fallback formatting
  - localized short-date formatting
  - localized window-date formatting if it is reused
- Make the formatter helpers accept the active language so they no longer hardcode `en-US`.

**Step 3: Keep `TrainingLoadCharts.tsx` compiling with the extracted helpers**

- Update imports and preserve current rendering behavior before any deeper component split.

**Step 4: Run the dashboard test file**

Run:
- `bun run --cwd frontend test src/pages/AppHomePage.test.tsx`

Expected: either FAIL on remaining localization work or PASS on unchanged regressions.

## Task 5: Split the load and TSB chart rendering into focused components

**Files:**
- Create: `frontend/src/features/dashboard/components/TrainingLoadLoadChart.tsx`
- Create: `frontend/src/features/dashboard/components/TrainingLoadTsbChart.tsx`
- Modify: `frontend/src/features/dashboard/components/TrainingLoadCharts.tsx`

**Step 1: Extract the load chart section**

- Move the fitness/fatigue card rendering into `TrainingLoadLoadChart.tsx`.
- Pass prepared props only:
  - axis labels
  - timeline labels
  - latest and hovered snapshot state
  - line paths and marker data
  - handlers and translated copy

**Step 2: Extract the TSB chart section**

- Move the form card rendering into `TrainingLoadTsbChart.tsx`.
- Pass prepared props only:
  - axis labels
  - timeline labels
  - zone boundary layout values
  - latest and hovered snapshot state
  - line and area path data
  - handlers and translated copy

**Step 3: Reduce `TrainingLoadCharts.tsx` to orchestration**

- Keep it responsible for:
  - deriving series arrays from `report`
  - computing shared chart data once
  - owning hover state
  - passing props into the two focused chart components

## Task 6: Localize chart and insight-card copy completely

**Files:**
- Modify: `frontend/src/features/dashboard/components/TrainingLoadCharts.tsx`
- Modify: `frontend/src/features/dashboard/components/TrainingLoadLoadChart.tsx`
- Modify: `frontend/src/features/dashboard/components/TrainingLoadTsbChart.tsx`
- Modify: `frontend/src/features/dashboard/components/TrainingLoadInsightCard.tsx`
- Modify: `frontend/src/features/dashboard/components/trainingLoadFormatters.ts`

**Step 1: Localize chart labels and tooltips**

- Add `useTranslation()` at the chart render boundary.
- Replace hardcoded legend labels, snapshot labels, zone labels, and metric labels with translation keys.
- Build chart ARIA labels with translation interpolation instead of inline English strings.

**Step 2: Localize insight-card copy**

- Replace all hardcoded insight/explainer text with translation keys.
- Keep the current TSB-zone-dependent headline logic, but drive the actual displayed strings from localized keys.
- Keep delta-dependent detail text behavior unchanged.

**Step 3: Localize date formatting**

- Make date formatting use the active i18n language while preserving the current raw-string fallback for invalid dates.

**Step 4: Run the focused frontend tests**

Run:
- `bun run --cwd frontend test src/pages/AppHomePage.test.tsx src/App.test.tsx`

Expected: PASS.

## Task 7: Replace backend TSB magic numbers with named constants

**Files:**
- Modify: `src/domain/training_load/use_cases.rs`
- Modify: `src/domain/training_load/tests.rs` if a small assertion update is needed

**Step 1: Write the failing or clarifying backend assertion if needed**

- If the existing tests do not make the threshold intent obvious enough, add a small focused assertion around the current threshold behavior.

**Step 2: Introduce named constants**

- Add private constants near `classify_tsb_zone(...)`, for example:
  - freshness threshold
  - high-risk threshold
- Replace inline `0.0` and `-30.0` with those constants.

**Step 3: Run the focused Rust test**

Run:
- `cargo test training_load -- --nocapture`

Expected: PASS.

## Task 8: Run targeted verification for touched areas

**Files:**
- No code changes expected

**Step 1: Run targeted frontend tests**

Run:
- `bun run --cwd frontend test src/pages/AppHomePage.test.tsx src/App.test.tsx src/features/dashboard/api/dashboard.test.ts`

Expected: PASS.

**Step 2: Run targeted Rust verification**

Run:
- `cargo fmt --all --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test training_load -- --nocapture`

Expected: PASS.

## Task 9: Run broader verification and graph refresh

**Files:**
- No code changes expected unless verification or review finds issues

**Step 1: Run broader frontend verification**

Run:
- `bun run --cwd frontend test`
- `bun run --cwd frontend build`

Expected: PASS.

**Step 2: Refresh graphify**

Run:
- `bash ./scripts/rebuild_graphify.sh`

Expected: PASS.

## Task 10: Perform the required review loop and final verification pass

**Files:**
- No code changes expected unless review finds issues

**Step 1: Perform four review iterations**

- In each iteration, review the changed files in three passes:
  - strict reviewer
  - very strict reviewer
  - nitpicker
- Convert confirmed findings into minimal fixes.

**Step 2: Rerun the most relevant verification after each fix set**

- Use the smallest command set that proves the affected behavior still works.

**Step 3: Finish with the final required checks**

Run the final relevant verification again after the last review iteration and read the output before claiming the work complete.
