# Adapters Mongo Training Plan Snapshots

> 12 nodes · cohesion 0.18

## Key Concepts

- **MongoTrainingPlanSnapshotRepository** (7 connections) — `src\adapters\mongo\training_plan_snapshots.rs`
- **training_plan_snapshots.rs** (5 connections) — `src\adapters\mongo\training_plan_snapshots.rs`
- **.collection()** (2 connections) — `src\adapters\mongo\training_plan_snapshots.rs`
- **.new()** (2 connections) — `src\adapters\mongo\training_plan_snapshots.rs`
- **map_day_to_document()** (1 connections) — `src\adapters\mongo\training_plan_snapshots.rs`
- **map_document_to_day()** (1 connections) — `src\adapters\mongo\training_plan_snapshots.rs`
- **.ensure_indexes()** (1 connections) — `src\adapters\mongo\training_plan_snapshots.rs`
- **.find_by_operation_key()** (1 connections) — `src\adapters\mongo\training_plan_snapshots.rs`
- **.map_document_to_snapshot()** (1 connections) — `src\adapters\mongo\training_plan_snapshots.rs`
- **.map_snapshot_to_document()** (1 connections) — `src\adapters\mongo\training_plan_snapshots.rs`
- **TrainingPlanDayDocument** (1 connections) — `src\adapters\mongo\training_plan_snapshots.rs`
- **TrainingPlanSnapshotDocument** (1 connections) — `src\adapters\mongo\training_plan_snapshots.rs`

## Relationships

- No strong cross-community connections detected

## Source Files

- `src\adapters\mongo\training_plan_snapshots.rs`

## Audit Trail

- EXTRACTED: 24 (100%)
- INFERRED: 0 (0%)
- AMBIGUOUS: 0 (0%)

---

*Part of the graphify knowledge wiki. See [[index]] to navigate.*