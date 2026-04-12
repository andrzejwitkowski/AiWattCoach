# Workout Summary Create And Get Flows

> 40 nodes · cohesion 0.05

## Key Concepts

- **create_and_get.rs** (31 connections) — `tests\workout_summary_service\create_and_get.rs`
- **RecordingMissingSettingsService** (10 connections) — `tests\workout_summary_service\create_and_get.rs`
- **append_user_message_allows_chat_when_availability_is_configured()** (1 connections) — `tests\workout_summary_service\create_and_get.rs`
- **append_user_message_checks_summary_before_missing_availability()** (1 connections) — `tests\workout_summary_service\create_and_get.rs`
- **append_user_message_requires_configured_availability_before_chat()** (1 connections) — `tests\workout_summary_service\create_and_get.rs`
- **append_user_message_uses_find_settings_without_creating_defaults()** (1 connections) — `tests\workout_summary_service\create_and_get.rs`
- **create_summary_defaults_recap_fields_to_none()** (1 connections) — `tests\workout_summary_service\create_and_get.rs`
- **create_summary_is_idempotent_when_summary_already_exists()** (1 connections) — `tests\workout_summary_service\create_and_get.rs`
- **get_summary_returns_not_found_when_missing()** (1 connections) — `tests\workout_summary_service\create_and_get.rs`
- **mark_saved_generates_recap_and_plan_for_latest_completed_activity()** (1 connections) — `tests\workout_summary_service\create_and_get.rs`
- **mark_saved_generates_recap_only_for_finished_conversation_on_non_latest_activity()** (1 connections) — `tests\workout_summary_service\create_and_get.rs`
- **mark_saved_maps_training_plan_failure_to_repository_error_after_persisting_save()** (1 connections) — `tests\workout_summary_service\create_and_get.rs`
- **mark_saved_reports_failed_plan_generation_for_latest_completed_activity()** (1 connections) — `tests\workout_summary_service\create_and_get.rs`
- **mark_saved_requires_rpe()** (1 connections) — `tests\workout_summary_service\create_and_get.rs`
- **mark_saved_returns_workflow_statuses_after_persisting_saved_state()** (1 connections) — `tests\workout_summary_service\create_and_get.rs`
- **mark_saved_skips_generation_when_training_plan_service_is_not_configured()** (1 connections) — `tests\workout_summary_service\create_and_get.rs`
- **mark_saved_skips_recap_and_plan_without_finished_conversation()** (1 connections) — `tests\workout_summary_service\create_and_get.rs`
- **mark_saved_triggers_training_plan_generation_after_persisting_saved_state()** (1 connections) — `tests\workout_summary_service\create_and_get.rs`
- **persist_workout_recap_does_not_bump_updated_at_when_recap_is_already_stored()** (1 connections) — `tests\workout_summary_service\create_and_get.rs`
- **persist_workout_recap_is_idempotent_for_repeated_values()** (1 connections) — `tests\workout_summary_service\create_and_get.rs`
- **persist_workout_recap_updates_recap_fields_and_timestamp()** (1 connections) — `tests\workout_summary_service\create_and_get.rs`
- **.find_calls()** (1 connections) — `tests\workout_summary_service\create_and_get.rs`
- **.find_settings()** (1 connections) — `tests\workout_summary_service\create_and_get.rs`
- **.get_calls()** (1 connections) — `tests\workout_summary_service\create_and_get.rs`
- **.get_settings()** (1 connections) — `tests\workout_summary_service\create_and_get.rs`
- *... and 15 more nodes in this community*

## Relationships

- No strong cross-community connections detected

## Source Files

- `tests\workout_summary_service\create_and_get.rs`

## Audit Trail

- EXTRACTED: 79 (100%)
- INFERRED: 0 (0%)
- AMBIGUOUS: 0 (0%)

---

*Part of the graphify knowledge wiki. See [[index]] to navigate.*