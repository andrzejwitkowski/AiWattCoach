# Google Auth RBAC Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a Stitch-based landing and login page with Google OAuth, server-side sessions, automatic user provisioning, and RBAC-protected frontend and backend flows.

**Architecture:** Introduce a new `identity` domain with ports for Google OAuth, Mongo user persistence, Mongo session persistence, and login-state persistence. Keep controllers thin, keep persistence behind adapters, and split the frontend into small feature-focused modules for auth, landing, and admin diagnostics.

**Tech Stack:** Rust, Axum, MongoDB, reqwest, React, Vite, TypeScript, Tailwind, Vitest, Testing Library

---

### Task 1: Save planning artifacts

**Files:**
- Create: `docs/plans/2026-03-21-google-auth-rbac-design.md`
- Create: `docs/plans/2026-03-21-google-auth-rbac.md`

**Step 1: Save the approved design doc**

Expected: the design doc explains Google OAuth, server-side sessions, `ADMIN_EMAILS`, route moves, and frontend decomposition.

**Step 2: Save the implementation plan**

Expected: the plan exists in `docs/plans/` with concrete backend, frontend, and verification tasks.

### Task 2: Add backend auth settings and tests

**Files:**
- Modify: `tests/settings.rs`
- Modify: `src/config/settings.rs`
- Modify: `.env.example`
- Modify: `README.md`

**Step 1: Write failing tests first**

Run: `cargo test settings_ -- --nocapture`
Expected: FAIL because auth settings are missing.

**Step 2: Implement auth settings**

Expected: backend settings support Google OAuth config, session config, and `ADMIN_EMAILS` normalization.

**Step 3: Re-run tests**

Run: `cargo test settings_ -- --nocapture`
Expected: PASS

### Task 3: Add identity domain model and service tests

**Files:**
- Create: `src/domain/identity/mod.rs`
- Create: `src/domain/identity/model.rs`
- Create: `src/domain/identity/ports.rs`
- Create: `src/domain/identity/service.rs`
- Modify: `src/domain/mod.rs`
- Create: `tests/identity_domain.rs`
- Create: `tests/identity_service.rs`

**Step 1: Write failing tests first**

Run: `cargo test identity_ -- --nocapture`
Expected: FAIL because the identity domain does not exist.

**Step 2: Implement minimal domain model and use cases**

Expected: user role assignment, Google identity validation, session lifecycle, and role authorization work through ports.

**Step 3: Re-run tests**

Run: `cargo test identity_ -- --nocapture`
Expected: PASS

### Task 4: Add Google OAuth adapter and Mongo identity repositories

**Files:**
- Modify: `Cargo.toml`
- Create: `src/adapters/google_oauth/mod.rs`
- Create: `src/adapters/google_oauth/client.rs`
- Create: `src/adapters/google_oauth/dto.rs`
- Create: `src/adapters/mongo/users.rs`
- Create: `src/adapters/mongo/sessions.rs`
- Create: `src/adapters/mongo/login_state.rs`
- Modify: `src/adapters/mod.rs`
- Modify: `src/adapters/mongo/mod.rs`

**Step 1: Write failing tests first**

Run: `cargo test google_oauth -- --nocapture`
Expected: FAIL because adapter code does not exist.

**Step 2: Implement the minimal adapters**

Expected: Google authorize URL generation, code exchange, user info mapping, and Mongo persistence adapters are available for wiring.

**Step 3: Re-run tests**

Run: `cargo test`
Expected: PASS on the new adapter and repository coverage.

### Task 5: Wire auth services and add REST endpoint tests

**Files:**
- Modify: `src/config/app_state.rs`
- Modify: `src/config/http.rs`
- Modify: `src/main.rs`
- Modify: `src/lib.rs`
- Create: `src/adapters/rest/auth.rs`
- Create: `src/adapters/rest/admin.rs`
- Modify: `src/adapters/rest/mod.rs`
- Create: `tests/auth_rest.rs`
- Create: `tests/auth_integration.rs`

**Step 1: Write failing tests first**

Run: `cargo test auth_ -- --nocapture`
Expected: FAIL because auth and admin routes do not exist.

**Step 2: Implement thin controllers and guards**

Expected: auth start, callback, `me`, logout, and admin system info routes work with server-side sessions and RBAC.

**Step 3: Re-run tests**

Run: `cargo test auth_ -- --nocapture`
Expected: PASS

### Task 6: Add frontend auth feature tests and bootstrap

