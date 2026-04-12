# Domain Workout Summary Service Use Cases

> 15 nodes · cohesion 0.15

## Key Concepts

- **WorkoutSummaryService<Repo, Ops, Time, Ids>** (10 connections) — `src\domain\workout_summary\service\use_cases.rs`
- **.send_message()** (3 connections) — `src\domain\workout_summary\service\use_cases.rs`
- **use_cases.rs** (2 connections) — `src\domain\workout_summary\service\use_cases.rs`
- **RecapSnapshot** (2 connections) — `src\domain\workout_summary\service\use_cases.rs`
- **.from_summary()** (2 connections) — `src\domain\workout_summary\service\use_cases.rs`
- **.append_user_message()** (2 connections) — `src\domain\workout_summary\service\use_cases.rs`
- **.generate_coach_reply()** (2 connections) — `src\domain\workout_summary\service\use_cases.rs`
- **.mark_saved()** (2 connections) — `src\domain\workout_summary\service\use_cases.rs`
- **status_message()** (1 connections) — `src\domain\workout_summary\service\use_cases.rs`
- **.create_summary()** (1 connections) — `src\domain\workout_summary\service\use_cases.rs`
- **.get_summary()** (1 connections) — `src\domain\workout_summary\service\use_cases.rs`
- **.list_summaries()** (1 connections) — `src\domain\workout_summary\service\use_cases.rs`
- **.persist_workout_recap()** (1 connections) — `src\domain\workout_summary\service\use_cases.rs`
- **.reopen_summary()** (1 connections) — `src\domain\workout_summary\service\use_cases.rs`
- **.update_rpe()** (1 connections) — `src\domain\workout_summary\service\use_cases.rs`

## Relationships

- No strong cross-community connections detected

## Source Files

- `src\domain\workout_summary\service\use_cases.rs`

## Audit Trail

- EXTRACTED: 32 (100%)
- INFERRED: 0 (0%)
- AMBIGUOUS: 0 (0%)

---

*Part of the graphify knowledge wiki. See [[index]] to navigate.*