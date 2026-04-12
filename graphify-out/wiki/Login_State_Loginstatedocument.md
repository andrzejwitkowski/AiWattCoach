# Login State Loginstatedocument

> 12 nodes · cohesion 0.23

## Key Concepts

- **MongoLoginStateRepository** (7 connections) — `src\adapters\mongo\login_state.rs`
- **login_state.rs** (4 connections) — `src\adapters\mongo\login_state.rs`
- **.from_login_state()** (3 connections) — `src\adapters\mongo\login_state.rs`
- **.new()** (3 connections) — `src\adapters\mongo\login_state.rs`
- **rejects_login_state_expiry_that_cannot_be_converted_to_bson_datetime()** (3 connections) — `src\adapters\mongo\login_state.rs`
- **LoginStateDocument** (2 connections) — `src\adapters\mongo\login_state.rs`
- **map_login_state_document()** (2 connections) — `src\adapters\mongo\login_state.rs`
- **.create()** (2 connections) — `src\adapters\mongo\login_state.rs`
- **.consume()** (1 connections) — `src\adapters\mongo\login_state.rs`
- **.delete()** (1 connections) — `src\adapters\mongo\login_state.rs`
- **.ensure_indexes()** (1 connections) — `src\adapters\mongo\login_state.rs`
- **.find_by_id()** (1 connections) — `src\adapters\mongo\login_state.rs`

## Relationships

- No strong cross-community connections detected

## Source Files

- `src\adapters\mongo\login_state.rs`

## Audit Trail

- EXTRACTED: 30 (100%)
- INFERRED: 0 (0%)
- AMBIGUOUS: 0 (0%)

---

*Part of the graphify knowledge wiki. See [[index]] to navigate.*