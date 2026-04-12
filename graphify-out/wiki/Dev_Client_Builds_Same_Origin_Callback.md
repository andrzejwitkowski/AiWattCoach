# Dev Client Builds Same Origin Callback

> 8 nodes · cohesion 0.50

## Key Concepts

- **.new()** (5 connections) — `src\adapters\google_oauth\dev_client.rs`
- **DevGoogleOAuthClient** (4 connections) — `src\adapters\google_oauth\dev_client.rs`
- **.exchange_code_for_identity()** (4 connections) — `src\adapters\google_oauth\dev_client.rs`
- **dev_client.rs** (4 connections) — `src\adapters\google_oauth\dev_client.rs`
- **builds_same_origin_callback_redirect()** (3 connections) — `src\adapters\google_oauth\dev_client.rs`
- **rejects_invalid_dev_code_as_unauthenticated()** (3 connections) — `src\adapters\google_oauth\dev_client.rs`
- **returns_configured_identity_for_dev_code()** (3 connections) — `src\adapters\google_oauth\dev_client.rs`
- **.build_authorize_url()** (2 connections) — `src\adapters\google_oauth\dev_client.rs`

## Relationships

- No strong cross-community connections detected

## Source Files

- `src\adapters\google_oauth\dev_client.rs`

## Audit Trail

- EXTRACTED: 28 (100%)
- INFERRED: 0 (0%)
- AMBIGUOUS: 0 (0%)

---

*Part of the graphify knowledge wiki. See [[index]] to navigate.*