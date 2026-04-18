# Training Load History Design

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Persist historically correct FTP changes and daily training-load projections so charts and LLM context can use domain-derived `TSS`, `CTL`, `ATL`, `TSB`, and related metrics.

**Architecture:** `completed_workouts` stays the durable source of workout facts, `ftp_history` stores UTC-effective FTP changes, and `training_load_daily_snapshots` stores per-day computed read models. Snapshot recompute runs after completed-workout import batches and after FTP changes in settings, so the app does not recalculate two years of history per imported workout.

**Tech Stack:** Rust, MongoDB, chrono, existing domain ports/adapters, Axum wiring in `main.rs`

---

## Scope

- Fetch two years of completed workouts on initial Intervals bootstrap instead of 30 days.
- Calculate training-load metrics from canonical domain workouts, not directly from provider models.
- Keep `IF` and `EF` from imported workout metrics.
- Use application-managed FTP over time for recomputed historical metrics.
- Preserve historical correctness by storing FTP changes in their own collection.
- Provide a durable per-day projection for future chart rendering and LLM context.

## Data Model

### `ftp_history`

- `user_id`
- `effective_from_date` as UTC `YYYY-MM-DD`
- `ftp_watts`
- `source`
- `created_at_epoch_seconds`
- `updated_at_epoch_seconds`

Indexes:

- unique `(user_id, effective_from_date)`

Rules:

- Each settings FTP change writes or updates the entry for the current UTC day.
- The first entry is backfilled from `settings.cycling.ftp_watts` using `settings.created_at_epoch_seconds`.
- This collection is the source of truth for which FTP was effective on a given day.

### `training_load_daily_snapshots`

- `user_id`
- `date`
- `daily_tss`
- `rolling_tss_7d`
- `rolling_tss_28d`
- `ctl`
- `atl`
- `tsb`
- `average_if_28d`
- `average_ef_28d`
- `ftp_effective_watts`
- `ftp_source`
- `recomputed_at_epoch_seconds`
- `created_at_epoch_seconds`
- `updated_at_epoch_seconds`

Indexes:

- unique `(user_id, date)`

Rules:

- This is a projection/read model, not a source of truth.
- It is recomputed from canonical workouts plus FTP history.
- It must be queryable efficiently by `user_id` and date range.

## Computation Rules

- Use canonical `completed_workouts` as the input workout series.
- Use `ftp_history` to select the effective FTP for each day.
- For days before the athlete entered the app, do not apply application FTP retroactively.
- The app-entry boundary is `settings.created_at_epoch_seconds`, converted to UTC date.
- For workouts on or after the app-entry date, app-managed FTP history is allowed to drive recomputation.
- `IF` and `EF` stay sourced from imported workout metrics.
- `TSS`, `CTL`, `ATL`, and `TSB` are derived from canonical data plus effective FTP.

## Integration Points

### Settings updates

- Extend cycling settings updates to detect FTP changes.
- Persist settings first.
- Seed or append `ftp_history` entries when FTP changes.
- Trigger training-load recompute from the changed effective date through today.
- Invalidate LLM context cache after the successful update path.

### Completed workout polling

- Keep importing completed workouts through `ExternalImportService`.
- Increase initial completed-workout fetch window to two years.
- After a successful poll batch, trigger one recompute from the earliest affected workout date through today.
- Do not recompute once per imported workout during bootstrap.

### Training context

- Historical aggregates and trend points should come from `training_load_daily_snapshots`.
- Recent workout details can still come from `completed_workouts`.
- `ftp_change` should come from chronological FTP history, not from provider workout ordering.

## Recommended Module Layout

- `src/domain/training_load/model.rs`
- `src/domain/training_load/ports.rs`
- `src/domain/training_load/service.rs`
- `src/domain/training_load/use_cases.rs`
- `src/adapters/mongo/ftp_history.rs`
- `src/adapters/mongo/training_load_daily_snapshots.rs`

## Verification Expectations

- Unit tests for FTP-history lookup and snapshot calculation.
- Settings-service tests for seed and update behavior.
- Polling tests proving one recompute per successful workout batch.
- Training-context tests proving historical metrics come from snapshots.
- Final verification with formatting, clippy, architecture checks, and full test suite.
