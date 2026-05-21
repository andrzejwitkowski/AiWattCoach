# Training Plan Supervisor Status UI

## Status Sources

- External sync status tracks provider lifecycle: `unsynced`, `pending`, `synced`, `modified`, `failed`.
- Supervisor status tracks AI review lifecycle: `pending`, `accepted`, `replaced`, `failed`.
- Missing supervisor status means the visible planned workout is worker-generated without a supervisor review state.

## UI Surfaces

- Calendar day cells show supervisor status independently from sync status for predicted planned workouts.
- Planned workout details show sync status and supervisor status as separate badges.
- Worker-generated planned workouts without supervisor status show a neutral `Worker generated` label instead of overloading sync state.

## Read Model Contract

- `training_plan_projected_days.supervisor_status` is carried into `calendar_entry_views.supervisor_status` during calendar refresh.
- Calendar list responses expose it as `projectedWorkout.supervisorStatus`.
- Gemini supervisor webhook completion refreshes the affected active projection range so the calendar read model reflects accepted, replaced, or failed reviews.
