# Generation Checkpoints Recap In Operation Before

> 8 nodes · cohesion 0.25

## Key Concepts

- **generation.rs** (7 connections) — `tests\training_plan_service\generation.rs`
- **checkpoints_recap_in_operation_before_persisting_to_workout_summary()** (1 connections) — `tests\training_plan_service\generation.rs`
- **existing_pending_operation_returns_unavailable_without_calling_generator()** (1 connections) — `tests\training_plan_service\generation.rs`
- **generates_snapshot_and_projected_days_for_saved_workout()** (1 connections) — `tests\training_plan_service\generation.rs`
- **next_day_generation_supersedes_only_overlapping_future_projected_days()** (1 connections) — `tests\training_plan_service\generation.rs`
- **persists_workout_recap_before_generating_training_plan_window()** (1 connections) — `tests\training_plan_service\generation.rs`
- **replay_of_same_saved_workout_generation_is_idempotent()** (1 connections) — `tests\training_plan_service\generation.rs`
- **successful_generation_records_real_workflow_attempts()** (1 connections) — `tests\training_plan_service\generation.rs`

## Relationships

- No strong cross-community connections detected

## Source Files

- `tests\training_plan_service\generation.rs`

## Audit Trail

- EXTRACTED: 14 (100%)
- INFERRED: 0 (0%)
- AMBIGUOUS: 0 (0%)

---

*Part of the graphify knowledge wiki. See [[index]] to navigate.*