# Workout Summary Messaging

> 39 nodes · cohesion 0.07

## Key Concepts

- **messaging.rs** (26 connections) — `tests\workout_summary_service\messaging.rs`
- **.new()** (14 connections) — `tests\workout_summary_service\messaging.rs`
- **FailingAthleteSummaryService** (5 connections) — `tests\workout_summary_service\messaging.rs`
- **CapturingAthleteSummaryCoach** (3 connections) — `tests\workout_summary_service\messaging.rs`
- **CountingCoach** (3 connections) — `tests\workout_summary_service\messaging.rs`
- **OneTimeFailureCoach** (3 connections) — `tests\workout_summary_service\messaging.rs`
- **AlwaysFailingCoach** (2 connections) — `tests\workout_summary_service\messaging.rs`
- **generate_coach_reply_continues_when_athlete_summary_is_unavailable()** (2 connections) — `tests\workout_summary_service\messaging.rs`
- **generate_coach_reply_marks_when_athlete_summary_was_regenerated()** (2 connections) — `tests\workout_summary_service\messaging.rs`
- **generate_coach_reply_passes_fresh_athlete_summary_text_to_coach_once()** (2 connections) — `tests\workout_summary_service\messaging.rs`
- **generate_coach_reply_preserves_oversized_context_error()** (2 connections) — `tests\workout_summary_service\messaging.rs`
- **generate_coach_reply_preserves_structured_llm_errors()** (2 connections) — `tests\workout_summary_service\messaging.rs`
- **generate_coach_reply_recovers_existing_message_without_losing_provider_metadata()** (2 connections) — `tests\workout_summary_service\messaging.rs`
- **generate_coach_reply_replays_persisted_response_for_saved_summary()** (2 connections) — `tests\workout_summary_service\messaging.rs`
- **generate_coach_reply_replays_persisted_response_message_after_partial_crash()** (2 connections) — `tests\workout_summary_service\messaging.rs`
- **generate_coach_reply_retries_after_non_retryable_failure()** (2 connections) — `tests\workout_summary_service\messaging.rs`
- **generate_coach_reply_retries_completion_write_after_coach_message_append()** (2 connections) — `tests\workout_summary_service\messaging.rs`
- **generate_coach_reply_retries_failure_checkpoint_write_before_returning()** (2 connections) — `tests\workout_summary_service\messaging.rs`
- **generate_coach_reply_retries_success_checkpoint_write_before_returning()** (2 connections) — `tests\workout_summary_service\messaging.rs`
- **generate_coach_reply_reuses_completed_operation_without_duplicate_coach_call()** (2 connections) — `tests\workout_summary_service\messaging.rs`
- **OversizedContextCoach** (2 connections) — `tests\workout_summary_service\messaging.rs`
- **.reply()** (1 connections) — `tests\workout_summary_service\messaging.rs`
- **append_user_message_persists_only_user_message()** (1 connections) — `tests\workout_summary_service\messaging.rs`
- **.athlete_summary_texts()** (1 connections) — `tests\workout_summary_service\messaging.rs`
- **.reply()** (1 connections) — `tests\workout_summary_service\messaging.rs`
- *... and 14 more nodes in this community*

## Relationships

- No strong cross-community connections detected

## Source Files

- `tests\workout_summary_service\messaging.rs`

## Audit Trail

- EXTRACTED: 102 (100%)
- INFERRED: 0 (0%)
- AMBIGUOUS: 0 (0%)

---

*Part of the graphify knowledge wiki. See [[index]] to navigate.*