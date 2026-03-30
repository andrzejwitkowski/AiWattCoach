# Calendar Mini Chart Colors Design

**Goal:** Preserve planned workout zone colors in calendar day-cell mini charts without disturbing the existing activity and modal flows.

## Problem

`buildPlannedWorkoutBars()` already returns per-bar colors, but `CalendarDayCell` drops that information by converting the bars to raw heights before passing them to `CalendarMiniChart`. The chart currently accepts only `number[]` plus a single tone, so planned workout mini charts collapse to one color.

## Chosen Approach

Extend `CalendarMiniChart` to accept either:

- plain numeric bar heights for existing fallback usage
- structured bars with `{ height, color }` for planned and completed workouts

`CalendarDayCell` will pass structured bars when workout helpers provide them and continue using numeric fallback bars only when no structured workout data exists.

## Why This Approach

- Smallest correct change: the chart becomes slightly more flexible, but no new component is introduced.
- Backwards compatible within the frontend codebase: existing numeric callers continue to work.
- Keeps zone-color logic in `workoutDetails.ts`, where workout visualization data is already derived.

## Testing

- Add a focused `CalendarDayCell` test that renders a planned workout with multiple zone segments.
- Assert that multiple mini-chart bars render with distinct inline background colors.
- Keep existing day-cell tests unchanged unless the new prop shape requires minimal updates.
