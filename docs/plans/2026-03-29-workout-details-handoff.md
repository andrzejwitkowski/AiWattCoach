# Workout Details Handoff

## Scope in this branch

- calendar workout detail modal for planned and completed workouts
- workout doc parsing and actual-vs-planned matching
- mini chart color refinements
- planned event FIT download support
- completed-workout enrichment from Intervals activity sub-resources
- backend and frontend tests for the above

## Completed so far

- added `WorkoutDetailModal` and supporting `workoutDetails` helpers
- exposed enriched interval/activity data through the intervals REST flow
- added completed-only rendering for metrics, intervals, and chart bars
- enriched `get_activity()` via Intervals `/activity/{id}/intervals` and `/activity/{id}/streams`
- kept completed-activity enrichment fail-open when sub-resource calls fail
- added regression coverage across service, REST, adapter, frontend, and Mongo BSON persistence

## Important decisions

- use custom SVG rendering, not a chart library
- use Intervals API data already fetched/stored, not local FIT parsing for charts
- keep planned and completed rendering paths logically separate
- only verified event FIT download is implemented
- completed-activity FIT download remains unchanged

## Important docs

- `docs/plans/2026-03-29-workout-review-fixes-design.md`
- `docs/plans/2026-03-29-workout-review-fixes.md`
- `docs/plans/2026-03-29-calendar-mini-chart-colors-design.md`
- `docs/plans/2026-03-29-calendar-mini-chart-colors.md`
- `docs/plans/2026-03-29-workout-fit-download-design.md`
- `docs/plans/2026-03-29-workout-fit-download.md`
- `docs/plans/2026-03-29-completed-workout-enrichment-design.md`
- `docs/plans/2026-03-29-completed-workout-enrichment.md`

## Remaining likely follow-ups

- manually validate completed-only workouts against real Intervals data in the running app
- if real data is still sparse, evaluate `interval-stats` or other Intervals activity sub-resources
- keep planned-vs-actual behavior guarded while iterating on completed-only rendering

## Fresh verification before this handoff commit

- `bun run verify:all`
- targeted Mongo BSON persistence test for enriched activities
