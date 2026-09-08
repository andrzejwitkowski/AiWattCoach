# Move planned workout: materialize-then-move

Date: 2026-09-08  
Status: draft (awaiting review)  
Depends on: existing `POST .../planned-workouts/{id}/move` behavior

## Goal

Make move operate on **one** durable source of truth: an **imported** `PlannedWorkout`.

Projected-only days are admitted only after an explicit **ensure-imported** step that persists local state and **supersedes** the active projection on the source date. Move then changes the imported row’s date (and id when date-suffixed), clears remotes, refreshes both days.

User-visible behavior stays the same: drag non-rest planned X→Y; Y empty or rest-only; synced remotes deleted; Y left unsynced.

## Non-goals

- Sharing provider delete/sync loops with update (separate ticket)
- Splitting `CalendarDayCell` DnD (separate ticket)
- Mobile DnD, LLM move tool, auto re-create remotes on Y
- Changing id scheme away from `operationKey:date` (keep rekey)

## Current problem

Move today branches through both stores for the whole pipeline: projected fallback load, dual occupancy scans, `relocate_active_date`, string rekey, imported upsert/delete. That is the dual-model tax.

## Approach (chosen)

**Materialize-then-move**, with projection policy **(1)**:

When ensure-imported creates/uses an imported row for a previously projected-only day on X, **supersede the active projection on X** so imported owns that day. Move does **not** call `relocate_active_date` on the happy path.

Rejected alternatives:

- **Projection-native move** — keeps two write paths forever
- **Unify reads only** — rearranges spaghetti without deleting relocate+rekey from move’s core path

## Invariants

1. After step “ensure imported”, the workout being moved exists in `planned_workouts` for `fromDate`.
2. Active non-rest projection on `fromDate` for that plan day is superseded once imported owns it (policy 1).
3. Persist local imported + projection supersede **before** Intervals/Wahoo deletes.
4. Target Y: empty or rest-only; reject other non-rest planned (imported or would-show projected), completed, race.
5. Sync states / remote deletes use the **pre-move** planned workout id; Y is unsynced.
6. Date-suffixed ids still rekey `…:fromDate` → `…:toDate` when the suffix matches; otherwise id unchanged.

## Flow

```
validate command
load source (imported on fromDate OR active projected matching id)
reject rest source / linked-to-completed
assert target allowed (single occupancy helper; date-scoped projection query preferred)
ensure imported on fromDate:
  if missing → map projected → upsert imported
  always supersede active projection on fromDate (imported owns X; even if imported already existed)
clear rest on toDate (imported delete + supersede projected rest on Y as today)
rekey id if needed; upsert moved imported; delete old imported id if rekeyed
delete remotes + clear sync for pre-move id (best-effort per provider)
refresh calendar view fromDate + toDate
```

## Components

| Piece | Responsibility |
| --- | --- |
| `ensure_imported_for_move` (domain helper) | Resolve projected → imported upsert; supersede projection on X; return `PlannedWorkout` |
| Occupancy helper | One place: imported non-rest on Y + active projected non-rest on Y (excluding the moving id) |
| `PlannedWorkoutMoveService::move_planned_workout` | Orchestrate validate → ensure → date move → remote delete → refresh; no projected fallback mid-pipeline |
| Projection port | Existing `supersede_active_dates`; `relocate_active_date` unused by move happy path (may remain for other callers) |
| REST / FE | Unchanged contracts |

## Error mapping

Unchanged semantics: `Validation` vs `Conflict` vs `NotFound` vs `Unavailable` / `Repository`. Ensure-imported mapping failures (missing projected payload) → `Validation` or `NotFound` as today for bad projected days.

## Testing

Domain (required):

- Projected-only source → ensure imported → move → imported on Y, none on X, projection on X superseded, no `relocate_active_date` required
- Imported already present → no second row; ensure still supersedes any active projection on fromDate, then moves
- Synced Intervals/Wahoo → remotes deleted; sync cleared for pre-move id
- Target conflicts: imported non-rest, projected non-rest, completed, race
- Rekey date-suffix ids; leave non-date ids alone

Optional: thin REST auth / 409 test if cheap.

## Done when

- `move.rs` does not branch “imported vs projected” through the whole pipeline — only at ensure-imported
- Happy-path move does not depend on `relocate_active_date`
- Existing user-facing move behavior and domain tests above pass
