# Compute Session Expiry Service

> 18 nodes · cohesion 0.16

## Key Concepts

- **service.rs** (8 connections) — `src\domain\identity\service.rs`
- **IdentityService<Users, Sessions, LoginStates, GoogleOAuth, Time, Ids>** (7 connections) — `src\domain\identity\service.rs`
- **.handle_google_callback()** (4 connections) — `src\domain\identity\service.rs`
- **compute_session_expiry()** (3 connections) — `src\domain\identity\service.rs`
- **.begin_google_login()** (3 connections) — `src\domain\identity\service.rs`
- **.get_current_user()** (3 connections) — `src\domain\identity\service.rs`
- **.new()** (3 connections) — `src\domain\identity\service.rs`
- **sanitize_return_to()** (3 connections) — `src\domain\identity\service.rs`
- **.get_valid_session()** (2 connections) — `src\domain\identity\service.rs`
- **.require_admin()** (2 connections) — `src\domain\identity\service.rs`
- **IdentityServiceConfig** (2 connections) — `src\domain\identity\service.rs`
- **validate_session_ttl_against_current_time()** (2 connections) — `src\domain\identity\service.rs`
- **GoogleLoginStart** (1 connections) — `src\domain\identity\service.rs`
- **GoogleLoginSuccess** (1 connections) — `src\domain\identity\service.rs`
- **IdentityService** (1 connections) — `src\domain\identity\service.rs`
- **.logout()** (1 connections) — `src\domain\identity\service.rs`
- **.new()** (1 connections) — `src\domain\identity\service.rs`
- **IdentityUseCases** (1 connections) — `src\domain\identity\service.rs`

## Relationships

- No strong cross-community connections detected

## Source Files

- `src\domain\identity\service.rs`

## Audit Trail

- EXTRACTED: 48 (100%)
- INFERRED: 0 (0%)
- AMBIGUOUS: 0 (0%)

---

*Part of the graphify knowledge wiki. See [[index]] to navigate.*