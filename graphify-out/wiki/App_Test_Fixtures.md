# App Test Fixtures

> 40 nodes · cohesion 0.08

## Key Concepts

- **LlmRestTestContext** (9 connections) — `tests\llm_rest\support\app.rs`
- **app.rs** (9 connections) — `tests\settings_rest\shared\app.rs`
- **frontend_fixture()** (8 connections) — `tests\workout_summary_rest\shared\app.rs`
- **test_mongo_client()** (8 connections) — `tests\workout_summary_rest\shared\app.rs`
- **app.rs** (8 connections) — `tests\intervals_rest\app.rs`
- **app.rs** (7 connections) — `tests\workout_summary_rest\shared\app.rs`
- **FrontendFixture** (6 connections) — `tests\workout_summary_rest\shared\app.rs`
- **app.rs** (6 connections) — `tests\llm_rest\support\app.rs`
- **EmptyTrainingPlanProjectionRepository** (5 connections) — `tests\intervals_rest\app.rs`
- **.dist_dir()** (5 connections) — `tests\workout_summary_rest\shared\app.rs`
- **intervals_test_app_with_projections()** (5 connections) — `tests\intervals_rest\app.rs`
- **settings_test_app_with_athlete_summary()** (5 connections) — `tests\settings_rest\shared\app.rs`
- **workout_summary_test_app_with_settings()** (5 connections) — `tests\workout_summary_rest\shared\app.rs`
- **InMemoryPlannedWorkoutSyncRepository** (4 connections) — `tests\intervals_rest\app.rs`
- **llm_rest_test_context()** (4 connections) — `tests\llm_rest\support\app.rs`
- **get_json()** (3 connections) — `tests\workout_summary_rest\shared\app.rs`
- **settings_test_app_with_intervals()** (3 connections) — `tests\settings_rest\shared\app.rs`
- **settings_test_app_with_services()** (3 connections) — `tests\settings_rest\shared\app.rs`
- **intervals_test_app()** (2 connections) — `tests\intervals_rest\app.rs`
- **session_cookie()** (2 connections) — `tests\workout_summary_rest\shared\app.rs`
- **settings_test_app()** (2 connections) — `tests\settings_rest\shared\app.rs`
- **TestClock** (2 connections) — `tests\intervals_rest\app.rs`
- **workout_summary_test_app()** (2 connections) — `tests\workout_summary_rest\shared\app.rs`
- **.find_active_by_operation_key()** (1 connections) — `tests\intervals_rest\app.rs`
- **.find_active_by_user_id_and_operation_key()** (1 connections) — `tests\intervals_rest\app.rs`
- *... and 15 more nodes in this community*

## Relationships

- No strong cross-community connections detected

## Source Files

- `tests\intervals_rest\app.rs`
- `tests\llm_rest\support\app.rs`
- `tests\settings_rest\shared\app.rs`
- `tests\workout_summary_rest\shared\app.rs`

## Audit Trail

- EXTRACTED: 130 (100%)
- INFERRED: 0 (0%)
- AMBIGUOUS: 0 (0%)

---

*Part of the graphify knowledge wiki. See [[index]] to navigate.*