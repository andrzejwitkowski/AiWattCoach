# Recovery Fail Operation Preserves Unavailable Error

> 7 nodes · cohesion 0.29

## Key Concepts

- **recovery.rs** (6 connections) — `tests\training_plan_service\recovery.rs`
- **fail_operation_preserves_unavailable_error_kind()** (1 connections) — `tests\training_plan_service\recovery.rs`
- **failed_operation_persistence_error_is_surfaced()** (1 connections) — `tests\training_plan_service\recovery.rs`
- **heals_pending_operation_when_snapshot_already_exists()** (1 connections) — `tests\training_plan_service\recovery.rs`
- **reclaim_resumes_from_stored_checkpoints_without_regenerating_completed_phases()** (1 connections) — `tests\training_plan_service\recovery.rs`
- **reclaim_with_stored_recap_skips_redundant_workout_summary_persistence()** (1 connections) — `tests\training_plan_service\recovery.rs`
- **replay_does_not_heal_pending_operation_when_snapshot_exists_without_projected_days()** (1 connections) — `tests\training_plan_service\recovery.rs`

## Relationships

- No strong cross-community connections detected

## Source Files

- `tests\training_plan_service\recovery.rs`

## Audit Trail

- EXTRACTED: 12 (100%)
- INFERRED: 0 (0%)
- AMBIGUOUS: 0 (0%)

---

*Part of the graphify knowledge wiki. See [[index]] to navigate.*