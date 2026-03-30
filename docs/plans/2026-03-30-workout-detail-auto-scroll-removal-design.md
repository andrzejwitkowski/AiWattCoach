# Workout Detail Auto-Scroll Removal Design

The workout detail modal currently auto-scrolls the interval list when the power chart hover or selection changes. The requested change is narrower: stop the list from jumping to `Ride X`, but keep the existing chart-to-interval highlight and selection behavior.

Chosen approach:
- remove only the `scrollIntoView` side effect in `frontend/src/features/calendar/components/WorkoutDetailModalPanels.tsx`
- keep hover-driven active row/chip state and chart overlays intact
- update the interaction tests so they no longer expect list scrolling behavior

Why this approach:
- smallest behavior change
- preserves the useful chart/interval synchronization
- avoids redesigning selection semantics that were not requested

Verification plan:
- add or update a focused modal interaction test that fails if `scrollIntoView` is called from chart hover/selection
- run the focused modal interaction/frontend tests after the change
