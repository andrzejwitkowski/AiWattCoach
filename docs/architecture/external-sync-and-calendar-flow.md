# External Sync And Calendar Flow

This document explains the current code flow from external Intervals.icu data into local domain documents, sync metadata, calendar projections, REST endpoints, and the frontend calendar.

It describes the code as it exists today, not the intended future architecture.

## Scope

This covers:

- background polling of Intervals calendar events and completed workouts
- normalization into internal import commands
- canonical local persistence
- sync metadata and dedup state
- `calendar_entry_views` projection refresh
- REST endpoints used by the calendar frontend
- how the frontend currently composes calendar data

It does not cover every Intervals mutation path in detail. It focuses on the read/import side and the pieces that affect the calendar UI.

## Runtime Wiring

The main runtime wiring lives in `src/main.rs`.

Relevant services are assembled there:

- `ProviderPollingService` for background polling
- `ExternalImportService` for canonical import persistence
- `CalendarEntryViewRefreshService` for rebuilding `calendar_entry_views`
- `CalendarService` for `/api/calendar/events`
- `CalendarLabelsService` for `/api/calendar/labels`
- `IntervalsService` for `/api/intervals/events` and `/api/intervals/activities`

The startup path also:

- ensures Mongo indexes for the involved collections
- reconciles poll states with current settings through `reconcile_intervals_poll_states()` in `src/main_support.rs`
- starts the background polling loop with `spawn_provider_polling_loop(...)`

## Main Collections

These collections are the important durable pieces for this flow.

| Collection | Role |
| --- | --- |
| `provider_poll_states` | Durable cursor and retry state for each user/provider/stream pair |
| `planned_workouts` | Canonical imported planned workouts from external calendar events |
| `completed_workouts` | Canonical imported completed workouts |
| `races` | Canonical imported race entities |
| `special_days` | Canonical imported special-day entities |
| `external_observations` | Records which external object was seen and which canonical entity it mapped to |
| `external_sync_states` | Per-canonical-entity sync status against the provider |
| `planned_workout_syncs` | Sync state for projected training-plan workouts pushed to Intervals |
| `calendar_entry_views` | Projected calendar read model built from canonical local state |
| `activities` | Locally cached Intervals activities used by the Intervals service |
| `training_plan_projected_days` | Predicted training-plan days used by the calendar service |
| `training_plan_snapshots` | Snapshot metadata used with projected training-plan days |

## High-Level Flow

```mermaid
flowchart TD
    A[provider_poll_states] --> B[ProviderPollingService]
    B --> C[Intervals list_events / list_activities]
    C --> D[import_mapping.rs]
    D --> E[ExternalImportService]
    E --> F[planned_workouts / completed_workouts / races / special_days]
    E --> G[external_observations]
    E --> H[external_sync_states]
    E --> I[CalendarEntryViewRefreshService]
    I --> J[calendar_entry_views]
    J --> K[/api/calendar/labels]
    J --> L[hidden linked race event ids]
    M[training_plan_projected_days + planned_workout_syncs] --> N[/api/calendar/events]
    O[IntervalsService list_activities] --> P[/api/intervals/activities]
    Q[frontend useCalendarData] --> N
    Q --> P
    Q --> K
```

## 1. Poll State Bootstrap

At startup, `reconcile_intervals_poll_states()` in `src/main_support.rs` compares user settings with existing `provider_poll_states`.

For each user with Intervals polling enabled, it makes sure there are two poll streams:

- `ProviderPollStream::Calendar`
- `ProviderPollStream::CompletedWorkouts`

If credentials were changed or polling needs to be reset, the poll state is recreated with:

- no cursor
- no error/backoff
- `next_due_at_epoch_seconds = now`

If polling should be inactive, the state is effectively parked by setting `next_due_at_epoch_seconds = i64::MAX`.

## 2. Background Polling Loop

