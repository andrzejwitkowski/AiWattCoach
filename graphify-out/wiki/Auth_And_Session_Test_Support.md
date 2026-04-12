# Auth And Session Test Support

> 68 nodes · cohesion 0.04

## Key Concepts

- **shared.rs** (12 connections) — `tests\health_check\shared.rs`
- **shared.rs** (10 connections) — `tests\auth_rest\shared.rs`
- **TestIdentityService** (9 connections) — `tests\identity_service\shared.rs`
- **TestSettingsService** (8 connections) — `tests\auth_rest\shared.rs`
- **shared.rs** (8 connections) — `tests\identity_service\shared.rs`
- **frontend_fixture()** (7 connections) — `tests\health_check\shared.rs`
- **FrontendFixture** (7 connections) — `tests\health_check\shared.rs`
- **InMemoryUsers** (7 connections) — `tests\identity_service\shared.rs`
- **test_mongo_client()** (7 connections) — `tests\health_check\shared.rs`
- **.dist_dir()** (6 connections) — `tests\health_check\shared.rs`
- **auth_test_app()** (5 connections) — `tests\auth_rest\shared.rs`
- **auth_test_app_with_custom_settings()** (5 connections) — `tests\auth_rest\shared.rs`
- **auth_test_app_with_settings()** (5 connections) — `tests\auth_rest\shared.rs`
- **auth_test_app_without_identity()** (5 connections) — `tests\auth_rest\shared.rs`
- **health_test_app()** (5 connections) — `tests\health_check\shared.rs`
- **InMemoryLoginStates** (5 connections) — `tests\identity_service\shared.rs`
- **.save_google_user_for_identity()** (5 connections) — `tests\identity_service\shared.rs`
- **keep_frontend_fixture()** (5 connections) — `tests\auth_rest\shared.rs`
- **HealthTestApp** (4 connections) — `tests\health_check\shared.rs`
- **InMemorySessions** (4 connections) — `tests\identity_service\shared.rs`
- **TestGoogleOAuthAdapter** (3 connections) — `tests\identity_service\shared.rs`
- **.app_js()** (2 connections) — `tests\health_check\shared.rs`
- **.index_html()** (2 connections) — `tests\health_check\shared.rs`
- **.no_extension_file()** (2 connections) — `tests\health_check\shared.rs`
- **.app_js()** (2 connections) — `tests\health_check\shared.rs`
- *... and 43 more nodes in this community*

## Relationships

- No strong cross-community connections detected

## Source Files

- `tests\auth_rest\shared.rs`
- `tests\health_check\shared.rs`
- `tests\identity_service\shared.rs`

## Audit Trail

- EXTRACTED: 192 (100%)
- INFERRED: 0 (0%)
- AMBIGUOUS: 0 (0%)

---

*Part of the graphify knowledge wiki. See [[index]] to navigate.*