**Files:**
- Modify: `frontend/src/lib/api/client.ts`
- Create: `frontend/src/features/auth/types.ts`
- Create: `frontend/src/features/auth/api/auth.ts`
- Create: `frontend/src/features/auth/api/auth.test.ts`
- Create: `frontend/src/features/auth/context/AuthProvider.tsx`
- Create: `frontend/src/features/auth/context/AuthProvider.test.tsx`
- Create: `frontend/src/features/auth/hooks/useAuth.ts`
- Create: `frontend/src/features/auth/guards/RequireAuth.tsx`
- Create: `frontend/src/features/auth/guards/RequireAuth.test.tsx`
- Create: `frontend/src/features/auth/guards/RequireRole.tsx`
- Create: `frontend/src/features/auth/guards/RequireRole.test.tsx`
- Create: `frontend/src/features/auth/components/UserMenu.tsx`

**Step 1: Write failing tests first**

Run: `bun run --cwd frontend test`
Expected: FAIL because the auth feature does not exist.

**Step 2: Implement the minimal auth feature modules**

Expected: frontend bootstraps the current user via `/api/auth/me`, includes credentials with requests, and protects routes by auth state and role.

**Step 3: Re-run tests**

Run: `bun run --cwd frontend test`
Expected: PASS

### Task 7: Add route redesign, landing page, and admin system info page

**Files:**
- Modify: `frontend/src/App.tsx`
- Modify: `frontend/src/App.test.tsx`
- Create: `frontend/src/app/PublicLayout.tsx`
- Create: `frontend/src/app/AuthenticatedLayout.tsx`
- Create: `frontend/src/pages/LandingPage.tsx`
- Create: `frontend/src/pages/LandingPage.test.tsx`
- Create: `frontend/src/pages/AppHomePage.tsx`
- Create: `frontend/src/pages/AdminSystemInfoPage.tsx`
- Create: `frontend/src/pages/AdminSystemInfoPage.test.tsx`
- Create: `frontend/src/features/landing/components/*`
- Create: `frontend/src/features/admin-system-info/components/*`
- Modify: `frontend/src/pages/HomePage.tsx`
- Modify: `frontend/src/pages/SettingsPage.tsx`
- Modify: `frontend/src/main.tsx`
- Modify: `frontend/src/styles.css`

**Step 1: Write failing tests first**

Run: `bun run --cwd frontend test`
Expected: FAIL because the new route structure and landing/admin pages do not exist.

**Step 2: Implement the new route structure and small UI components**

Expected: `/` is the public Stitch-based landing page, app routes are authenticated, and the old overview is available only under admin `System Info`.

**Step 3: Re-run tests and build**

Run:
- `bun run --cwd frontend test`
- `bun run --cwd frontend build`

Expected: PASS

### Task 8: Update frontend and backend environment support

**Files:**
- Modify: `frontend/vite.config.ts`
- Modify: `frontend/.env.example`
- Modify: `frontend/src/config/env.ts`
- Modify: `frontend/src/config/env.test.ts`
- Modify: `.env.example`
- Modify: `README.md`

**Step 1: Write failing tests first where config logic changes**

Run: `bun run --cwd frontend test`
Expected: FAIL if env support is incomplete.

**Step 2: Implement `/api` proxying and document auth env vars**

Expected: local development supports same-origin API calls and Google OAuth configuration is documented clearly.

**Step 3: Re-run tests**

Run: `bun run --cwd frontend test`
Expected: PASS

### Task 9: Run full verification

**Files:**
- Verify: `tests/settings.rs`
- Verify: `tests/identity_domain.rs`
- Verify: `tests/identity_service.rs`
- Verify: `tests/auth_rest.rs`
- Verify: `tests/auth_integration.rs`
- Verify: `frontend/src/**/*`

**Step 1: Run backend tests**

Run: `cargo test`
Expected: PASS

**Step 2: Run frontend tests**

Run: `bun run --cwd frontend test`
Expected: PASS

**Step 3: Run frontend build**

Run: `bun run --cwd frontend build`
Expected: PASS

**Step 4: Run the full gate**

Run: `bun run test:all`
Expected: PASS

### Task 10: Review in three passes

**Files:**
- Review: backend auth and identity files
- Review: frontend auth, landing, and admin files
- Review: final feature behavior against issue `#33`

**Step 1: Backend reviewer pass**

Expected: verify hexagonal boundaries, persistence ordering, session safety, and RBAC correctness.

**Step 2: Frontend reviewer pass**

Expected: verify component decomposition, route protection, auth bootstrap, and UI consistency.

**Step 3: Project owner pass**

Expected: verify that the delivered feature matches issue `#33` and the approved design.