`spawn_provider_polling_loop()` in `src/config/provider_polling/mod.rs` runs `poll_due_once()` every minute.

`poll_due_once()`:

1. loads all due states from `provider_poll_states`
2. calls `process_due_state()` for each

The important reliability rule is already applied here:

- `process_due_state()` first persists `mark_attempted(...)`
- only after that does it call the external provider

That means the app has durable local evidence that a poll attempt started before any Intervals call happens.

## 3. Polling The Intervals Calendar Stream

For `ProviderPollStream::Calendar`, `ProviderPollingService` does this:

1. loads Intervals credentials from settings
2. computes the date range with `calendar_poll_range()`
3. calls `intervals_api.list_events(...)`
4. maps each Intervals event through `map_event_to_import_command()`
5. imports each mapped command with `ExternalImportService::import(...)`
6. advances the stream cursor with `advance_calendar_cursor(...)`
7. if this was the initial sync, refreshes the full range in `calendar_entry_views`

The normalization step in `src/adapters/intervals_icu/import_mapping.rs` maps Intervals event categories like this:

- `Workout` -> `ExternalImportCommand::UpsertPlannedWorkout`
- `Race`, `RaceA`, `RaceB`, `RaceC` -> `ExternalImportCommand::UpsertRace`
- `Note`, `Target`, `Season`, `Other` -> `ExternalImportCommand::UpsertSpecialDay`

So the external calendar stream is treated as a source of local canonical planned workouts, races, and special days.

## 4. Polling The Completed Workouts Stream

For `ProviderPollStream::CompletedWorkouts`, `ProviderPollingService` does this:

1. loads Intervals credentials
2. computes the date range with `completed_workout_poll_range()`
3. calls `intervals_api.list_activities(...)`
4. maps each activity through `map_activity_to_import_command()`
5. imports each one with `ExternalImportService::import(...)`
6. advances the stream cursor with `advance_completed_workout_cursor(...)`
7. if this was the initial sync, refreshes the full range in `calendar_entry_views`

The activity mapper builds `ExternalImportCommand::UpsertCompletedWorkout` and converts the Intervals payload into the internal `CompletedWorkout` model.

## 5. Canonical Import Persistence

`ExternalImportService` in `src/domain/external_sync/import/mod.rs` is the central canonical import path.

Each import variant first upserts the canonical entity into its local repository:

- planned workout -> `planned_workouts`
- completed workout -> `completed_workouts`
- race -> `races`
- special day -> `special_days`

After the canonical entity is stored, the shared import finalization path in `src/domain/external_sync/import/import_outcome.rs` does two things:

1. persists sync metadata
2. triggers a best-effort `calendar_entry_views` refresh for the affected date

The ordering is important:

- canonical entity is upserted first
- sync metadata is persisted next
- calendar view refresh happens last and is best effort

If the refresh fails, the import still succeeds and a warning is logged.

## 6. Sync Metadata

Each import persists two related but different records.

### `external_observations`

This records that a specific external object was seen and what canonical entity it mapped to.

Stored fields include:

- `provider`
- `external_object_kind`
- `external_id`
- `canonical_entity`
- `normalized_payload_hash`
- `dedup_key`
- `observed_at_epoch_seconds`

This is the durable trace from remote object to local canonical entity.

### `external_sync_states`

This records the latest sync state for a canonical entity against a provider.

For imports, the state is loaded or created and then `mark_synced(...)` is persisted with:

- the provider external id
- the synced payload hash
- the sync timestamp

This is what later lets projected views show provider sync status for canonical entities like races and planned workouts.

## 7. Completed Workout Dedup

Completed workout imports have an extra dedup step in `src/domain/external_sync/import/completed_workout_dedup.rs`.

The flow is:

1. build a dedup key from a minute bucket of `start_date_local`
2. include rounded duration
3. include rounded distance
4. include a bucket of available stream types
5. look up matching observations in `external_observations`

