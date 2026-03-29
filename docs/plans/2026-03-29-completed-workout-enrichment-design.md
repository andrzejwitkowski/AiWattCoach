# Completed Workout Enrichment Design

**Goal:** Enrich completed-workout activity details for Intervals.icu and Strava-imported activities so the calendar modal can render real metrics, streams, and interval sections without relying on planned events.

## Problem

- The current completed-workout modal fetches `GET /api/intervals/activities/{id}` and renders whatever the backend returns.
- For Strava-imported activities, the Intervals.icu base activity endpoint can return sparse stub objects with missing title, duration, metrics, and stream metadata.
- Our backend currently relies on:
  - `GET /api/v1/athlete/{athlete_id}/activities`
  - `GET /api/v1/activity/{id}?intervals=true`
  - `GET /api/v1/activity/{id}/streams?types=...`
- That is sufficient for richer native Intervals activities, but not for sparse imported completed workouts.

## Chosen Approach

- Keep calendar list loading lightweight and unchanged.
- Enrich only detailed completed-activity fetches in the backend.
- For `get_activity(user_id, activity_id)`:
  - fetch the base activity from `GET /api/v1/activity/{id}`
  - fetch dedicated interval data from `GET /api/v1/activity/{id}/intervals`
  - fetch streams from `GET /api/v1/activity/{id}/streams` with `includeDefaults=true`
  - merge those responses into one `Activity` domain object
  - persist the enriched activity in Mongo before returning it
- Use the enriched `activity.details.intervals` and stream data in the existing completed-workout modal instead of inventing fake event data.

## Why This Approach

- It matches the Intervals API docs, which explicitly warn that base activity endpoints can return empty stub objects for Strava activities.
- It keeps planned-event and planned-vs-actual behavior unchanged.
- It minimizes cost by enriching only the detailed activity path, not every calendar list item.
- It preserves the current architecture: enrichment stays in the adapter/service boundary, and the frontend consumes a richer existing DTO rather than a new transport shape.

## Data Flow

1. Calendar day click selects a completed activity.
2. Frontend calls `GET /api/intervals/activities/{activity_id}`.
3. Backend `IntervalsUseCases::get_activity(...)`:
   - resolves user credentials
   - fetches the base activity from Intervals
   - fetches dedicated intervals and streams sub-resources
   - merges the results into one `Activity`
   - upserts the merged activity to `intervals_activities`
   - returns the merged activity DTO
4. Frontend modal renders:
   - title and summary metrics from enriched activity fields
   - completed interval sections from `activity.details.intervals`
   - charts from `activity.details.streams`

## Merge Rules

- Base activity remains the source of identity and top-level metadata.
- Dedicated `/intervals` data replaces or fills `activity.details.intervals` and `activity.details.interval_groups`.
- `/streams?includeDefaults=true` replaces or fills `activity.details.streams`.
- If a sub-resource request fails, keep the base activity data and continue returning a partial result.
- Do not fail the entire completed-workout modal when one enrichment sub-call is unavailable.

## Frontend Behavior

- Completed-only workouts should render real interval sections when `activity.details.intervals` is populated.
- Existing completed metrics/cards should continue to read from activity metrics.
- Existing chart helpers should use enriched stream data when present.
- No synthetic event or fake planned-workout structure should be introduced for completed-only activities.

## Testing

- Backend adapter tests:
  - sparse base activity + rich `/intervals` + rich `/streams` => merged `Activity` contains intervals and streams
  - base activity success + sub-resource failure => partial result still returned
- Backend service/integration tests:
  - `get_activity(...)` persists enriched activity payload to Mongo
  - `list_activities(...)` remains lightweight and unchanged
- Frontend tests:
  - completed modal renders interval sections from enriched `activity.details.intervals`
  - completed modal renders charts/metrics from enriched stream and metric payloads
