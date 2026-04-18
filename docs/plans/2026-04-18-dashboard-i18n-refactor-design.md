# Dashboard I18n Refactor Design

**Goal:** Refactor the training load dashboard UI into smaller frontend-local pieces, localize dashboard and `/app` shell copy through the existing i18n system, and replace backend TSB threshold magic numbers with named constants without changing the existing API contract or dashboard behavior.

**Architecture:** Keep the current fetch and rendering flow unchanged: `AppHomePage` still loads the dashboard payload and renders `TrainingLoadReport`. Limit frontend work to the existing `/app` shell and dashboard feature files, and limit backend work to the existing training-load use-case module so the change stays behavioral-neutral.

**Tech Stack:** React, TypeScript, react-i18next, Tailwind CSS, inline SVG, Vitest, Testing Library, Rust

---

## Scope

- Keep routing, dashboard API calls, Zod parsing, and response shapes unchanged.
- Refactor `frontend/src/features/dashboard/components/TrainingLoadCharts.tsx` into smaller dashboard-local pieces.
- Localize dashboard copy including headings, legends, tooltips, chart labels, empty states, loading/error states, and ARIA narration.
- Extend the localization pass to the `/app` shell in `AuthenticatedLayout.tsx` for page titles, navigation labels, and the notifications button label.
- Replace backend TSB threshold literals in `classify_tsb_zone(...)` with named constants while preserving current thresholds and zone mapping.

## Design Direction

### Frontend Structure

- Keep `TrainingLoadCharts.tsx` as the orchestration layer only.
- Move chart-specific rendering into focused dashboard-local components such as:
  - a load chart section
  - a TSB chart section
  - small presentational pieces such as legend and metric blocks
- Move reusable chart math and display formatting into local helper files instead of keeping geometry, formatting, rendering, and copy in one component.

### Localization Strategy

- Use the existing `react-i18next` setup and extend the existing `frontend/src/locales/en/translation.json` and `frontend/src/locales/pl/translation.json` files.
- Localize both visible UI copy and accessibility-facing text:
  - chart ARIA labels
  - tooltip labels
  - insight-card copy
  - range-switch legend text
  - loading, error, and empty-state strings
  - `/app` navigation labels and page titles
- Replace hardcoded `en-US` date formatting with helpers that format using the active i18n language while preserving the existing invalid-date fallback.

### Data Flow

- Keep dashboard data loading in `AppHomePage` unchanged.
- In the refactored chart layer, compute shared axis, timeline, and point data once in `TrainingLoadCharts.tsx`, then pass only the required prepared data into the focused chart sections.
- Keep translation calls at render boundaries:
  - static labels via `t(...)`
  - dynamic tooltip and ARIA copy via interpolation
  - localized date and number formatting through small helpers

### Error Handling and Null Behavior

- Preserve current empty, loading, and error behavior, only localizing the current messages.
- Preserve existing chart null handling exactly:
  - gaps remain gaps
  - latest-point fallback stays intact
  - hover tooltips render only when both the hovered snapshot and plotted point exist
- Preserve display fallbacks:
  - invalid dates render as raw strings
  - null metrics render as `-`
- Keep backend threshold extraction as a naming cleanup only, with no logic change.

## Backend Cleanup

- Introduce named constants for TSB zone boundaries in `src/domain/training_load/use_cases.rs`.
- Keep the current semantics:
  - greater than `0.0` maps to `FreshnessPeak`
  - less than `-30.0` maps to `HighRisk`
  - everything else maps to `OptimalTraining`
- Do not move this logic across architectural boundaries or change DTO/domain shapes.

## Testing

- Update frontend tests in `frontend/src/pages/AppHomePage.test.tsx` to reflect the localized dashboard and existing regression scenarios under the default English locale.
- Add focused frontend coverage only if the refactor introduces a real gap.
- Keep backend training-load tests proving current TSB zone classification behavior; add or adjust coverage only if needed to make the named-threshold intent explicit.
- After implementation, run the relevant targeted frontend and Rust tests, then the broader verification required by the changed files before calling the work complete.