If exactly one canonical completed workout is found for that dedup key, the incoming workout is merged into the existing canonical workout instead of creating a second one.

If multiple canonical matches are found, the import fails as ambiguous.

This means dedup is not based only on external ids. It can also recover when the same completed workout arrives through slightly different provider identities.

## 8. Calendar Entry View Projection

`CalendarEntryViewRefreshService` in `src/domain/calendar_view/refresh.rs` rebuilds the `calendar_entry_views` read model for a user and date range.

For the requested range it loads:

- planned workouts from `PlannedWorkoutRepository`
- completed workouts from `CompletedWorkoutRepository`
- planned workout sync records from `PlannedWorkoutSyncRepository`
- races from `RaceRepository`
- special days from `SpecialDayRepository`
- external sync state from `ExternalSyncStateRepository`

It then projects these into `CalendarEntryView` rows and replaces the stored range in `calendar_entry_views`.

The projections are built in `src/domain/calendar_view/projection.rs`:

- planned workouts become `planned:*` entries
- completed workouts become `completed:*` entries
- races become `race:*` entries
- special days become `special:*` entries

Important detail:

- race entries include sync metadata and a structured description payload
- that structured race description is later parsed back out by the calendar label source

## 9. What `calendar_entry_views` Is Used For Today

`calendar_entry_views` exists as a local calendar read model, but the current frontend does not read it directly.

Today it is used mainly for two backend purposes:

1. `CalendarLabelsService` reads race labels from it through `MongoCalendarEntryViewCalendarSource`
2. `CalendarService` asks the same source for linked race event ids that should be hidden from the live Intervals calendar event list

That means:

- imported races do affect the current frontend calendar
- imported planned workouts, completed workouts, and special days are projected locally, but are not directly rendered by the current calendar frontend

This is the most important current-state caveat in this area.

## 10. `/api/calendar/events` Is A Live Composition Path

`/api/calendar/events` is served by `CalendarService::list_events()` in `src/domain/calendar/service.rs`.

It does not read `calendar_entry_views`.

Instead it composes the response from:

- live Intervals calendar events from `IntervalsService.list_events(...)`
- projected training-plan days from `training_plan_projected_days`
- sync state from `planned_workout_syncs`
- hidden linked event ids from the race label source backed by `calendar_entry_views`

The result is:

- predicted workouts appear as synthetic calendar events
- synced projected workouts can hide their linked Intervals event
- race-linked Intervals events can also be hidden
- plain Intervals events are otherwise returned directly

So the current calendar event feed is a hybrid of remote Intervals data and local predicted workout data.

## 11. Planned Workout Syncs

`planned_workout_syncs` belong to the training-plan projection flow, not the external import flow, but they matter to the calendar UI.

When `CalendarService::sync_planned_workout()` pushes a projected workout to Intervals, it:

1. finds the projected training-plan day
2. creates or updates a `planned_workout_syncs` record as `Pending`
3. creates or updates the Intervals event
4. marks the sync record as `Synced` or `Failed`
5. marks the calendar poll due soon
6. refreshes the affected date in `calendar_entry_views`

These sync records are used by `/api/calendar/events` to show predicted workout sync status and to hide the linked remote Intervals event when appropriate.

## 12. `/api/calendar/labels` Comes From `calendar_entry_views`

`/api/calendar/labels` is served by `CalendarLabelsService`.

The Mongo source in `src/adapters/mongo/calendar_entry_view_calendar.rs` queries `calendar_entry_views` for `entry_kind = "race"` and maps those rows into label DTOs.

It also extracts:

- race identity
- discipline
- priority
- distance
- sync status
- linked Intervals event id

So the label endpoint is the main current frontend-facing path that depends directly on imported canonical race data.

## 13. `/api/intervals/activities` Is Live Intervals Data

The calendar frontend also calls `/api/intervals/activities`.

