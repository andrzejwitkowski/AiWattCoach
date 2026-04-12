# Athlete Summary Service

> 58 nodes · cohesion 0.09

## Key Concepts

- **athlete_summary_service.rs** (30 connections) — `tests\athlete_summary_service.rs`
- **.new()** (22 connections) — `tests\athlete_summary_service.rs`
- **new_call_log()** (17 connections) — `tests\athlete_summary_service.rs`
- **.succeeds_with()** (16 connections) — `tests\athlete_summary_service.rs`
- **InMemoryAthleteSummaryOperationRepository** (10 connections) — `tests\athlete_summary_service.rs`
- **generate_summary_force_true_ignores_completed_operation_and_regenerates()** (8 connections) — `tests\athlete_summary_service.rs`
- **generate_summary_non_force_reuses_completed_operation_with_persisted_summary_without_second_generator_call()** (8 connections) — `tests\athlete_summary_service.rs`
- **.with_operation()** (8 connections) — `tests\athlete_summary_service.rs`
- **InMemoryAthleteSummaryRepository** (8 connections) — `tests\athlete_summary_service.rs`
- **.with_summary()** (8 connections) — `tests\athlete_summary_service.rs`
- **generate_summary_reclaims_failed_operation_and_retries()** (7 connections) — `tests\athlete_summary_service.rs`
- **generate_summary_reclaims_stale_pending_operation()** (7 connections) — `tests\athlete_summary_service.rs`
- **generate_summary_when_missing_claims_pending_operation_before_calling_generator()** (7 connections) — `tests\athlete_summary_service.rs`
- **push_call()** (7 connections) — `tests\athlete_summary_service.rs`
- **summary()** (7 connections) — `tests\athlete_summary_service.rs`
- **ensure_fresh_summary_reads_repository_once_when_summary_is_fresh()** (6 connections) — `tests\athlete_summary_service.rs`
- **ensure_fresh_summary_regenerates_when_older_than_monday()** (6 connections) — `tests\athlete_summary_service.rs`
- **ensure_fresh_summary_reuses_summary_generated_this_week()** (6 connections) — `tests\athlete_summary_service.rs`
- **generate_summary_force_true_regenerates_even_when_fresh()** (6 connections) — `tests\athlete_summary_service.rs`
- **generate_summary_ignores_stale_completed_operation_when_persisted_summary_is_missing()** (6 connections) — `tests\athlete_summary_service.rs`
- **generate_summary_recovers_from_completed_operation_record_without_regenerating()** (6 connections) — `tests\athlete_summary_service.rs`
- **completed_operation()** (5 connections) — `tests\athlete_summary_service.rs`
- **FailingUpsertAthleteSummaryOperationRepository** (5 connections) — `tests\athlete_summary_service.rs`
- **ensure_fresh_summary_generates_when_missing()** (4 connections) — `tests\athlete_summary_service.rs`
- **FailingUpsertAthleteSummaryRepository** (4 connections) — `tests\athlete_summary_service.rs`
- *... and 33 more nodes in this community*

## Relationships

- No strong cross-community connections detected

## Source Files

- `tests\athlete_summary_service.rs`

## Audit Trail

- EXTRACTED: 290 (100%)
- INFERRED: 0 (0%)
- AMBIGUOUS: 0 (0%)

---

*Part of the graphify knowledge wiki. See [[index]] to navigate.*