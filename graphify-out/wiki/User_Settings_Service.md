# User Settings Service

> 35 nodes · cohesion 0.12

## Key Concepts

- **UserSettingsService<Repo, Time>** (10 connections) — `src\domain\settings\service.rs`
- **.get_or_create()** (10 connections) — `src\domain\settings\service.rs`
- **service.rs** (10 connections) — `src\domain\settings\service.rs`
- **InMemoryUserSettingsRepository** (9 connections) — `src\domain\settings\service.rs`
- **.find_by_user_id()** (8 connections) — `src\domain\settings\service.rs`
- **.now_epoch_seconds()** (7 connections) — `src\domain\settings\service.rs`
- **.update_ai_agents()** (7 connections) — `src\domain\settings\service.rs`
- **.new()** (6 connections) — `src\domain\settings\service.rs`
- **.with_settings()** (5 connections) — `src\domain\settings\service.rs`
- **RecordingCacheRepository** (5 connections) — `src\domain\settings\service.rs`
- **update_ai_agents_invalidates_llm_cache_when_provider_config_changes()** (5 connections) — `src\domain\settings\service.rs`
- **update_ai_agents_skips_llm_cache_invalidation_when_provider_config_is_unchanged()** (5 connections) — `src\domain\settings\service.rs`
- **.update_availability()** (5 connections) — `src\domain\settings\service.rs`
- **.update_cycling()** (5 connections) — `src\domain\settings\service.rs`
- **.update_intervals()** (5 connections) — `src\domain\settings\service.rs`
- **.update_options()** (5 connections) — `src\domain\settings\service.rs`
- **.update_ai_agents()** (4 connections) — `src\domain\settings\service.rs`
- **update_availability_normalizes_inconsistent_configured_flag()** (4 connections) — `src\domain\settings\service.rs`
- **find_settings_does_not_create_defaults_when_missing()** (3 connections) — `src\domain\settings\service.rs`
- **.update_availability()** (3 connections) — `src\domain\settings\service.rs`
- **.find_settings()** (3 connections) — `src\domain\settings\service.rs`
- **.with_llm_context_cache_repository()** (3 connections) — `src\domain\settings\service.rs`
- **.update_cycling()** (2 connections) — `src\domain\settings\service.rs`
- **.update_intervals()** (2 connections) — `src\domain\settings\service.rs`
- **.update_options()** (2 connections) — `src\domain\settings\service.rs`
- *... and 10 more nodes in this community*

## Relationships

- No strong cross-community connections detected

## Source Files

- `src\domain\settings\service.rs`

## Audit Trail

- EXTRACTED: 148 (100%)
- INFERRED: 0 (0%)
- AMBIGUOUS: 0 (0%)

---

*Part of the graphify knowledge wiki. See [[index]] to navigate.*