That path goes through `IntervalsService::list_activities()` in `src/domain/intervals/service/activities.rs`.

The current behavior is:

1. load credentials
2. call the Intervals API live with `list_activities(...)`
3. best-effort cache the returned activities in the local `activities` collection
4. best-effort refresh `calendar_entry_views` for the requested range
5. return the live activities to the caller

Important detail:

- this endpoint is not reading canonical `completed_workouts`
- it is reading live provider activities and caching them locally

So there are currently two different local notions of completed workout data:

- canonical imported completed workouts in `completed_workouts`
- cached live Intervals activities in `activities`

The frontend calendar currently uses the second one.

## 14. Frontend Calendar Consumption

The main frontend entry point is `frontend/src/features/calendar/hooks/useCalendarData.ts`.

For each visible range it fetches three sources in parallel:

- `listCalendarEvents(...)` -> `/api/calendar/events`
- `listActivities(...)` -> `/api/intervals/activities`
- `listCalendarLabels(...)` -> `/api/calendar/labels`

Then it groups everything by date and builds `CalendarWeek` values.

Current frontend meaning of those three sources:

- events: Intervals calendar events plus predicted workouts
- activities: completed activity data used for weekly totals and activity detail
- labels: race labels from `calendar_entry_views`

The weekly summary numbers in `useCalendarData()` are computed from `activities`, not from `calendar_entry_views`.

## 15. What Reaches The Frontend Today

Here is the honest current state for each canonical imported entity kind.

| Canonical entity | Stored locally | Projected into `calendar_entry_views` | Directly used by current calendar frontend |
| --- | --- | --- | --- |
| Planned workout import | Yes | Yes | No |
| Completed workout import | Yes | Yes | No |
| Race import | Yes | Yes | Yes, via labels and hidden linked events |
| Special day import | Yes | Yes | No |

Separately:

- live Intervals calendar events are used directly by `/api/calendar/events`
- live Intervals activities are used directly by `/api/intervals/activities`
- predicted training-plan workouts are used directly by `/api/calendar/events`

## 16. End-To-End Summary

The current system has two overlapping calendar data paths.

### Path A: canonical import path

This is the durable internal model:

- provider polling
- normalization into import commands
- canonical writes
- observation/sync metadata writes
- `calendar_entry_views` refresh

This path is the main durable source of truth for imported domain entities.

### Path B: active calendar frontend path

This is the UI path used today:

- `/api/calendar/events` for Intervals events plus predicted workouts
- `/api/intervals/activities` for live activity data
- `/api/calendar/labels` for race labels from `calendar_entry_views`

So the current frontend does not consume the full canonical calendar projection directly. It only consumes the race-label slice of that projection, while the rest of the UI still depends on live Intervals reads and training-plan projections.

## File Map

The most relevant files for this flow are:

- `src/main.rs`
- `src/main_support.rs`
- `src/config/provider_polling/mod.rs`
- `src/adapters/intervals_icu/import_mapping.rs`
- `src/domain/external_sync/import/mod.rs`
- `src/domain/external_sync/import/import_outcome.rs`
- `src/domain/external_sync/import/completed_workout_dedup.rs`
- `src/domain/calendar_view/refresh.rs`
- `src/domain/calendar_view/projection.rs`
- `src/domain/calendar/service.rs`
- `src/domain/calendar_labels/service.rs`
- `src/adapters/mongo/provider_poll_states.rs`
- `src/adapters/mongo/external_observations.rs`
- `src/adapters/mongo/external_sync_states.rs`
- `src/adapters/mongo/calendar_entry_views.rs`
- `src/adapters/mongo/calendar_entry_view_calendar.rs`
- `src/adapters/mongo/planned_workout_syncs.rs`
- `frontend/src/features/calendar/hooks/useCalendarData.ts`
- `frontend/src/features/intervals/api/intervals.ts`
- `frontend/src/features/calendar/api/calendar.ts`
