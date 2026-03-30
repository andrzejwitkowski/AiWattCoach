# Workout FIT Download Design

**Goal:** Expose the existing event FIT download capability from the workout detail modal without adding unverified completed-activity export behavior.

## Problem

The backend already serves event FIT downloads and the frontend already has a typed `downloadFit()` API helper, but there is no user-facing action in the workout detail UI. That leaves the capability implemented but unreachable in the main workout flow.

## Chosen Approach

Add a `Download FIT` action to `WorkoutDetailModal` when a selected workout has an event id. The modal will call the existing `downloadFit(apiBaseUrl, eventId)` helper, create a temporary object URL from the returned bytes, and trigger a browser download named `event-{id}.fit`.

## Why This Approach

- Minimal diff: no backend changes, no new route, no new data shape.
- Honest scope: only planned/event FIT export is wired because completed-activity FIT export is still unverified.
- Keeps export close to the detailed workout context instead of cluttering the calendar grid.

## Testing

- Add a `WorkoutDetailModal` test that mocks `downloadFit` and verifies clicking the button triggers the download helper for the selected event.
- Mock browser URL and anchor behavior only as needed to keep the test focused on user-visible behavior.
