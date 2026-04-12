# Session Endpoints Logout Clears Session Cookie

> 6 nodes · cohesion 0.33

## Key Concepts

- **session_endpoints.rs** (5 connections) — `tests\auth_rest\session_endpoints.rs`
- **logout_clears_session_cookie()** (1 connections) — `tests\auth_rest\session_endpoints.rs`
- **logout_forwards_session_id_to_identity_service()** (1 connections) — `tests\auth_rest\session_endpoints.rs`
- **me_reads_session_cookie_from_later_cookie_header()** (1 connections) — `tests\auth_rest\session_endpoints.rs`
- **me_returns_current_user_when_cookie_matches_session()** (1 connections) — `tests\auth_rest\session_endpoints.rs`
- **me_returns_unauthenticated_without_cookie()** (1 connections) — `tests\auth_rest\session_endpoints.rs`

## Relationships

- No strong cross-community connections detected

## Source Files

- `tests\auth_rest\session_endpoints.rs`

## Audit Trail

- EXTRACTED: 10 (100%)
- INFERRED: 0 (0%)
- AMBIGUOUS: 0 (0%)

---

*Part of the graphify knowledge wiki. See [[index]] to navigate.*