# Backend Setup Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Bootstrap the AiWattCoach Rust backend with Axum, MongoDB wiring, environment configuration, and a matching Obsidian handbook for the project.

**Architecture:** Use a single Rust crate with folders aligned to ports-and-adapters boundaries. Keep startup wiring in `config`, HTTP in `adapters/rest`, Mongo bootstrap in `adapters/mongo`, and leave domain code framework-independent.

**Tech Stack:** Rust, Cargo, Axum, Tokio, MongoDB Rust driver, Serde, Tower, Obsidian Markdown

---

### Task 1: Branch and planning artifacts

**Files:**
- Create: `docs/plans/2026-03-20-backend-setup-design.md`
- Create: `docs/plans/2026-03-20-backend-setup.md`

**Step 1: Create the feature branch**

Run: `git checkout -b feature/task-3-backend-setup`
Expected: branch created and checked out

**Step 2: Save design and plan docs**

Expected: both files exist under `docs/plans/`

### Task 2: Create minimal Obsidian project handbook

**Files:**
- Create: external vault folder `obsidian/andrzej.witkowski/`
- Create: external notes under `obsidian/andrzej.witkowski/opencode/`

**Step 1: Create a root getting-started note**

Expected: note links into the project handbook and explains how to start OpenCode sessions for this project.

**Step 2: Create minimal Rust-specific handbook notes**

Expected: the handbook covers architecture boundaries, Rust backend rules, Mongo reliability, HTTP adapter rules, clean code, and completion checks.

### Task 3: Write the failing health-route test

**Files:**
- Create: `tests/health_check.rs`
- Modify: `Cargo.toml`

**Step 1: Add dependencies needed for HTTP testing**

Expected: `axum`, `tokio`, `tower`, `serde`, `serde_json`, and test support dependencies are declared.

**Step 2: Write a failing integration test for `GET /health`**

Expected: test compiles once app factory exists and initially fails because the route/app bootstrap does not exist yet.

### Task 4: Write the failing settings test

**Files:**
- Create: `src/config/settings.rs`
- Create: `tests/settings.rs`

**Step 1: Write a test for loading required settings from env**

Expected: test fails because the settings loader has not been implemented yet.

### Task 5: Implement minimal application bootstrap

**Files:**
- Create: `Cargo.toml`
- Create: `src/lib.rs`
- Create: `src/main.rs`
- Create: `src/config/mod.rs`
- Create: `src/config/app_state.rs`
- Create: `src/config/settings.rs`
- Create: `src/config/http.rs`
- Create: `src/adapters/mod.rs`
- Create: `src/adapters/rest/mod.rs`
- Create: `src/adapters/rest/health.rs`
- Create: `src/adapters/mongo/mod.rs`
- Create: `src/adapters/mongo/client.rs`
- Create: `src/domain/mod.rs`
- Create: `src/domain/system/mod.rs`

**Step 1: Implement settings loader and app state**

Expected: configuration is parsed from env and shared through application state.

**Step 2: Implement Mongo bootstrap**

Expected: app startup creates a Mongo client from configuration and stores it in app state.

**Step 3: Implement Axum router and `/health` handler**

Expected: route returns a small JSON payload with service status.

**Step 4: Implement `main` startup**

Expected: server binds using configured host/port and serves the application.

### Task 6: Verify tests and baseline build

**Files:**
- Verify: `tests/health_check.rs`
- Verify: `tests/settings.rs`

**Step 1: Run targeted tests**

Run: `cargo test health_check`
Expected: PASS

**Step 2: Run settings test**

Run: `cargo test settings`
Expected: PASS

**Step 3: Run full backend verification**

Run: `cargo test`
Expected: PASS
