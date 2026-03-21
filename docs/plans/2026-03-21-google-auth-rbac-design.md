# Google Auth RBAC Design

## Goal

Add a Stitch-based landing and login flow for AiWattCoach with Google OAuth, server-side sessions stored in MongoDB, automatic user provisioning, and role-based access control for `user` and `admin`.

## Product decisions

- The public route `/` becomes the new landing and login page.
- The current public overview page moves into an admin-only area as `System Info`.
- Authentication uses Google OAuth with minimal scopes: `openid`, `email`, and `profile`.
- The backend owns login, callback handling, session issuance, user persistence, and RBAC.
- `admin` is assigned from the normalized emails listed in `ADMIN_EMAILS`.

## Architecture

### Backend

Use a new hexagonal business context under `src/domain/identity`.

- `src/domain/identity/model.rs`
  - `AppUser`
  - `Role`
  - `AuthSession`
  - `GoogleIdentity`
  - `LoginState`
- `src/domain/identity/ports.rs`
  - `GoogleOAuthPort`
  - `UserRepository`
  - `SessionRepository`
  - `LoginStateRepository`
  - `Clock`
- `src/domain/identity/service.rs`
  - begin login
  - handle callback
  - resolve current user
  - logout
  - role authorization

Adapters:

- `src/adapters/google_oauth/*`
  - build Google authorize URL
  - exchange authorization code for tokens
  - fetch verified Google identity data
- `src/adapters/mongo/users.rs`
  - find/create/update users
- `src/adapters/mongo/sessions.rs`
  - create/find/delete server-side sessions
- `src/adapters/mongo/login_state.rs`
  - persist login state before redirect-sensitive work
- `src/adapters/rest/auth.rs`
  - `/api/auth/google/start`
  - `/api/auth/google/callback`
  - `/api/auth/me`
  - `/api/auth/logout`
- `src/adapters/rest/admin.rs`
  - `/api/admin/system-info`

### Frontend

Split the UI into small feature-focused modules.

- `frontend/src/features/auth/*`
  - auth API client
  - auth provider and hooks
  - route guards
  - user menu
- `frontend/src/features/landing/*`
  - small components derived from the Stitch layout
- `frontend/src/features/admin-system-info/*`
  - admin diagnostics components
- `frontend/src/pages/*`
  - `LandingPage`
  - `AppHomePage`
  - `SettingsPage`
  - `AdminSystemInfoPage`
- `frontend/src/app/*`
  - public and authenticated layouts

Routing:

- `/` public landing and login page
- `/app` authenticated app page
- `/settings` authenticated settings page
- `/admin/system-info` admin-only diagnostics page

Use `BrowserRouter` instead of `HashRouter` so OAuth redirects and deep links use normal SPA paths.

## Login flow

1. User opens `/`.
2. User clicks the Google sign-in CTA.
3. Backend creates and persists a login state record.
4. Backend redirects to Google OAuth.
5. Google redirects back to `/api/auth/google/callback`.
6. Backend validates state, exchanges the code, and fetches Google identity info.
7. Backend rejects login if the Google email is not verified.
8. Backend finds the user by Google subject and falls back to normalized email when helpful.
9. Backend creates or updates the app user.
10. Backend calculates roles, creates a server-side session, sets an `HttpOnly` cookie, and redirects into the app.

## Security and RBAC

- The backend stores sessions in MongoDB and sends only a session ID cookie to the browser.
- Cookie settings:
  - `HttpOnly`
  - `SameSite=Lax`
  - `Path=/`
  - `Secure` enabled outside local development
- Public endpoints:
  - `/`
  - `/health`
  - `/ready`
  - `/api/auth/google/start`
  - `/api/auth/google/callback`
- Authenticated endpoints:
  - `/api/auth/me`
  - `/api/auth/logout`
- Admin endpoints:
  - `/api/admin/*`

Role rules:

- Every authenticated user receives `user`.
- `admin` is additionally granted when `email_normalized` matches `ADMIN_EMAILS`.

## Data model

### `app_users`

- `_id`
- `user_id`
- `google_subject`
- `email`
- `email_normalized`
- `email_verified`
- `display_name`
- `avatar_url`
- `roles`

### `auth_sessions`

- `_id`
- `session_id`
- `user_id`
- `expires_at`
- `expires_at_epoch_seconds`
- `created_at_epoch_seconds`

### `oauth_login_states`

- `_id`
- `state_id`
- `return_to`
- `expires_at`
- `expires_at_epoch_seconds`
- `created_at_epoch_seconds`

The current persisted Mongo schema uses explicit document keys (`user_id` / `session_id` / `state_id`), keeps epoch-second timestamp fields for domain logic, and also persists BSON `expires_at` date fields for Mongo TTL indexes on sessions and login states.

## API contract

### `GET /api/auth/me`

Unauthenticated:

```json
{
  "authenticated": false
}
```

Authenticated:

```json
{
  "authenticated": true,
  "user": {
    "id": "user_123",
    "email": "athlete@gmail.com",
    "displayName": "Athlete",
    "avatarUrl": "https://...",
    "roles": ["user", "admin"]
  }
}
```

### `GET /api/admin/system-info`

Protected admin payload with app metadata and diagnostic values used by the migrated system info page.

## Testing

- backend settings tests for auth env vars
- domain tests for identity rules and RBAC policy
- service tests for login, callback, create/update user, session, and logout
- REST tests for protected endpoints and cookie behavior
- frontend tests for auth bootstrap, guards, route access, and landing/admin rendering
- final verification with `bun run test:all`

## Review sequence after implementation

1. backend reviewer
2. frontend reviewer
3. project owner validation against issue #33

Also inspect for common Copilot and CodeRabbit concerns:

- config ambiguity
- internal codes leaking to UI
- missing fallback branches
- weak auth edge-case coverage
- SPA fallback regression for `/api/*`
