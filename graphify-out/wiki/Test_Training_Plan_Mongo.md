# Test Training Plan Mongo

> 18 nodes · cohesion 0.36

## Key Concepts

- **training_plan_mongo.rs** (16 connections) — `tests\training_plan_mongo.rs`
- **.new()** (11 connections) — `tests\training_plan_mongo.rs`
- **mongo_fixture_or_skip()** (10 connections) — `tests\training_plan_mongo.rs`
- **.cleanup()** (9 connections) — `tests\training_plan_mongo.rs`
- **training_plan_projection_repository_replaces_window_and_supersedes_overlapping_future_days()** (7 connections) — `tests\training_plan_mongo.rs`
- **sample_snapshot()** (6 connections) — `tests\training_plan_mongo.rs`
- **training_plan_projection_repository_keeps_past_days_active_when_late_window_replacement_runs()** (6 connections) — `tests\training_plan_mongo.rs`
- **training_plan_projection_repository_replay_heals_partial_same_operation_inserts()** (6 connections) — `tests\training_plan_mongo.rs`
- **training_plan_snapshot_repository_finds_snapshot_by_operation_key()** (6 connections) — `tests\training_plan_mongo.rs`
- **sample_projected_days()** (5 connections) — `tests\training_plan_mongo.rs`
- **training_plan_generation_operation_repository_round_trips_and_reclaims_failed_operations()** (5 connections) — `tests\training_plan_mongo.rs`
- **training_plan_generation_operation_repository_round_trips_recap_timestamp()** (5 connections) — `tests\training_plan_mongo.rs`
- **sample_operation()** (4 connections) — `tests\training_plan_mongo.rs`
- **training_plan_projection_repository_creates_operation_unsuperseded_date_index()** (4 connections) — `tests\training_plan_mongo.rs`
- **training_plan_snapshot_repository_creates_unique_operation_key_index()** (4 connections) — `tests\training_plan_mongo.rs`
- **MongoFixture** (3 connections) — `tests\training_plan_mongo.rs`
- **sample_snapshot_for_user()** (3 connections) — `tests\training_plan_mongo.rs`
- **sample_planned_workout()** (1 connections) — `tests\training_plan_mongo.rs`

## Relationships

- No strong cross-community connections detected

## Source Files

- `tests\training_plan_mongo.rs`

## Audit Trail

- EXTRACTED: 111 (100%)
- INFERRED: 0 (0%)
- AMBIGUOUS: 0 (0%)

---

*Part of the graphify knowledge wiki. See [[index]] to navigate.*