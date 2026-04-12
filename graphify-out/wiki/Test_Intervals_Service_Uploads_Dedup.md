# Test Intervals Service Uploads Dedup

> 10 nodes · cohesion 0.20

## Key Concepts

- **uploads_dedup.rs** (9 connections) — `tests\intervals_service\uploads_dedup.rs`
- **upload_activity_does_not_dedupe_ride_and_virtualride()** (1 connections) — `tests\intervals_service\uploads_dedup.rs`
- **upload_activity_does_not_dedupe_when_external_ids_conflict_even_if_fallback_matches()** (1 connections) — `tests\intervals_service\uploads_dedup.rs`
- **upload_activity_normalizes_external_id_before_forwarding_to_api()** (1 connections) — `tests\intervals_service\uploads_dedup.rs`
- **upload_activity_persists_uploaded_activities()** (1 connections) — `tests\intervals_service\uploads_dedup.rs`
- **upload_activity_returns_cached_duplicate_without_credentials()** (1 connections) — `tests\intervals_service\uploads_dedup.rs`
- **upload_activity_returns_existing_activity_when_external_id_matches()** (1 connections) — `tests\intervals_service\uploads_dedup.rs`
- **upload_activity_returns_existing_activity_when_external_id_matches_after_trim()** (1 connections) — `tests\intervals_service\uploads_dedup.rs`
- **upload_activity_returns_existing_activity_when_fallback_identity_matches()** (1 connections) — `tests\intervals_service\uploads_dedup.rs`
- **upload_activity_uses_positive_timer_time_when_elapsed_time_is_zero()** (1 connections) — `tests\intervals_service\uploads_dedup.rs`

## Relationships

- No strong cross-community connections detected

## Source Files

- `tests\intervals_service\uploads_dedup.rs`

## Audit Trail

- EXTRACTED: 18 (100%)
- INFERRED: 0 (0%)
- AMBIGUOUS: 0 (0%)

---

*Part of the graphify knowledge wiki. See [[index]] to navigate.*