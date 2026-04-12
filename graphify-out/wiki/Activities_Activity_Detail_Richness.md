# Activities Activity Detail Richness

> 24 nodes · cohesion 0.16

## Key Concepts

- **MongoActivityRepository** (12 connections) — `src\adapters\mongo\activities.rs`
- **activities.rs** (12 connections) — `src\adapters\mongo\activities.rs`
- **normalize_activity()** (9 connections) — `src\adapters\mongo\activities.rs`
- **merge_activity_for_storage()** (7 connections) — `src\adapters\mongo\activities.rs`
- **.upsert()** (4 connections) — `src\adapters\mongo\activities.rs`
- **normalize_activity_document()** (4 connections) — `src\adapters\mongo\activities.rs`
- **build_activity_document()** (3 connections) — `src\adapters\mongo\activities.rs`
- **infer_event_id_hint()** (3 connections) — `src\adapters\mongo\activities.rs`
- **merge_activity_details()** (3 connections) — `src\adapters\mongo\activities.rs`
- **.find_by_user_id_and_fallback_identity()** (3 connections) — `src\adapters\mongo\activities.rs`
- **.find_by_user_id_and_range()** (3 connections) — `src\adapters\mongo\activities.rs`
- **.new()** (3 connections) — `src\adapters\mongo\activities.rs`
- **prefer_non_empty()** (3 connections) — `src\adapters\mongo\activities.rs`
- **activity_detail_richness()** (2 connections) — `src\adapters\mongo\activities.rs`
- **merge_activity_metrics()** (2 connections) — `src\adapters\mongo\activities.rs`
- **.cleanup_legacy_time_streams()** (2 connections) — `src\adapters\mongo\activities.rs`
- **.find_by_user_id_and_activity_id()** (2 connections) — `src\adapters\mongo\activities.rs`
- **.find_by_user_id_and_external_id()** (2 connections) — `src\adapters\mongo\activities.rs`
- **.find_latest_by_user_id()** (2 connections) — `src\adapters\mongo\activities.rs`
- **.upsert_many()** (2 connections) — `src\adapters\mongo\activities.rs`
- **should_store_stream_type()** (2 connections) — `src\adapters\mongo\activities.rs`
- **ActivityDocument** (1 connections) — `src\adapters\mongo\activities.rs`
- **.delete()** (1 connections) — `src\adapters\mongo\activities.rs`
- **.ensure_indexes()** (1 connections) — `src\adapters\mongo\activities.rs`

## Relationships

- No strong cross-community connections detected

## Source Files

- `src\adapters\mongo\activities.rs`

## Audit Trail

- EXTRACTED: 88 (100%)
- INFERRED: 0 (0%)
- AMBIGUOUS: 0 (0%)

---

*Part of the graphify knowledge wiki. See [[index]] to navigate.*