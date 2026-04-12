# Adapters Mongo Time

> 4 nodes · cohesion 0.83

## Key Concepts

- **time.rs** (3 connections) — `src\adapters\mongo\time.rs`
- **epoch_seconds_to_bson_datetime()** (3 connections) — `src\adapters\mongo\time.rs`
- **converts_epoch_seconds_to_bson_datetime()** (2 connections) — `src\adapters\mongo\time.rs`
- **rejects_epoch_seconds_that_overflow_bson_millis()** (2 connections) — `src\adapters\mongo\time.rs`

## Relationships

- No strong cross-community connections detected

## Source Files

- `src\adapters\mongo\time.rs`

## Audit Trail

- EXTRACTED: 10 (100%)
- INFERRED: 0 (0%)
- AMBIGUOUS: 0 (0%)

---

*Part of the graphify knowledge wiki. See [[index]] to navigate.*