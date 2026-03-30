# Workout Review Fixes Design

**Goal:** Fix the reviewed backend and frontend regressions in workout enrichment, stream handling, modal resilience, accessibility, and FIT-action scoping with minimal architectural cleanup.

## Root Causes

- Backend enrichment was placed in the REST adapter instead of the intervals service, making handlers too smart and coupling transport code to matching orchestration.
- Same-day activity matching shortlisted only already-matchable list items before hydration, so valid detailed activities could be skipped.
- Stream extraction converted null/non-numeric samples into zeros, corrupting actual workout data.
- The modal treated event and activity fetches as an all-or-nothing operation and could not render completed data from `event.actualWorkout` alone.
- Rest days became interactive buttons even though they have no action.
- The FIT button was shown in states where the current implementation does not match the user-facing intent.

## Chosen Approach

- Move event enrichment into `IntervalsService` behind a dedicated use-case method, keeping Axum handlers thin.
- For same-day candidate hydration, fetch the event, list same-day activities, then hydrate detailed activities until a valid best match is found instead of requiring the list payload to match first.
- Make stream extraction drop invalid samples instead of coercing them to zero.
- Make the modal tolerate partial fetch failure and render completed charts/metrics from `event.actualWorkout` when no detailed activity is available.
- Render rest days with a non-interactive container.
- Show the FIT action only in planned/event mode.

## Testing

- Add backend regression tests for:
  - same-day detailed hydration when listed activities are insufficient for initial matching
  - invalid/null stream values not being converted to zeros
- Add frontend regression tests for:
  - modal still rendering event data when activity lookup fails
  - completed modal rendering from `event.actualWorkout` without loaded `activity`
  - rest days not being rendered as interactive buttons
  - FIT button hidden for completed mode
