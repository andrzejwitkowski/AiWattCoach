# Adapters Mongo Users

> 16 nodes · cohesion 0.23

## Key Concepts

- **MongoUserRepository** (12 connections) — `src\adapters\mongo\users.rs`
- **.save_google_user_for_identity()** (6 connections) — `src\adapters\mongo\users.rs`
- **.upsert_google_user()** (5 connections) — `src\adapters\mongo\users.rs`
- **.build_user_document()** (4 connections) — `src\adapters\mongo\users.rs`
- **.new()** (4 connections) — `src\adapters\mongo\users.rs`
- **users.rs** (3 connections) — `src\adapters\mongo\users.rs`
- **map_user_document()** (3 connections) — `src\adapters\mongo\users.rs`
- **.find_by_normalized_email()** (3 connections) — `src\adapters\mongo\users.rs`
- **.save()** (3 connections) — `src\adapters\mongo\users.rs`
- **.from_user()** (3 connections) — `src\adapters\mongo\users.rs`
- **.find_by_google_subject()** (2 connections) — `src\adapters\mongo\users.rs`
- **.find_one_by_google_subject()** (2 connections) — `src\adapters\mongo\users.rs`
- **.find_one_by_normalized_email()** (2 connections) — `src\adapters\mongo\users.rs`
- **UserDocument** (2 connections) — `src\adapters\mongo\users.rs`
- **.ensure_indexes()** (1 connections) — `src\adapters\mongo\users.rs`
- **.find_by_id()** (1 connections) — `src\adapters\mongo\users.rs`

## Relationships

- No strong cross-community connections detected

## Source Files

- `src\adapters\mongo\users.rs`

## Audit Trail

- EXTRACTED: 56 (100%)
- INFERRED: 0 (0%)
- AMBIGUOUS: 0 (0%)

---

*Part of the graphify knowledge wiki. See [[index]] to navigate.*