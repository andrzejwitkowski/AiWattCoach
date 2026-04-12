# Main And Review Followups Tests

> 59 nodes · cohesion 0.05

## Key Concepts

- **TestWorkoutSummaryService** (16 connections) — `tests\workout_summary_rest\shared\workout_summary.rs`
- **main_tests.rs** (7 connections) — `src\main_tests.rs`
- **workout_summary_mongo.rs** (7 connections) — `tests\workout_summary_mongo.rs`
- **MongoTrainingPlanProjectionRepository** (7 connections) — `src\adapters\mongo\training_plan_projections.rs`
- **2026-04-06-review-followups.md** (6 connections) — `docs\plans\2026-04-06-review-followups.md`
- **Make projection replay heal partial same-operation inserts** (6 connections) — `docs\plans\2026-04-06-review-followups.md`
- **Close low-level test and docs nits** (6 connections) — `docs\plans\2026-04-06-review-followups.md`
- **Make workout-summary legacy fallback deterministic** (6 connections) — `docs\plans\2026-04-06-review-followups.md`
- **training_plan_projections.rs** (6 connections) — `src\adapters\mongo\training_plan_projections.rs`
- **.new()** (6 connections) — `tests\workout_summary_mongo.rs`
- **workout_summary_repository_list_uses_legacy_fallback_when_current_match_is_absent()** (6 connections) — `tests\workout_summary_mongo.rs`
- **SharedLogBuffer** (5 connections) — `src\main_tests.rs`
- **Preserve correction retry budget across reclaim** (5 connections) — `docs\plans\2026-04-06-review-followups.md`
- **Run targeted tests, fmt, and clippy for the follow-up batch** (5 connections) — `docs\plans\2026-04-06-review-followups.md`
- **Close final-review findings around projection replay healing, workout-summary legacy identifiers, correction retry budget, and test/doc nits** (5 connections) — `docs\plans\2026-04-06-review-followups.md`
- **mongo_fixture_or_skip()** (5 connections) — `tests\workout_summary_mongo.rs`
- **workout_summary_repository_prefers_current_workout_id_over_legacy_event_id()** (5 connections) — `tests\workout_summary_mongo.rs`
- **MongoFixture** (4 connections) — `tests\workout_summary_mongo.rs`
- **.cleanup()** (4 connections) — `tests\workout_summary_mongo.rs`
- **workout_summary_repository_creates_legacy_event_id_index()** (4 connections) — `tests\workout_summary_mongo.rs`
- **.contents()** (3 connections) — `src\main_tests.rs`
- **.new()** (3 connections) — `src\adapters\mongo\training_plan_projections.rs`
- **.collection()** (3 connections) — `tests\workout_summary_mongo.rs`
- **sample_summary()** (3 connections) — `tests\workout_summary_mongo.rs`
- **ctrl_c_registration_error_logs_and_does_not_finish_shutdown_future()** (2 connections) — `src\main_tests.rs`
- *... and 34 more nodes in this community*

## Relationships

- No strong cross-community connections detected

## Source Files

- `docs\plans\2026-04-06-review-followups.md`
- `src\adapters\mongo\training_plan_projections.rs`
- `src\main_tests.rs`
- `tests\training_plan_service\main.rs`
- `tests\workout_summary_mongo.rs`
- `tests\workout_summary_rest\shared\workout_summary.rs`

## Audit Trail

- EXTRACTED: 176 (100%)
- INFERRED: 0 (0%)
- AMBIGUOUS: 0 (0%)

---

*Part of the graphify knowledge wiki. See [[index]] to navigate.*