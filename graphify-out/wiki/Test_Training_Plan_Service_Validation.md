# Test Training Plan Service Validation

> 10 nodes · cohesion 0.20

## Key Concepts

- **validation.rs** (9 connections) — `tests\training_plan_service\validation.rs`
- **correction_ignores_changes_for_dates_that_were_not_invalid()** (1 connections) — `tests\training_plan_service\validation.rs`
- **correction_retry_exhaustion_marks_failed_operation_and_keeps_raw_responses()** (1 connections) — `tests\training_plan_service\validation.rs`
- **correction_retry_keeps_omitted_invalid_day_in_retry_set()** (1 connections) — `tests\training_plan_service\validation.rs`
- **correction_retry_persists_latest_invalid_scope_after_scope_shifts()** (1 connections) — `tests\training_plan_service\validation.rs`
- **correction_round_merges_corrected_days_and_succeeds()** (1 connections) — `tests\training_plan_service\validation.rs`
- **duplicate_dates_fail_durably_instead_of_overwriting()** (1 connections) — `tests\training_plan_service\validation.rs`
- **invalid_day_parse_records_date_scoped_validation_issue()** (1 connections) — `tests\training_plan_service\validation.rs`
- **non_contiguous_windows_fail_durably()** (1 connections) — `tests\training_plan_service\validation.rs`
- **reclaim_with_stored_invalid_correction_response_keeps_full_retry_budget()** (1 connections) — `tests\training_plan_service\validation.rs`

## Relationships

- No strong cross-community connections detected

## Source Files

- `tests\training_plan_service\validation.rs`

## Audit Trail

- EXTRACTED: 18 (100%)
- INFERRED: 0 (0%)
- AMBIGUOUS: 0 (0%)

---

*Part of the graphify knowledge wiki. See [[index]] to navigate.*