# Plan: Completed Workouts Cache via React Context

## Problem
`GET /api/completed-workouts?oldest=xxx&newest=xxx` is invoked every time a component is entered.
Two consumers:
- `useCalendarData` — calls `listActivities()` per week range during pagination
- `useWorkoutList` — calls `listActivities()` on mount for a 12-week lookback

No shared caching exists between them. Each navigation re-fetches the same date ranges.

## Solution
Create a `CompletedWorkoutsProvider` React context that caches activities by date range,
following the existing `SettingsProvider` pattern.

## Files to Create

### 1. `frontend/src/features/intervals/context/CompletedWorkoutsContext.tsx`
- Context provider with range-keyed cache (`Map<string, CachedRange>`)
- `CachedRange = { activities: IntervalActivity[], loadedAt: number }` — epoch millis timestamp
- TTL: 5 minutes for automatic staleness
- Exposed API:
  - `getActivitiesForRange(oldest, newest)` — returns cached or triggers fetch; throws on non-auth errors
  - `invalidateRange(oldest, newest)` — clear specific range and bump invalidation token
  - `invalidateAll()` — clear entire cache and bump global invalidation token
  - `isLoading: boolean` — derived from active inflight request count
  - `error: string | null` — global error state, cleared on cache hits
- In-flight request deduplication via `inflightRef` map
- Invalidation tokens prevent late responses from repopulating cleared cache entries
- Non-auth errors are rethrown after setting context error; auth errors redirect to `/`

### 2. `frontend/src/features/intervals/context/index.ts`
- Re-export provider and hook

## Files to Modify

### 3. `frontend/src/features/intervals/api/intervals.ts`
- No changes — `listActivities` stays as the raw fetch function
- The context will call it internally

### 4. `frontend/src/features/calendar/hooks/useCalendarData.ts`
- Replace direct `listActivities(apiBaseUrl, range)` call with `useCompletedWorkouts().getActivitiesForRange(oldest, newest)`
- Keep the parallel `Promise.all` with events and labels
- The context handles deduplication of in-flight requests for the same range

### 5. `frontend/src/features/coach/hooks/useWorkoutList.ts`
- Replace direct `listActivities(apiBaseUrl, range)` call with context method
- The 12-week range will likely hit cache if user navigated from calendar first

### 6. `frontend/src/App.tsx` (or wherever providers are composed)
- Wrap app with `<CompletedWorkoutsProvider apiBaseUrl={...}>`

## Implementation Details

### Cache Key
`${oldest}|${newest}` — simple string key from the query params

### Deduplication
Use a `Set<string>` of in-flight range keys to prevent duplicate concurrent requests
(same pattern as `inflightWeekKeysRef` in `useCalendarData`)

### Staleness Strategy
- TTL-based: ranges older than 5 minutes are considered stale
- Background refresh: return stale data immediately, refresh in background
- Manual invalidation: `invalidateAll()` after workout upload/update/delete

### Error Handling
- AuthenticationError → redirect to `/` (existing pattern)
- HttpError 422 → surface as `credentials-required`
- Other errors → surface in `error` state, don't crash

## Verification
- Calendar navigation doesn't re-fetch same ranges
- Coach workout list uses cached data when available
- Upload/delete invalidates relevant cache entries
- All existing tests pass
- No regression in loading states
