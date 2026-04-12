# User Settings Mongo Documents

> 41 nodes · cohesion 0.08

## Key Concepts

- **settings.rs** (29 connections) — `src\adapters\mongo\settings.rs`
- **MongoUserSettingsRepository** (10 connections) — `src\adapters\mongo\settings.rs`
- **map_document_availability_to_domain()** (9 connections) — `src\adapters\mongo\settings.rs`
- **update_availability_updates_only_target_user_document()** (8 connections) — `src\adapters\mongo\settings.rs`
- **.new()** (5 connections) — `src\adapters\mongo\settings.rs`
- **build_settings_document()** (4 connections) — `src\adapters\mongo\settings.rs`
- **default_availability_document()** (4 connections) — `src\adapters\mongo\settings.rs`
- **map_domain_availability_to_document()** (4 connections) — `src\adapters\mongo\settings.rs`
- **map_domain_to_document()** (4 connections) — `src\adapters\mongo\settings.rs`
- **map_document_availability_to_domain_falls_back_for_legacy_empty_days()** (3 connections) — `src\adapters\mongo\settings.rs`
- **map_document_to_domain()** (3 connections) — `src\adapters\mongo\settings.rs`
- **map_domain_cycling_to_document()** (3 connections) — `src\adapters\mongo\settings.rs`
- **.update_availability()** (3 connections) — `src\adapters\mongo\settings.rs`
- **RedactedOptionalText** (3 connections) — `src\adapters\mongo\settings.rs`
- **repair_availability_days()** (3 connections) — `src\adapters\mongo\settings.rs`
- **CyclingDocument** (2 connections) — `src\adapters\mongo\settings.rs`
- **.fmt()** (2 connections) — `src\adapters\mongo\settings.rs`
- **default_availability_day_documents()** (2 connections) — `src\adapters\mongo\settings.rs`
- **has_complete_explicit_week()** (2 connections) — `src\adapters\mongo\settings.rs`
- **map_document_availability_to_domain_keeps_partial_legacy_week_unconfigured()** (2 connections) — `src\adapters\mongo\settings.rs`
- **map_document_availability_to_domain_repairs_case_and_missing_days()** (2 connections) — `src\adapters\mongo\settings.rs`
- **map_document_availability_to_domain_sanitizes_invalid_duration_without_resetting_week()** (2 connections) — `src\adapters\mongo\settings.rs`
- **map_document_availability_to_domain_treats_duplicate_weekdays_as_unconfigured()** (2 connections) — `src\adapters\mongo\settings.rs`
- **map_document_cycling_to_domain()** (2 connections) — `src\adapters\mongo\settings.rs`
- **.update_cycling()** (2 connections) — `src\adapters\mongo\settings.rs`
- *... and 16 more nodes in this community*

## Relationships

- No strong cross-community connections detected

## Source Files

- `src\adapters\mongo\settings.rs`

## Audit Trail

- EXTRACTED: 134 (100%)
- INFERRED: 0 (0%)
- AMBIGUOUS: 0 (0%)

---

*Part of the graphify knowledge wiki. See [[index]] to navigate.*