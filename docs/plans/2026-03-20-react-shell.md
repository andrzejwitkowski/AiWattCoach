# React Shell Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a separate React frontend with a minimal shell, a settings entry point, and a real connection to the Rust backend health endpoints.

**Architecture:** Keep the backend as the system-of-record API and add a standalone `frontend/` application that consumes explicit HTTP contracts. Put UI composition under `src/app`, route screens under `src/pages`, and backend calls behind a small API client layer.

**Tech Stack:** Bun, Vite, React, TypeScript, Tailwind CSS, Vitest, Testing Library, Rust, Axum

---

### Task 1: Save planning artifacts

**Files:**
- Create: `docs/plans/2026-03-20-react-shell-design.md`
- Create: `docs/plans/2026-03-20-react-shell.md`

**Step 1: Save the approved design doc**

Expected: the design exists in `docs/plans/` and explains the split frontend/backend architecture.

**Step 2: Save the implementation plan**

Expected: the plan exists in `docs/plans/` and lists concrete frontend, integration, and verification tasks.

### Task 2: Scaffold the frontend workspace

**Files:**
- Create: `frontend/package.json`
- Create: `frontend/tsconfig.json`
- Create: `frontend/vite.config.ts`
- Create: `frontend/index.html`
- Create: `frontend/src/*`

**Step 1: Create the frontend app shell structure**

Expected: `frontend/` builds as an independent Bun + Vite React app.

**Step 2: Add testing and styling dependencies**

Expected: the frontend can run tests and Tailwind styles.

### Task 3: Write failing frontend tests

**Files:**
- Create: `frontend/src/app/AppShell.test.tsx`
- Create: `frontend/src/lib/api/system.test.ts`

**Step 1: Write a failing shell test**

Run: `bun --cwd frontend test`
Expected: FAIL because the application shell and backend status UI do not exist yet.

**Step 2: Write a failing API client test**

Run: `bun --cwd frontend test`
Expected: FAIL because the health client and parsing logic do not exist yet.

### Task 4: Implement the minimal frontend shell

**Files:**
- Create: `frontend/src/main.tsx`
- Create: `frontend/src/App.tsx`
- Create: `frontend/src/app/AppShell.tsx`
- Create: `frontend/src/pages/HomePage.tsx`
- Create: `frontend/src/pages/SettingsPage.tsx`
- Create: `frontend/src/styles.css`

**Step 1: Implement the shell layout and navigation**

Expected: the app renders a responsive shell with home and settings entry points.

**Step 2: Implement the visual system**

Expected: the app uses Tailwind styling and looks intentional on desktop and mobile.

### Task 5: Implement backend connectivity

**Files:**
- Create: `frontend/src/config/env.ts`
- Create: `frontend/src/lib/api/client.ts`
- Create: `frontend/src/lib/api/system.ts`

**Step 1: Implement API base configuration**

Expected: the frontend resolves the backend base URL from environment safely.

**Step 2: Implement health and readiness calls**

Expected: the frontend loads backend status from the real Rust API.

### Task 6: Integrate repo workflows

**Files:**
- Modify: `README.md`
- Modify: `.github/workflows/ci.yml`
- Create: `package.json`

**Step 1: Document local development**

Expected: the repo explains how to run backend and frontend together.

**Step 2: Add shared verification entrypoint**

Expected: `bun test:all` runs repo-level verification that includes backend and frontend checks.

**Step 3: Extend CI**

Expected: CI verifies frontend install, tests, and build in addition to the Rust checks.

### Task 7: Run final verification

**Files:**
- Verify: `frontend/src/app/AppShell.test.tsx`
- Verify: `frontend/src/lib/api/system.test.ts`
- Verify: `.github/workflows/ci.yml`

**Step 1: Run frontend tests**

Run: `bun --cwd frontend test`
Expected: PASS

**Step 2: Run frontend build**

Run: `bun --cwd frontend build`
Expected: PASS

**Step 3: Run backend tests**

Run: `cargo test`
Expected: PASS

**Step 4: Run repo-level verification**

Run: `bun test:all`
Expected: PASS
